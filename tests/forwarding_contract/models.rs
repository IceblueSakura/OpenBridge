//! Verifies authenticated Models projection, topology privacy, and lifecycle visibility.

use super::*;

#[tokio::test]
async fn models_endpoints_preserve_public_projection_and_hide_topology() {
    let app = app_with_transport(Arc::new(RecordingTransport::default()));

    // Keep standard list and detail responses on the strict four-field OpenAI projection.
    let standard_list = authenticated_get(&app, "/v1/models").await;
    assert_eq!(standard_list["object"], "list");
    assert_eq!(
        standard_list["data"],
        serde_json::json!([{
            "id": "public-model",
            "object": "model",
            "created": 1_785_715_200_u64,
            "owned_by": "openbridge"
        }])
    );
    let standard_detail = authenticated_get(&app, "/v1/models/public-model").await;
    assert_eq!(standard_detail, standard_list["data"][0]);

    // Return the same safe error shape from standard and extended unknown-model lookups.
    for path in [
        "/v1/models/not-configured",
        "/openbridge/v1/models/not-configured",
    ] {
        let response = authenticated_response(&app, path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], "model_not_found");
        assert_eq!(error["error"]["param"], "model");
    }

    // Keep the extension list and detail on one actionable capability DTO.
    let extended_list = authenticated_get(&app, "/openbridge/v1/models").await;
    assert_eq!(extended_list["object"], "list");
    let extended = &extended_list["data"][0];
    let extended_detail = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    assert_eq!(&extended_detail, extended);
    assert!(
        extended["capabilities"]
            .get("supported_parameters")
            .is_none(),
        "model facts must not duplicate the actionable interface parameter contract"
    );
    assert!(extended["interfaces"]["chat_completions"].is_object());
    assert!(extended["interfaces"]["responses"].is_object());

    // Prevent internal deployment identities from entering either public representation.
    let serialized = serde_json::to_string(&extended_list).unwrap();
    for private_value in [
        "openai-main",
        "upstream-model",
        "api.openai.com",
        "openai-primary",
        "routes",
        "upstream_api",
    ] {
        assert!(
            !serialized.contains(private_value),
            "leaked {private_value}"
        );
    }
}

#[tokio::test]
async fn extended_models_filter_by_executable_native_generation_protocol() {
    // Give each Public Model one Native protocol and one opposite-direction Bridge surface.
    let mut definition = support::definition("native-filter-test", "template", "upstream-model");
    let template = definition.public_models.remove(0);
    definition.routes = vec![
        RouteConfig {
            id: "chat-native-chat".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "chat-native-responses-bridge".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: OperationKind::Responses,
            mode: RouteMode::GenerationBridge(GenerationBridgeDirection::ResponsesToChat),
        },
        RouteConfig {
            id: "responses-native-chat-bridge".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::GenerationBridge(GenerationBridgeDirection::ChatToResponses),
        },
        RouteConfig {
            id: "responses-native-responses".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::Responses,
            mode: RouteMode::Native,
        },
    ];
    definition.public_models = vec![
        openbridge::registry::PublicModelConfig {
            id: "chat-native".to_owned(),
            display_name: "Chat Native".to_owned(),
            routes: vec![
                "chat-native-chat".to_owned(),
                "chat-native-responses-bridge".to_owned(),
            ],
            ..template.clone()
        },
        openbridge::registry::PublicModelConfig {
            id: "responses-native".to_owned(),
            display_name: "Responses Native".to_owned(),
            routes: vec![
                "responses-native-chat-bridge".to_owned(),
                "responses-native-responses".to_owned(),
            ],
            ..template
        },
    ];
    let app =
        app_with_transport_and_definition(Arc::new(RecordingTransport::default()), definition);

    // Omission preserves the deterministic full list; each filter keeps only a true Native surface.
    let unfiltered = authenticated_get(&app, "/openbridge/v1/models").await;
    assert_eq!(
        unfiltered["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|model| model["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["chat-native", "responses-native"]
    );
    for (protocol, expected_id) in [
        ("chat_completions", "chat-native"),
        ("responses", "responses-native"),
    ] {
        let filtered = authenticated_get(
            &app,
            &format!("/openbridge/v1/models?native_protocol={protocol}"),
        )
        .await;
        assert_eq!(filtered["object"], "list");
        assert_eq!(filtered["data"].as_array().unwrap().len(), 1);
        assert_eq!(filtered["data"][0]["id"], expected_id);
        let original = unfiltered["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["id"] == expected_id)
            .unwrap();
        assert_eq!(&filtered["data"][0], original);

        // Filtering is a private execution-snapshot predicate, not a new topology projection.
        let serialized = serde_json::to_string(&filtered).unwrap();
        for private_value in [
            "\"routes\"",
            "\"upstream_target\"",
            "\"upstream_model\"",
            "\"mode\"",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }

    // Reject malformed or misspelled filters so callers cannot mistake an unfiltered list for a match.
    for (path, expected_code, expected_param) in [
        (
            "/openbridge/v1/models?native_protocol=",
            "invalid_query_parameter",
            "native_protocol",
        ),
        (
            "/openbridge/v1/models?native_protocol=embeddings",
            "invalid_query_parameter",
            "native_protocol",
        ),
        (
            "/openbridge/v1/models?native_protocol=responses&native_protocol=chat_completions",
            "invalid_query_parameter",
            "native_protocol",
        ),
        (
            "/openbridge/v1/models?protocol=responses",
            "unknown_parameter",
            "protocol",
        ),
    ] {
        let response = authenticated_response(&app, path).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["type"], "invalid_request_error", "{path}");
        assert_eq!(error["error"]["code"], expected_code, "{path}");
        assert_eq!(error["error"]["param"], expected_param, "{path}");
    }
}

#[tokio::test]
async fn retired_public_models_are_hidden_and_cannot_be_requested() {
    // Mark a valid Public Model as disabled while preserving valid lifecycle timestamps.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.public_models[0].lifecycle = openbridge::registry::ModelLifecycle {
        status: openbridge::registry::ModelLifecycleStatus::Retired,
        deprecated_at: None,
        retired_at: Some(definition.public_models[0].created + 1),
    };
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Standard and extended catalogs share one visibility check, and details hide existence consistently.
    for path in ["/v1/models", "/openbridge/v1/models"] {
        let list = authenticated_get(&app, path).await;
        assert_eq!(list["data"], serde_json::json!([]));
    }
    for path in [
        "/v1/models/public-model",
        "/openbridge/v1/models/public-model",
    ] {
        let response = authenticated_response(&app, path).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // The generation path reads the same catalog; a disabled model must return 404 before any egress.
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(transport.requests.lock().unwrap().is_empty());
}
