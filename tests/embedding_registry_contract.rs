//! Verifies the checked-in OpenAI Embeddings model, target, Route, and Public Model contract.

use bytes::Bytes;
use openbridge::{
    config::parse_bootstrap_config,
    core::{EmbeddingEncoding, EmbeddingInputForm, OperationKind},
    pipeline::{EmbeddingRequestError, analyze_embedding_request, plan_embedding_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_config},
    registry::{
        InputModality, ModelMode, OutputModality, ReasoningSupport, RouteMode, StateAffinity,
        TransportKind, UpstreamApiCapabilities,
    },
};
use serde_json::json;

const CANONICAL_MODEL: &str = "openai/text-embedding-3-small";
const TARGET: &str = "openai-text-embedding-3-small";
const ROUTE: &str = "text-embedding-3-small-openai-embeddings";

#[test]
fn checked_in_catalog_registers_one_dedicated_openai_embedding_route() {
    let definition = compiled_config();

    // Verify provider-independent model facts and the deliberately conservative parameter surface.
    let model = definition
        .models
        .iter()
        .find(|model| model.id == CANONICAL_MODEL)
        .expect("the canonical embedding model must be compiled");
    assert_eq!(model.mode, Some(ModelMode::Embedding));
    assert_eq!(model.input_modalities, Some(vec![InputModality::Text]));
    assert_eq!(
        model.output_modalities,
        Some(vec![OutputModality::Embedding])
    );
    assert_eq!(model.context_length.input_tokens(), Some(8_192));
    assert_eq!(model.supported_parameters, ["encoding_format", "user"]);
    assert_eq!(model.reasoning, ReasoningSupport::Unsupported);

    // Verify the dedicated trusted target has only the bounded JSON Embeddings API.
    let target = definition
        .upstream_targets
        .iter()
        .find(|target| target.id == TARGET)
        .expect("the dedicated embedding target must be compiled");
    assert_ne!(target.id, "openai-main");
    assert_eq!(target.provider, ProviderKind::OpenAi);
    assert_eq!(target.model, CANONICAL_MODEL);
    assert_eq!(target.base_url, "https://api.openai.com");
    assert_eq!(target.credential_pool, "openai-primary");
    assert!(target.enabled);
    let [api] = target.upstream_apis.as_slice() else {
        panic!("the embedding target must contain exactly one Upstream API");
    };
    assert_eq!(api.id, "embeddings");
    assert_eq!(api.operation, OperationKind::EmbeddingsCreate);
    assert_eq!(api.upstream_model, "text-embedding-3-small");
    assert_eq!(api.endpoint_profile, "public-api");
    assert_eq!(api.transport, TransportKind::HttpJson);
    assert_eq!(api.state_affinity, StateAffinity::Unbound);
    let UpstreamApiCapabilities::Embeddings(capabilities) = api.capabilities else {
        panic!("the dedicated API must expose only Embeddings capabilities");
    };
    assert!(capabilities.enabled);
    assert_eq!(
        capabilities.input_forms,
        [
            EmbeddingInputForm::String,
            EmbeddingInputForm::StringArray,
            EmbeddingInputForm::TokenArray,
            EmbeddingInputForm::TokenArrayArray,
        ]
    );
    assert_eq!(capabilities.default_encoding, EmbeddingEncoding::Float);
    assert_eq!(
        capabilities.allowed_encodings,
        Some([EmbeddingEncoding::Float, EmbeddingEncoding::Base64].as_slice())
    );
    assert_eq!(capabilities.default_dimensions, 1_536);
    assert_eq!(capabilities.allowed_dimensions, None);
    assert_eq!(capabilities.max_inputs, 2_048);
    assert_eq!(capabilities.max_tokens_per_input, Some(8_192));
    assert_eq!(capabilities.max_total_tokens, Some(300_000));
    assert_eq!(
        capabilities.locally_counted_input_forms,
        [
            EmbeddingInputForm::TokenArray,
            EmbeddingInputForm::TokenArrayArray,
        ]
    );
    assert_eq!(
        capabilities.supported_parameters,
        ["encoding_format", "user"]
    );

    // Verify one Native Route is the complete candidate set for the downstream Public Model.
    let route = definition
        .routes
        .iter()
        .find(|route| route.id == ROUTE)
        .expect("the embedding Route must be compiled");
    assert_eq!(route.upstream_target, TARGET);
    assert_eq!(route.upstream_api, "embeddings");
    assert_eq!(route.downstream_operation, OperationKind::EmbeddingsCreate);
    assert_eq!(route.mode, RouteMode::Native);
    let public_model = definition
        .public_models
        .iter()
        .find(|model| model.id == "text-embedding-3-small")
        .expect("the embedding Public Model must be compiled");
    assert_eq!(public_model.routes, [ROUTE]);
}

