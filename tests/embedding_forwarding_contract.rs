//! Verifies Embeddings request analysis, fixed-interface preflight, and trusted Native egress.

mod support;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Method};
use openbridge::{
    config::parse_bootstrap_config,
    core::{
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        OperationKind,
    },
    ingress::{GatewayState, build_router},
    pipeline::{analyze_embedding_request, plan_embedding_request},
    provider::{PreparedUpstreamRequest, ProviderKind},
    providers::compiled_config,
    registry::{
        InputModality, ModelConfig, ModelContextLength, ModelLifecycle, ModelMode, OutputModality,
        PublicModelConfig, ReasoningSupport, RegistryConfig, RouteConfig, RouteMode, StateAffinity,
        TransportKind, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTarget, UpstreamTargetConfig, build_registry,
    },
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use support::{BOOTSTRAP, users_and_credentials};

const INPUT_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::String,
    EmbeddingInputForm::StringArray,
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const ENCODINGS: &[EmbeddingEncoding] = &[EmbeddingEncoding::Float, EmbeddingEncoding::Base64];
const DIMENSIONS: &[u32] = &[2, 4];
const LOCALLY_COUNTED_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const PARAMETERS: &[&str] = &["dimensions", "encoding_format", "user"];
const DOWNSTREAM_KEY: &str = "downstream-token-0000000000000000";

#[derive(Debug)]
struct RecordedEmbeddingRequest {
    target_id: String,
    provider: ProviderKind,
    method: Method,
    path: String,
    authorization: String,
    untrusted_credential_header: Option<String>,
    body: Value,
}

#[derive(Default)]
struct RecordingEmbeddingTransport {
    requests: Mutex<Vec<RecordedEmbeddingRequest>>,
}

impl UpstreamTransport for RecordingEmbeddingTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record only synthetic test values and the trusted adapter output.
        let recorded = RecordedEmbeddingRequest {
            target_id: target.id().to_owned(),
            provider: target.kind(),
            method: request.method().clone(),
            path: request.relative_uri().path().to_owned(),
            authorization: headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned(),
            untrusted_credential_header: headers
                .get("x-upstream-credential")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        };
        self.requests.lock().unwrap().push(recorded);

        // Return a synthetic body; bounded validation and model projection belong to stage 3.
        Box::pin(async {
            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                http::HeaderValue::from_static("application/json"),
            );
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    r#"{"object":"list","data":[],"model":"embedding-upstream","usage":{"prompt_tokens":0,"total_tokens":0}}"#,
                ),
            ))
        })
    }
}

fn embedding_capabilities() -> EmbeddingsCapabilities {
    EmbeddingsCapabilities {
        enabled: true,
        input_forms: INPUT_FORMS,
        default_encoding: EmbeddingEncoding::Float,
        allowed_encodings: Some(ENCODINGS),
        default_dimensions: 4,
        allowed_dimensions: Some(EmbeddingDimensionDomain::Values { values: DIMENSIONS }),
        max_inputs: 2,
        max_tokens_per_input: Some(3),
        max_total_tokens: Some(4),
        locally_counted_input_forms: LOCALLY_COUNTED_FORMS,
        supported_parameters: PARAMETERS,
    }
}

fn embedding_registry_definition() -> RegistryConfig {
    // Keep an existing generation-only Public Model so interface mismatch can be distinguished from unknown model.
    let mut definition = support::definition(
        "embedding-forwarding",
        "generation-test",
        "generation-upstream",
    );
    definition.models.push(ModelConfig {
        id: "openai/embedding-test".to_owned(),
        name: "Embedding test model".to_owned(),
        description: Some("Synthetic Embeddings contract model.".to_owned()),
        context_length: ModelContextLength::new(None, Some(8_192), None),
        mode: Some(ModelMode::Embedding),
        input_modalities: Some(vec![InputModality::Text]),
        output_modalities: Some(vec![OutputModality::Embedding]),
        tokenizer: Some("synthetic-tokenizer".to_owned()),
        knowledge_cutoff: None,
        supported_parameters: PARAMETERS.iter().map(|value| (*value).to_owned()).collect(),
        reasoning: ReasoningSupport::Unsupported,
        reasoning_levels: Vec::new(),
    });
    definition.upstream_targets.push(UpstreamTargetConfig {
        id: "embedding-target".to_owned(),
        provider: ProviderKind::OpenAi,
        model: "openai/embedding-test".to_owned(),
        base_url: "https://api.openai.com".to_owned(),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(30),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            id: "embeddings".to_owned(),
            operation: OperationKind::EmbeddingsCreate,
            upstream_model: "embedding-upstream".to_owned(),
            endpoint_profile: "public-api".to_owned(),
            transport: TransportKind::HttpJson,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(embedding_capabilities()),
            state_affinity: StateAffinity::Unbound,
        }],
    });
    definition.routes.push(RouteConfig {
        id: "embedding-route".to_owned(),
        upstream_target: "embedding-target".to_owned(),
        upstream_api: "embeddings".to_owned(),
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    });
    definition.public_models.push(PublicModelConfig {
        id: "embedding-test".to_owned(),
        created: 1_785_715_200,
        display_name: "Embedding test".to_owned(),
        description: Some("Synthetic Embeddings Public Model.".to_owned()),
        lifecycle: ModelLifecycle::active(),
        routes: vec!["embedding-route".to_owned()],
    });
    definition
}

