//! Contract tests for the typed Embeddings registry definition and Models projection.

mod support;

use openbridge::{
    core::{
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        OperationKind,
    },
    provider::ProviderKind,
    registry::{
        IgnorableGenerationParameter, ModelMode, OutputModality, RegistryError, RouteConfig,
        RouteMode, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        build_registry,
    },
};
use serde_json::json;

use support::{BOOTSTRAP, bootstrap, definition};

const INPUT_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::String,
    EmbeddingInputForm::StringArray,
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const ENCODINGS: &[EmbeddingEncoding] = &[EmbeddingEncoding::Float, EmbeddingEncoding::Base64];
const DIMENSIONS: &[u32] = &[256, 512, 1_024];
const LOCALLY_COUNTED_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const PARAMETERS: &[&str] = &["dimensions", "encoding_format", "user"];

fn embedding_capabilities() -> EmbeddingsCapabilities {
    EmbeddingsCapabilities {
        enabled: true,
        input_forms: INPUT_FORMS,
        default_encoding: EmbeddingEncoding::Float,
        allowed_encodings: Some(ENCODINGS),
        default_dimensions: 1_024,
        allowed_dimensions: Some(EmbeddingDimensionDomain::Values { values: DIMENSIONS }),
        max_inputs: 32,
        max_tokens_per_input: Some(8_192),
        max_total_tokens: Some(262_144),
        locally_counted_input_forms: LOCALLY_COUNTED_FORMS,
        supported_parameters: PARAMETERS,
    }
}

fn embedding_definition() -> openbridge::registry::RegistryConfig {
    // Reuse the synthetic trusted target while replacing its generation model and API surface.
    let mut definition = definition("embedding-contract", "embedding-test", "embedding-upstream");
    let model = &mut definition.models[0];
    model.mode = Some(ModelMode::Embedding);
    model.output_modalities = Some(vec![OutputModality::Embedding]);
    model.supported_parameters = PARAMETERS.iter().map(|value| (*value).to_owned()).collect();

    // Bind one JSON-only Native Embeddings API and Route to the synthetic Public Model.
    definition.upstream_targets[0].upstream_apis = vec![UpstreamApiConfig {
        upstream_model: "embedding-upstream".to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::Embeddings(embedding_capabilities()),
        streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
        state_affinity: openbridge::registry::StateAffinity::Unbound,
    }];
    definition.routes = vec![RouteConfig {
        id: "public-embeddings".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    }];
    definition.public_models[0].routes = vec!["public-embeddings".to_owned()];
    definition
}

#[test]
fn embedding_definition_compiles_into_the_typed_models_interface() {
    // Compile the synthetic Embeddings-only registry without publishing a checked-in model.
    let registry = build_registry(bootstrap(BOOTSTRAP), embedding_definition())
        .expect("the typed Embeddings definition should compile");
    let public_model = registry.public_model("embedding-test").unwrap();

    // Project the exact callable contract without exposing Provider or Route topology.
    let actual = serde_json::to_value(public_model.info()).unwrap();
    assert_eq!(actual["capabilities"]["tasks"], json!(["embedding"]));
    assert_eq!(
        actual["capabilities"]["modalities"]["input"],
        json!(["text"])
    );
    assert_eq!(
        actual["capabilities"]["modalities"]["output"],
        json!(["embedding"])
    );
    assert_eq!(actual["interfaces"]["chat_completions"], json!(null));
    assert_eq!(actual["interfaces"]["responses"], json!(null));
    assert_eq!(
        actual["interfaces"]["embeddings"],
        json!({
            "input_forms": ["string", "string_array", "token_array", "token_array_array"],
            "encoding": {
                "default": "float",
                "allowed": ["float", "base64"]
            },
            "dimensions": {
                "default": 1024,
                "allowed": {
                    "kind": "values",
                    "values": [256, 512, 1024]
                }
            },
            "limits": {
                "max_inputs": 32,
                "max_tokens_per_input": 8192,
                "max_total_tokens": 262144,
                "locally_counted_input_forms": ["token_array", "token_array_array"]
            },
            "supported_parameters": ["dimensions", "encoding_format", "user"]
        })
    );
    assert!(actual.to_string().find("openai-main").is_none());
    assert!(actual.to_string().find("embedding-upstream").is_none());

    // Keep runtime operation identity independent from the generation-only protocol enum.
    let target = registry.upstream_target("openai-main").unwrap();
    assert_eq!(
        target
            .upstream_api(OperationKind::EmbeddingsCreate)
            .unwrap()
            .operation(),
        OperationKind::EmbeddingsCreate
    );
    assert_eq!(
        registry
            .route("public-embeddings")
            .unwrap()
            .downstream_operation(),
        OperationKind::EmbeddingsCreate
    );
    assert_eq!(target.kind(), ProviderKind::OpenAi);
}

