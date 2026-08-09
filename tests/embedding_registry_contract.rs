//! Verifies that checked-in Embeddings interfaces are privately routed and directly plannable.

use bytes::Bytes;
use openbridge::{
    config::parse_bootstrap_config,
    core::{EmbeddingEncoding, OperationKind},
    pipeline::{EmbeddingRequestError, analyze_embedding_request, plan_embedding_request},
    providers::build_compiled_registry,
    registry::RouteMode,
};
use serde_json::json;

#[test]
fn checked_in_embedding_interfaces_are_private_and_directly_plannable() {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("the checked-in bootstrap must remain valid");
    let registry = build_compiled_registry(bootstrap)
        .expect("the checked-in Embeddings registrations must compile");
    let cases = [
        (
            "text-embedding-3-small",
            br#"{"model":"text-embedding-3-small","input":["alpha","beta"],"encoding_format":"base64","user":"synthetic-user"}"#.as_slice(),
            EmbeddingEncoding::Base64,
            1_536,
        ),
        (
            "qwen3.7-text-embedding",
            br#"{"model":"qwen3.7-text-embedding","input":["alpha","beta"],"encoding_format":"float","dimensions":512}"#.as_slice(),
            EmbeddingEncoding::Float,
            512,
        ),
    ];

    // Exercise every checked-in Embeddings Public Model through its published interface.
    for (model_id, request, expected_encoding, expected_dimensions) in cases {
        let public_model = registry
            .public_model(model_id)
            .expect("the Embeddings Public Model must be discoverable");
        let [route_id] = public_model.routes() else {
            panic!("{model_id} must expose exactly one Embeddings Route");
        };
        let route = registry
            .route(route_id)
            .expect("the published Route must resolve");
        assert_eq!(route.mode(), RouteMode::Native, "{model_id}");
        assert_eq!(
            route.upstream_operation(),
            OperationKind::EmbeddingsCreate,
            "{model_id}"
        );
        assert_eq!(
            route.downstream_operation(),
            OperationKind::EmbeddingsCreate,
            "{model_id}"
        );

        let body = Bytes::copy_from_slice(request);
        let requirements = analyze_embedding_request(&body).unwrap();
        let plan = plan_embedding_request(&registry, &requirements, body).unwrap();
        assert_eq!(plan.candidate().route_id(), route_id, "{model_id}");
        assert_eq!(
            plan.candidate().upstream_operation(),
            OperationKind::EmbeddingsCreate,
            "{model_id}"
        );
        assert_eq!(plan.input_count(), 2, "{model_id}");
        assert_eq!(plan.encoding(), expected_encoding, "{model_id}");
        assert_eq!(plan.dimensions(), expected_dimensions, "{model_id}");

        // Keep the Models DTO useful to clients while hiding private Route and Target identities.
        let info = serde_json::to_value(public_model.info()).unwrap();
        assert_eq!(info["capabilities"]["tasks"], json!(["embedding"]));
        assert!(info["interfaces"]["embeddings"].is_object());
        assert_eq!(info["interfaces"]["chat_completions"], json!(null));
        assert_eq!(info["interfaces"]["responses"], json!(null));
        let serialized = info.to_string();
        assert!(!serialized.contains(route_id), "{model_id}");
        assert!(
            !serialized.contains(plan.candidate().upstream_target_id()),
            "{model_id}"
        );
    }

    // Keep unsupported OpenAI dimensions closed while Qwen's registered dimension succeeds above.
    let body = Bytes::from_static(
        br#"{"model":"text-embedding-3-small","input":"alpha","dimensions":512}"#,
    );
    let requirements = analyze_embedding_request(&body).unwrap();
    assert!(matches!(
        plan_embedding_request(&registry, &requirements, body),
        Err(EmbeddingRequestError::UnsupportedModelCapability {
            param: "dimensions"
        })
    ));
}

#[test]
fn qwen_checked_in_dimension_domain_matches_the_bailian_contract() {
    // Compile the checked-in registry and assert the exact downstream dimension projection.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("the checked-in bootstrap must remain valid");
    let registry = build_compiled_registry(bootstrap)
        .expect("the checked-in Embeddings registrations must compile");
    let public_model = registry
        .public_model("qwen3.7-text-embedding")
        .expect("the Qwen Embeddings Public Model must be discoverable");
    let info = serde_json::to_value(public_model.info()).unwrap();
    assert_eq!(
        info["interfaces"]["embeddings"]["dimensions"],
        json!({
            "default": 1024,
            "allowed": {
                "kind": "values",
                "values": [256, 512, 768, 1024, 1536, 2048, 2560]
            }
        })
    );

    // Accept every declared dimension through request preflight.
    for dimensions in [256, 512, 768, 1_024, 1_536, 2_048, 2_560] {
        let body = Bytes::from(
            serde_json::to_vec(&json!({
                "model": "qwen3.7-text-embedding",
                "input": "alpha",
                "dimensions": dimensions
            }))
            .unwrap(),
        );
        let requirements = analyze_embedding_request(&body).unwrap();
        let plan = plan_embedding_request(&registry, &requirements, body).unwrap();
        assert_eq!(plan.dimensions(), dimensions);
    }

    // Reject dimensions that the former catalog exposed but Bailian does not support.
    for dimensions in [64, 128] {
        let body = Bytes::from(
            serde_json::to_vec(&json!({
                "model": "qwen3.7-text-embedding",
                "input": "alpha",
                "dimensions": dimensions
            }))
            .unwrap(),
        );
        let requirements = analyze_embedding_request(&body).unwrap();
        assert!(matches!(
            plan_embedding_request(&registry, &requirements, body),
            Err(EmbeddingRequestError::UnsupportedModelCapability {
                param: "dimensions"
            })
        ));
    }
}
