//! Verifies authenticated Models projection, topology privacy, and lifecycle visibility.

use super::*;

#[tokio::test]
async fn target_tool_choice_restriction_is_public_and_rejected_before_egress() {
    // Exercise the observed direct-API restriction through production registration and preflight.
    let transport = Arc::new(MimoImageTransport::default());
    let bootstrap = support::bootstrap(support::BOOTSTRAP);
    let registry = build_compiled_registry_with_active_pools(
        bootstrap,
        &std::collections::BTreeSet::from(["deepseek-primary".to_owned()]),
    )
    .unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-00000000000000000000000000000000",
        &registry,
        "upstream-token",
    );
    let app = build_router(GatewayState::new(
        Arc::new(registry),
        transport.clone(),
        users,
        credentials,
    ));
    let model = "deepseek-v4-flash-vision-exp";
    let tool = serde_json::json!({
        "name": "lookup", "parameters": {"type": "object", "properties": {}}
    });
    for (path, protocol) in [
        ("/v1/chat/completions", "chat_completions"),
        ("/v1/responses", "responses"),
    ] {
        let mut body = serde_json::json!({"model": model, "stream": false});
        if protocol == "chat_completions" {
            body["messages"] = serde_json::json!([{"role": "user", "content": "hello"}]);
            body["tools"] = serde_json::json!([{"type": "function", "function": tool}]);
        } else {
            body["input"] = serde_json::json!("hello");
            let mut response_tool = tool.clone();
            response_tool["type"] = serde_json::json!("function");
            body["tools"] = serde_json::json!([response_tool]);
        }
        let named = if protocol == "chat_completions" {
            serde_json::json!({"type": "function", "function": {"name": "lookup"}})
        } else {
            serde_json::json!({"type": "function", "name": "lookup"})
        };

        // A rejected tool mode must not reach transport, regardless of advertised delivery.
        for choice in [serde_json::json!("required"), named] {
            for streaming in [false, true] {
                body["tool_choice"] = choice.clone();
                body["stream"] = serde_json::json!(streaming);
                transport.requests.lock().unwrap().clear();
                let response = app
                    .clone()
                    .oneshot(
                        Request::post(path)
                            .header(
                                AUTHORIZATION,
                                "Bearer downstream-token-00000000000000000000000000000000",
                            )
                            .header(CONTENT_TYPE, "application/json")
                            .body(Body::from(body.to_string()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    response.status(),
                    StatusCode::BAD_REQUEST,
                    "{protocol} {choice}"
                );
                let error: Value =
                    serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                        .unwrap();
                assert_eq!(error["error"]["param"], "tool_choice");
                assert!(transport.requests.lock().unwrap().is_empty());
            }
        }

        // Keep the actionable Models contract aligned without narrowing sibling targets.
        let public =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model}")).await;
        assert_eq!(
            public["interfaces"][protocol]["tools"]["tool_choice_modes"],
            serde_json::json!(["none", "auto"])
        );
        let sibling =
            compiled_authenticated_get(&app, "/openbridge/v1/models/deepseek-v4-flash").await;
        assert!(
            sibling["interfaces"][protocol]["tools"]["tool_choice_modes"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("required"))
        );

        // Accepted controls still reach the same Native wire unchanged.
        body["stream"] = serde_json::json!(false);
        for choice in ["auto", "none"] {
            body["tool_choice"] = serde_json::json!(choice);
            transport.requests.lock().unwrap().clear();
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .header(
                            AUTHORIZATION,
                            "Bearer downstream-token-00000000000000000000000000000000",
                        )
                        .header(CONTENT_TYPE, "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            to_bytes(response.into_body(), 65536).await.unwrap();
            let recorded = transport.requests.lock().unwrap();
            assert_eq!(recorded.len(), 1);
            assert_eq!(recorded[0].body["tool_choice"], choice);
        }
    }
}

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
    for protocol in ["chat_completions", "responses"] {
        let parameters = extended["interfaces"][protocol]["supported_parameters"]
            .as_array()
            .unwrap();
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter == "prompt_cache_key"),
            "{protocol} must advertise the downstream-safe cache hint"
        );
    }

    // Prevent internal deployment identities from entering either public representation.
    let serialized = serde_json::to_string(&extended_list).unwrap();
    for private_value in [
        "openai-main",
        "upstream-model",
        "api.openai.com",
        "openai-primary",
        "routes",
        "upstream_api",
        "forwards_prompt_cache_key",
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
    definition.public_models = vec![
        openbridge::registry::PublicModelConfig {
            id: "chat-native".to_owned(),
            display_name: "Chat Native".to_owned(),
            routes: vec![
                RouteConfig {
                    upstream_target: "openai-main".to_owned(),
                    upstream_operation: OperationKind::ChatCompletions,
                    downstream_operation: OperationKind::ChatCompletions,
                },
                RouteConfig {
                    upstream_target: "openai-main".to_owned(),
                    upstream_operation: OperationKind::ChatCompletions,
                    downstream_operation: OperationKind::Responses,
                },
            ],
            ..template.clone()
        },
        openbridge::registry::PublicModelConfig {
            id: "responses-native".to_owned(),
            display_name: "Responses Native".to_owned(),
            routes: vec![
                RouteConfig {
                    upstream_target: "openai-main".to_owned(),
                    upstream_operation: OperationKind::Responses,
                    downstream_operation: OperationKind::ChatCompletions,
                },
                RouteConfig {
                    upstream_target: "openai-main".to_owned(),
                    upstream_operation: OperationKind::Responses,
                    downstream_operation: OperationKind::Responses,
                },
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