#[test]
fn embedding_compiler_derives_batch_limit_from_the_json_response_budget() {
    // Force the worst-case 1,024-dimension float response to fit once but not twice.
    let one_vector_budget = BOOTSTRAP.replace(
        "max_json_response_body_bytes = 16777216",
        "max_json_response_body_bytes = 40000",
    );
    let registry = build_registry(bootstrap(&one_vector_budget), embedding_definition()).unwrap();
    let actual =
        serde_json::to_value(registry.public_model("embedding-test").unwrap().info()).unwrap();
    assert_eq!(
        actual["interfaces"]["embeddings"]["limits"]["max_inputs"],
        1
    );

    // Reject startup when even one valid worst-case vector cannot fit the configured budget.
    let impossible_budget = BOOTSTRAP.replace(
        "max_json_response_body_bytes = 16777216",
        "max_json_response_body_bytes = 1024",
    );
    assert!(matches!(
        build_registry(bootstrap(&impossible_budget), embedding_definition()),
        Err(RegistryError::EmbeddingResponseBudgetTooSmall { public_model })
            if public_model == "embedding-test"
    ));
}

#[test]
fn embedding_compiler_rejects_invalid_closed_contracts() {
    // Exercise each independently owned closed set, default, domain, limit, and parameter boundary.
    let cases: &[fn(&mut EmbeddingsCapabilities)] = &[
        |capabilities| capabilities.input_forms = &[],
        |capabilities| {
            capabilities.input_forms = &[EmbeddingInputForm::String, EmbeddingInputForm::String];
        },
        |capabilities| capabilities.allowed_encodings = Some(&[]),
        |capabilities| {
            capabilities.default_encoding = EmbeddingEncoding::Base64;
            capabilities.allowed_encodings = Some(&[EmbeddingEncoding::Float]);
        },
        |capabilities| capabilities.default_dimensions = 0,
        |capabilities| {
            capabilities.allowed_dimensions = Some(EmbeddingDimensionDomain::Range {
                minimum: 512,
                maximum: 256,
            });
        },
        |capabilities| {
            capabilities.allowed_dimensions = Some(EmbeddingDimensionDomain::Values {
                values: &[256, 512],
            });
        },
        |capabilities| capabilities.max_inputs = 0,
        |capabilities| {
            capabilities.max_tokens_per_input = Some(300_000);
            capabilities.max_total_tokens = Some(200_000);
        },
        |capabilities| {
            capabilities.locally_counted_input_forms = &[EmbeddingInputForm::String];
        },
        |capabilities| capabilities.supported_parameters = &["unknown_parameter"],
        |capabilities| capabilities.supported_parameters = &["dimensions", "user"],
    ];
    for mutate in cases {
        let mut definition = embedding_definition();
        let UpstreamApiCapabilities::Embeddings(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[0].capabilities
        else {
            panic!("expected Embeddings capabilities");
        };
        mutate(capabilities);
        assert!(matches!(
            build_registry(bootstrap(BOOTSTRAP), definition),
            Err(RegistryError::InvalidEmbeddingsCapabilities { .. })
        ));
    }
}

#[test]
fn embedding_compiler_derives_operation_and_enforces_model_task_identity() {
    // Reject a generation canonical model and a Native Route whose operations differ.
    let mut model = embedding_definition();
    model.models[0].mode = Some(ModelMode::Chat);
    model.models[0].supported_parameters.clear();
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), model),
        Err(RegistryError::EmbeddingsModelTaskMismatch { .. })
    ));
    let mut route = embedding_definition();
    route.routes[0].downstream_operation = OperationKind::Responses;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), route),
        Err(RegistryError::NativeRouteOperationMismatch { .. })
    ));
}

#[test]
fn embedding_api_rejects_generation_parameter_ignore_rules() {
    let mut definition = embedding_definition();
    definition.models[0]
        .supported_parameters
        .push("temperature".to_owned());
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];

    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), definition),
        Err(RegistryError::InconsistentUpstreamApiModelRules { .. })
    ));
}

#[test]
fn embedding_compiler_rejects_bridge_and_multiple_candidates() {
    // Embeddings has no Protocol Bridge representation.
    let mut bridged = embedding_definition();
    bridged.routes[0].mode = RouteMode::Bridged;
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), bridged),
        Err(RegistryError::InvalidBridgedRouteOperations { .. })
    ));

    // The current focus permits exactly one executable Embeddings candidate per Public Model.
    let mut multiple = embedding_definition();
    let mut second = multiple.routes[0].clone();
    second.id = "public-embeddings-second".to_owned();
    multiple.routes.push(second);
    multiple.public_models[0]
        .routes
        .push("public-embeddings-second".to_owned());
    assert!(matches!(
        build_registry(bootstrap(BOOTSTRAP), multiple),
        Err(RegistryError::MultipleEmbeddingsCandidates { .. })
    ));
}