fn app() -> (axum::Router, Arc<RecordingEmbeddingTransport>) {
    // Compile the synthetic mixed-operation registry and inject isolated test credentials/transport.
    let bootstrap = parse_bootstrap_config(BOOTSTRAP).unwrap();
    let registry = Arc::new(build_registry(bootstrap, embedding_registry_definition()).unwrap());
    let (users, credentials) =
        users_and_credentials(DOWNSTREAM_KEY, &registry, "upstream-test-key");
    let transport = Arc::new(RecordingEmbeddingTransport::default());
    let state = GatewayState::new(registry, transport.clone(), users, credentials);
    (build_router(state), transport)
}

fn embedding_request(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("authorization", format!("Bearer {DOWNSTREAM_KEY}"))
        .header(CONTENT_TYPE, "application/json")
        .header("x-upstream-credential", "client-controlled-value")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn four_embedding_input_forms_use_one_fixed_native_egress_contract() {
    let (app, transport) = app();
    let cases = [
        json!("alpha"),
        json!(["alpha", "beta"]),
        json!([1, 2, 3]),
        json!([[1, 2], [3, 4]]),
    ];

    // Send all four input forms with the strongest explicitly permitted optional fields.
    for input in &cases {
        let response = app
            .clone()
            .oneshot(embedding_request(json!({
                "model": "embedding-test",
                "input": input,
                "encoding_format": "base64",
                "dimensions": 2,
                "user": "synthetic-user"
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 4_096).await.unwrap();
    }

    // Verify trusted path/auth/model rewrites while preserving every client-owned Embeddings field.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), cases.len());
    for (request, input) in requests.iter().zip(cases) {
        assert_eq!(request.target_id, "embedding-target");
        assert_eq!(request.provider, ProviderKind::OpenAi);
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.path, "/v1/embeddings");
        assert_eq!(request.authorization, "Bearer upstream-test-key");
        assert!(request.untrusted_credential_header.is_none());
        assert_eq!(request.body["model"], "embedding-upstream");
        assert_eq!(request.body["input"], input);
        assert_eq!(request.body["encoding_format"], "base64");
        assert_eq!(request.body["dimensions"], 2);
        assert_eq!(request.body["user"], "synthetic-user");
    }
}

#[tokio::test]
async fn omitted_fields_resolve_interface_defaults_without_adapter_invention() {
    // Resolve omitted options from the fixed interface before any Provider adapter runs.
    let body = Bytes::from_static(br#"{"model":"embedding-test","input":"alpha"}"#);
    let requirements = analyze_embedding_request(&body).unwrap();
    let registry = build_registry(
        parse_bootstrap_config(BOOTSTRAP).unwrap(),
        embedding_registry_definition(),
    )
    .unwrap();
    let plan = plan_embedding_request(&registry, &requirements, body).unwrap();
    assert_eq!(plan.input_count(), 1);
    assert_eq!(plan.encoding(), EmbeddingEncoding::Float);
    assert_eq!(plan.dimensions(), 4);

    // Keep omitted optional fields absent on the wire while rewriting only the trusted model.
    let (app, transport) = app();
    let response = app
        .oneshot(embedding_request(json!({
            "model": "embedding-test",
            "input": "alpha"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["model"], "embedding-upstream");
    assert!(requests[0].body.get("encoding_format").is_none());
    assert!(requests[0].body.get("dimensions").is_none());
    assert!(requests[0].body.get("user").is_none());
}

#[tokio::test]
async fn embedding_local_rejections_make_zero_upstream_calls() {
    let (app, transport) = app();
    let invalid_requests = [
        (
            json!({"model":"embedding-test","input":""}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":null}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":["alpha",1]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[[1],2]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[-1,2]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[1.5,2]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[4294967296_u64]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[1,2,3,4]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":[[1,2,3],[4,5]]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":["a","b","c"]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":"alpha","encoding_format":"hex"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":"alpha","dimensions":3}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":"alpha","stream":false}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":"alpha","messages":[]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"embedding-test","input":"alpha","upstream_url":"https://attacker.invalid"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"generation-test","input":"alpha"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"model":"missing-test","input":"alpha"}),
            StatusCode::NOT_FOUND,
        ),
    ];

    // Reject malformed shapes, unsupported contract values, unknown fields, and wrong interfaces locally.
    for (body, expected_status) in invalid_requests {
        let response = app.clone().oneshot(embedding_request(body)).await.unwrap();
        assert_eq!(response.status(), expected_status);
    }
    assert!(transport.requests.lock().unwrap().is_empty());

    // Preserve endpoint authentication before any parser or egress work.
    let unauthenticated = Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"embedding-test","input":"alpha"}"#))
        .unwrap();
    let response = app.oneshot(unauthenticated).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn embedding_endpoint_is_authenticated_and_json_only_before_egress() {
    let (app, transport) = app();

    // Reject authenticated requests with a missing or incompatible JSON media type.
    for content_type in [None, Some("text/plain")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/embeddings")
            .header("authorization", format!("Bearer {DOWNSTREAM_KEY}"));
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(r#"{"model":"embedding-test","input":"alpha"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn checked_in_catalog_remains_generation_only_during_synthetic_egress_stage() {
    // Keep Embeddings callable only through this stage's synthetic registry fixture.
    let definition = compiled_config();
    assert!(
        definition
            .models
            .iter()
            .all(|model| model.mode != Some(ModelMode::Embedding))
    );
    assert!(definition.upstream_targets.iter().all(|target| {
        target
            .upstream_apis
            .iter()
            .all(|api| api.operation != OperationKind::EmbeddingsCreate)
    }));
    assert!(
        definition
            .routes
            .iter()
            .all(|route| route.downstream_operation != OperationKind::EmbeddingsCreate)
    );
    assert!(
        definition
            .public_models
            .iter()
            .all(|model| model.id != "embedding-primary")
    );
}