#[test]
fn checked_in_embedding_interface_is_discoverable_and_directly_plannable() {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("the checked-in bootstrap must remain valid");
    let registry = build_compiled_registry(bootstrap)
        .expect("the checked-in Embeddings registration must compile");

    // Verify the Models projection exposes only the fixed Embeddings contract without topology.
    let public_model = registry
        .public_model("text-embedding-3-small")
        .expect("the embedding Public Model must be available");
    assert_eq!(public_model.routes(), [ROUTE]);
    let info = serde_json::to_value(public_model.info()).unwrap();
    assert_eq!(info["id"], "text-embedding-3-small");
    assert_eq!(info["capabilities"]["tasks"], json!(["embedding"]));
    assert_eq!(info["interfaces"]["chat_completions"], json!(null));
    assert_eq!(info["interfaces"]["responses"], json!(null));
    let interface = &info["interfaces"]["embeddings"];
    assert_eq!(
        interface["input_forms"],
        json!(["string", "string_array", "token_array", "token_array_array"])
    );
    assert_eq!(
        interface["encoding"],
        json!({"default": "float", "allowed": ["float", "base64"]})
    );
    assert_eq!(
        interface["dimensions"],
        json!({"default": 1536, "allowed": null})
    );
    assert_eq!(interface["limits"]["max_tokens_per_input"], 8_192);
    assert_eq!(interface["limits"]["max_total_tokens"], 300_000);
    let effective_max_inputs = interface["limits"]["max_inputs"].as_u64().unwrap();
    assert!((1..2_048).contains(&effective_max_inputs));
    assert_eq!(
        interface["limits"]["locally_counted_input_forms"],
        json!(["token_array", "token_array_array"])
    );
    assert_eq!(
        interface["supported_parameters"],
        json!(["encoding_format", "user"])
    );
    let serialized = info.to_string();
    assert!(!serialized.contains(TARGET));
    assert!(!serialized.contains(ROUTE));

    // Plan one request from the same published contract and retain only the trusted candidate.
    let body = Bytes::from_static(
        br#"{"model":"text-embedding-3-small","input":["alpha","beta"],"encoding_format":"base64","user":"synthetic-user"}"#,
    );
    let requirements = analyze_embedding_request(&body).unwrap();
    let plan = plan_embedding_request(&registry, &requirements, body).unwrap();
    assert_eq!(plan.candidate().route_id(), ROUTE);
    assert_eq!(plan.candidate().upstream_target_id(), TARGET);
    assert_eq!(plan.candidate().upstream_api_id(), "embeddings");
    assert_eq!(plan.input_count(), 2);
    assert_eq!(plan.encoding(), EmbeddingEncoding::Base64);
    assert_eq!(plan.dimensions(), 1_536);

    // Keep explicit dimensions closed until an evidence-backed domain is registered.
    let dimensions = Bytes::from_static(
        br#"{"model":"text-embedding-3-small","input":"alpha","dimensions":512}"#,
    );
    let requirements = analyze_embedding_request(&dimensions).unwrap();
    assert!(matches!(
        plan_embedding_request(&registry, &requirements, dimensions),
        Err(EmbeddingRequestError::UnsupportedModelCapability {
            param: "dimensions"
        })
    ));
}
