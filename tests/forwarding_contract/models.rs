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
    let expected_levels =
        serde_json::json!(["none", "minimal", "low", "medium", "high", "xhigh", "max"]);
    for interface in ["chat_completions", "responses"] {
        assert_eq!(
            qwen38_extended["interfaces"][interface]["reasoning"]["levels"], expected_levels,
            "{interface}"
        );
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
