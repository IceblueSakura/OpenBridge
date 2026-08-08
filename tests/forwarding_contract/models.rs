//! Verifies authenticated Models projection, lifecycle visibility, and compiled model facts.

use super::*;

#[tokio::test]
async fn models_lists_only_public_models_after_authentication() {
    let app = app_with_transport(Arc::new(RecordingTransport::default()));
    // The standard list returns only the four standard OpenAI Model fields.
    let standard_list = authenticated_get(&app, "/v1/models").await;
    assert_eq!(standard_list["object"], "list");
    assert_eq!(
        standard_list["data"],
        serde_json::json!([
            {
                "id": "public-model",
                "object": "model",
                "created": 1_785_715_200_u64,
                "owned_by": "openbridge"
            }
        ])
    );

    // The standard single-model object matches a list element exactly; an unknown ID returns a safe 404.
    let standard_detail = authenticated_get(&app, "/v1/models/public-model").await;
    assert_eq!(standard_detail, standard_list["data"][0]);
    let unknown = authenticated_response(&app, "/v1/models/not-configured").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let unknown: Value =
        serde_json::from_slice(&to_bytes(unknown.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(unknown["error"]["code"], "model_not_found");
    assert_eq!(unknown["error"]["param"], "model");
    let unknown = authenticated_response(&app, "/openbridge/v1/models/not-configured").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let unknown: Value =
        serde_json::from_slice(&to_bytes(unknown.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(unknown["error"]["code"], "model_not_found");
    assert_eq!(unknown["error"]["param"], "model");

    // The extended list and single-model endpoints share one complete capability DTO.
    let extended_list = authenticated_get(&app, "/openbridge/v1/models").await;
    assert_eq!(extended_list["object"], "list");
    let extended = &extended_list["data"][0];
    let extended_detail = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    assert_eq!(&extended_detail, extended);
    assert_eq!(extended["schema_version"], "1");
    assert_eq!(extended["name"], "Test public model");
    assert_eq!(extended["lifecycle"]["status"], "active");
    assert!(
        extended["capabilities"]
            .get("supported_parameters")
            .is_none(),
        "model facts must not duplicate the actionable interface parameter contract"
    );
    assert_eq!(
        extended["interfaces"]["chat_completions"]["supported_parameters"],
        serde_json::json!(["stream", "tool_choice", "tools"])
    );
    assert_eq!(
        extended["capabilities"]["context_window"],
        serde_json::json!({
            "max_context_tokens": 128_000,
            "max_input_tokens": null,
            "max_output_tokens": 8_192
        })
    );
    assert_eq!(
        extended["interfaces"]["chat_completions"]["tools"]["support"],
        "supported"
    );
    assert_eq!(
        extended["interfaces"]["chat_completions"]["tools"]["parallel_calls"],
        "unsupported"
    );
    assert_eq!(
        extended["interfaces"]["responses"]["state"]["previous_response_id"],
        "unsupported"
    );

    // Public objects must not expose deployment topology, upstream models, endpoints, or credential-pool IDs.
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
async fn compiled_models_endpoint_exposes_gpt_sol_model_facts() {
    let app = app_with_compiled_registry(Arc::new(RecordingTransport::default()));
    let list = compiled_authenticated_get(&app, "/openbridge/v1/models").await;
    let gpt_sol = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "gpt-5.6-sol")
        .expect("compiled GPT model should be listed");

    assert_eq!(
        gpt_sol["description"],
        "OpenAI flagship model for complex reasoning, coding, and multi-step agentic workflows."
    );
    assert_eq!(
        gpt_sol["capabilities"]["context_window"],
        serde_json::json!({
            "max_context_tokens": 272_000,
            "max_input_tokens": 272_000,
            "max_output_tokens": 128_000
        })
    );
    assert_eq!(
        gpt_sol["capabilities"]["modalities"]["input"],
        serde_json::json!(["text", "image", "file"])
    );
    assert_eq!(gpt_sol["capabilities"]["tokenizer"], "GPT");
    assert_eq!(gpt_sol["capabilities"]["knowledge_cutoff"], "2026-02-16");
    assert_eq!(
        gpt_sol["capabilities"]["reasoning"]["levels"],
        serde_json::json!(["none", "low", "medium", "high", "xhigh", "max"])
    );
    assert_eq!(
        gpt_sol["interfaces"]["chat_completions"]["context_window"]["max_input_tokens"],
        1_050_000
    );

    let detail = compiled_authenticated_get(&app, "/openbridge/v1/models/gpt-5.6-sol").await;
    assert_eq!(detail, *gpt_sol);
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
