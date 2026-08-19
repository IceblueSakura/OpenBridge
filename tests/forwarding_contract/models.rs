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

    // Exercise a compiled Embeddings model through both Models projections without copying its catalog fields.
    let compiled = app_with_compiled_registry(Arc::new(RecordingTransport::default()));
    let standard_list = compiled_authenticated_get(&compiled, "/v1/models").await;
    let standard = standard_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "qwen3.7-text-embedding")
        .expect("compiled Embeddings model should be listed");
    let standard_detail =
        compiled_authenticated_get(&compiled, "/v1/models/qwen3.7-text-embedding").await;
    assert_eq!(&standard_detail, standard);

    let extended_list = compiled_authenticated_get(&compiled, "/openbridge/v1/models").await;
    let extended = extended_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "qwen3.7-text-embedding")
        .expect("compiled Embeddings capability DTO should be listed");
    let extended_detail =
        compiled_authenticated_get(&compiled, "/openbridge/v1/models/qwen3.7-text-embedding").await;
    assert_eq!(&extended_detail, extended);
    assert_eq!(
        extended["capabilities"]["tasks"],
        serde_json::json!(["embedding"])
    );
    assert_eq!(
        extended["interfaces"]["chat_completions"],
        serde_json::json!(null)
    );
    assert_eq!(extended["interfaces"]["responses"], serde_json::json!(null));
    let dimensions = &extended["interfaces"]["embeddings"]["dimensions"];
    let default_dimension = dimensions["default"].as_u64().unwrap();
    assert!(default_dimension > 0);
    assert!(
        dimensions["allowed"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dimension| dimension.as_u64() == Some(default_dimension))
    );

    // Project Qwen3.8 Max through both HTTP catalogs with two actionable Native interfaces.
    let qwen38_standard = standard_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "qwen3.8-max")
        .expect("compiled Qwen3.8 Max model should be listed");
    let qwen38_standard_detail =
        compiled_authenticated_get(&compiled, "/v1/models/qwen3.8-max").await;
    assert_eq!(&qwen38_standard_detail, qwen38_standard);

    let qwen38_extended = extended_list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "qwen3.8-max")
        .expect("compiled Qwen3.8 Max capability DTO should be listed");
    let qwen38_extended_detail =
        compiled_authenticated_get(&compiled, "/openbridge/v1/models/qwen3.8-max").await;
    assert_eq!(&qwen38_extended_detail, qwen38_extended);
    assert_eq!(
        qwen38_extended["capabilities"]["tasks"],
        serde_json::json!(["chat", "text_generation"])
    );
    let expected_levels =
        serde_json::json!(["none", "minimal", "low", "medium", "high", "xhigh", "max"]);
    for interface in ["chat_completions", "responses"] {
        let reasoning = &qwen38_extended["interfaces"][interface]["reasoning"];
        assert_eq!(reasoning["levels"], expected_levels, "{interface}");
        assert_eq!(reasoning["accepted_levels"], expected_levels, "{interface}");
        assert_eq!(
            reasoning["input_policy"], "clamp_positive_floor",
            "{interface}"
        );
    }
}

#[tokio::test]
async fn kimi_k3_models_contract_excludes_unsupported_reasoning_none() {
    let app = app_with_compiled_registry(Arc::new(RecordingTransport::default()));
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/kimi-k3").await;
    let expected_levels = serde_json::json!(["low", "high", "max"]);
    let expected_accepted_levels =
        serde_json::json!(["minimal", "low", "medium", "high", "xhigh", "max"]);

    // Kimi K3 always reasons; positive input levels may clamp, but none remains unavailable.
    for interface in ["chat_completions", "responses"] {
        let reasoning = &model["interfaces"][interface]["reasoning"];
        assert_eq!(reasoning["levels"], expected_levels, "{interface}");
        assert_eq!(
            reasoning["accepted_levels"], expected_accepted_levels,
            "{interface}"
        );
    }
}

#[tokio::test]
async fn longcat_models_contract_uses_the_confirmed_context_window() {
    let app = app_with_compiled_registry(Arc::new(RecordingTransport::default()));
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/LongCat-2.0").await;
    let context = &model["capabilities"]["context_window"];

    // Preserve the official total context value without propagating a transposed catalog digit.
    assert_eq!(context["max_context_tokens"], 1_048_576);
    assert_eq!(context["max_input_tokens"], 1_048_576);
}

#[tokio::test]
async fn deepseek_models_contract_matches_confirmed_direct_parameter_boundaries() {
    let app = app_with_compiled_registry(Arc::new(RecordingTransport::default()));
    let expected_chat = serde_json::json!([
        "frequency_penalty",
        "logprobs",
        "max_tokens",
        "presence_penalty",
        "reasoning_effort",
        "response_format",
        "stop",
        "stream",
        "stream_options",
        "structured_outputs",
        "temperature",
        "tool_choice",
        "tools",
        "top_logprobs",
        "top_p"
    ]);
    let expected_responses = serde_json::json!([
        "frequency_penalty",
        "max_output_tokens",
        "presence_penalty",
        "reasoning",
        "stream",
        "structured_outputs",
        "temperature",
        "text",
        "tool_choice",
        "tools",
        "top_logprobs",
        "top_p",
        "user"
    ]);

    for model_id in ["deepseek-v4-pro", "deepseek-v4-flash"] {
        let model =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model_id}")).await;
        assert_eq!(
            model["interfaces"]["chat_completions"]["supported_parameters"], expected_chat,
            "{model_id} Chat"
        );
        assert_eq!(
            model["interfaces"]["responses"]["supported_parameters"], expected_responses,
            "{model_id} Responses"
        );
        assert_eq!(
            model["interfaces"]["responses"]["response_includes"],
            serde_json::json!([]),
            "{model_id} Responses include"
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
            mode: RouteMode::Bridged,
        },
        RouteConfig {
            id: "responses-native-chat-bridge".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::Bridged,
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
