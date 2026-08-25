//! Verifies Embeddings analysis, trusted egress, bounded response validation, replay, and cancellation.

mod support;

use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use bytes::Bytes;
use futures_util::{Stream, future::BoxFuture};
use http::{HeaderMap, HeaderValue, Method};
use openbridge::{
    config::parse_bootstrap_config,
    core::{
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        OperationKind,
    },
    ingress::{GatewayState, build_router},
    pipeline::{analyze_embedding_request, plan_embedding_request},
    provider::{PreparedUpstreamRequest, ProviderKind},
    registry::{
        CanonicalModelTask, CanonicalTaskKind, EmbeddingModelProfile, InputModality, ModelConfig,
        ModelLifecycle, PublicModelConfig, RegistryConfig, RouteConfig, RouteMode,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules,
        UpstreamTarget, UpstreamTargetConfig, build_registry,
    },
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use support::{BOOTSTRAP, users_and_credential_pool, users_and_credentials};

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

struct RecordingEmbeddingTransport {
    requests: Mutex<Vec<RecordedEmbeddingRequest>>,
    responses: Mutex<VecDeque<SyntheticEmbeddingResponse>>,
}

struct TimeoutThenSuccessEmbeddingTransport {
    attempts: AtomicUsize,
}

struct PendingEmbeddingRequestTransport {
    attempts: AtomicUsize,
    started: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}

struct PendingEmbeddingResponseTransport {
    attempts: AtomicUsize,
    body_started: Arc<tokio::sync::Notify>,
    body_dropped: Arc<AtomicBool>,
}

struct BackoffEmbeddingTransport {
    attempts: AtomicUsize,
    first_attempt: Arc<tokio::sync::Notify>,
    second_attempt: Arc<tokio::sync::Notify>,
}

#[derive(Clone, Copy)]
enum FixedEmbeddingTransportFailure {
    Timeout,
    InvalidTarget,
}

struct FailingEmbeddingTransport {
    attempts: AtomicUsize,
    failure: FixedEmbeddingTransportFailure,
}

struct UnsafeUpstreamFailureTransport {
    attempts: AtomicUsize,
}

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct PendingEmbeddingBody {
    started: Arc<tokio::sync::Notify>,
    _drop_signal: DropSignal,
}

impl Stream for PendingEmbeddingBody {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Signal that response validation is actively awaiting upstream body bytes.
        self.started.notify_one();
        Poll::Pending
    }
}

struct SyntheticEmbeddingResponse {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    body: Vec<u8>,
}

impl SyntheticEmbeddingResponse {
    /// Builds one synthetic JSON success response without retaining business payloads elsewhere.
    fn json(body: Value) -> Self {
        Self {
            status: StatusCode::OK,
            content_type: Some(HeaderValue::from_static("application/json")),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    /// Builds one raw response for media-type, budget, or JSON-shape rejection cases.
    fn raw(content_type: Option<&'static str>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: StatusCode::OK,
            content_type: content_type.map(HeaderValue::from_static),
            body: body.into(),
        }
    }

    /// Builds one retryable synthetic HTTP response.
    fn status(status: StatusCode) -> Self {
        Self {
            status,
            content_type: Some(HeaderValue::from_static("application/json")),
            body: br#"{"error":{"message":"synthetic upstream failure"}}"#.to_vec(),
        }
    }

    /// Converts the owned synthetic response into the transport contract.
    fn into_upstream(self) -> UpstreamResponse {
        let mut headers = HeaderMap::new();
        if let Some(content_type) = self.content_type {
            headers.insert(CONTENT_TYPE, content_type);
        }
        UpstreamResponse::new(self.status, headers, Body::from(self.body))
    }
}

impl UpstreamTransport for TimeoutThenSuccessEmbeddingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Count the attempt and retain only the synthetic request shape needed for a valid response.
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        let body: Value = serde_json::from_slice(request.body()).unwrap();

        // Fail the first send with a timeout and complete the replay with a valid response.
        Box::pin(async move {
            if attempt == 1 {
                Err(TransportError::Timeout)
            } else {
                Ok(valid_embedding_response(&body).into_upstream())
            }
        })
    }
}

impl UpstreamTransport for PendingEmbeddingRequestTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Mark the current send as started and keep a drop signal inside its pending future.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        let signal = DropSignal(self.dropped.clone());

        // Remain pending until downstream cancellation drops the complete upstream send future.
        Box::pin(async move {
            let _signal = signal;
            std::future::pending::<Result<UpstreamResponse, TransportError>>().await
        })
    }
}

impl UpstreamTransport for PendingEmbeddingResponseTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Return response headers immediately and keep the success body pending before commit.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = Body::from_stream(PendingEmbeddingBody {
            started: self.body_started.clone(),
            _drop_signal: DropSignal(self.body_dropped.clone()),
        });
        Box::pin(async move { Ok(UpstreamResponse::new(StatusCode::OK, headers, body)) })
    }
}

impl UpstreamTransport for BackoffEmbeddingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Distinguish the first retryable response from any forbidden post-cancellation attempt.
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt == 1 {
            self.first_attempt.notify_one();
        } else {
            self.second_attempt.notify_one();
        }

        // Return a retryable status so a replayable request enters the cancellable backoff.
        Box::pin(async {
            Ok(SyntheticEmbeddingResponse::status(StatusCode::SERVICE_UNAVAILABLE).into_upstream())
        })
    }
}

impl UpstreamTransport for FailingEmbeddingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record each bounded attempt and construct only the requested synthetic transport category.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let failure = self.failure;
        Box::pin(async move {
            Err(match failure {
                FixedEmbeddingTransportFailure::Timeout => TransportError::Timeout,
                FixedEmbeddingTransportFailure::InvalidTarget => TransportError::InvalidTarget,
            })
        })
    }
}

impl UpstreamTransport for UnsafeUpstreamFailureTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Count the non-retryable upstream rejection and attach both safe and forbidden headers.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "openai-request-id",
            HeaderValue::from_static("req_synthetic"),
        );
        headers.insert(
            "x-ratelimit-limit-requests",
            HeaderValue::from_static("100"),
        );
        headers.insert("set-cookie", HeaderValue::from_static("private=value"));
        headers.insert(
            "x-internal-target",
            HeaderValue::from_static("private-target"),
        );

        // Return a body carrying every value that the normalized downstream error must discard.
        Box::pin(async move {
            Ok(UpstreamResponse::new(
                StatusCode::BAD_REQUEST,
                headers,
                Body::from(
                    r#"{"error":{"message":"UPSTREAM_BODY_SENTINEL embedding-upstream https://private.invalid private-key"}}"#,
                ),
            ))
        })
    }
}

impl RecordingEmbeddingTransport {
    /// Creates an isolated recorder with optional ordered upstream responses.
    fn new(responses: impl IntoIterator<Item = SyntheticEmbeddingResponse>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
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
        let default_response = valid_embedding_response(&recorded.body);
        self.requests.lock().unwrap().push(recorded);

        // Return the ordered rejection fixture or a valid response derived from the request shape.
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(default_response);
        Box::pin(async move { Ok(response.into_upstream()) })
    }
}

/// Builds a valid response for the existing request/egress cases after response validation lands.
fn valid_embedding_response(request: &Value) -> SyntheticEmbeddingResponse {
    // Derive only the public input count, effective encoding, and dimension from the synthetic request.
    let input = &request["input"];
    let input_count = match input {
        Value::String(_) => 1,
        Value::Array(values) if values.first().is_some_and(Value::is_number) => 1,
        Value::Array(values) => values.len(),
        _ => unreachable!("preflighted test input has one supported shape"),
    };
    let encoding = request["encoding_format"].as_str().unwrap_or("float");
    let dimensions = request["dimensions"].as_u64().unwrap_or(4) as usize;

    // Preserve one deterministic vector per logical input in canonical order.
    let data = (0..input_count)
        .map(|index| {
            let embedding = match encoding {
                "float" => Value::Array(
                    (0..dimensions)
                        .map(|offset| json!((index * dimensions + offset + 1) as f64 / 10.0))
                        .collect(),
                ),
                "base64" if dimensions == 2 => json!("AQIDBAUGBwg="),
                _ => unreachable!("synthetic cases use only supported encoding dimensions"),
            };
            json!({
                "object": "embedding",
                "embedding": embedding,
                "index": index
            })
        })
        .collect::<Vec<_>>();
    SyntheticEmbeddingResponse::json(json!({
        "object": "list",
        "data": data,
        "model": "embedding-upstream",
        "usage": {
            "prompt_tokens": input_count,
            "total_tokens": input_count
        }
    }))
}

fn embedding_capabilities() -> EmbeddingsCapabilities {
    EmbeddingsCapabilities {
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
        tokenizer: Some("synthetic-tokenizer".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Embedding(EmbeddingModelProfile {
            max_input_tokens: Some(8_192),
            input_modalities: Some(vec![InputModality::Text]),
            supported_parameters: PARAMETERS.iter().map(|value| (*value).to_owned()).collect(),
        }),
    });
    definition.upstream_targets.push(UpstreamTargetConfig {
        id: "embedding-target".to_owned(),
        provider_instance: "openai".to_owned(),
        canonical_model: "openai/embedding-test".to_owned(),
        provider_model: "openai/embedding-test".to_owned(),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        timeout_policy: openbridge::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(30)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                OperationKind::EmbeddingsCreate,
                CanonicalTaskKind::Embedding,
            ),
            upstream_model: "embedding-upstream".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(embedding_capabilities()),
            streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
        }],
    });
    definition.routes.push(RouteConfig {
        id: "embedding-route".to_owned(),
        upstream_target: "embedding-target".to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    });
    definition.public_models.push(PublicModelConfig {
        id: "embedding-test".to_owned(),
        created: 1_785_715_200,
        display_name: "Embedding test".to_owned(),
        description: Some("Synthetic Embeddings Public Model.".to_owned()),
        lifecycle: ModelLifecycle::active(),
        reasoning_level_policy: openbridge::registry::ReasoningLevelPolicy::Strict,
        routes: vec!["embedding-route".to_owned()],
    });
    definition
}

fn app() -> (axum::Router, Arc<RecordingEmbeddingTransport>) {
    app_with_bootstrap_and_responses(BOOTSTRAP, [])
}

fn app_with_responses(
    responses: impl IntoIterator<Item = SyntheticEmbeddingResponse>,
) -> (axum::Router, Arc<RecordingEmbeddingTransport>) {
    app_with_bootstrap_and_responses(BOOTSTRAP, responses)
}

fn app_with_credentials_and_responses(
    upstream_secrets: &[&str],
    responses: impl IntoIterator<Item = SyntheticEmbeddingResponse>,
) -> (axum::Router, Arc<RecordingEmbeddingTransport>) {
    // Compile the synthetic registry and inject the requested ordered credential members.
    let bootstrap = parse_bootstrap_config(BOOTSTRAP).unwrap();
    let registry = Arc::new(build_registry(bootstrap, embedding_registry_definition()).unwrap());
    let (users, credentials) =
        users_and_credential_pool(DOWNSTREAM_KEY, &registry, upstream_secrets);
    let transport = Arc::new(RecordingEmbeddingTransport::new(responses));
    let state = GatewayState::new(registry, transport.clone(), users, credentials);
    (build_router(state), transport)
}

fn app_with_bootstrap_and_responses(
    bootstrap_document: &str,
    responses: impl IntoIterator<Item = SyntheticEmbeddingResponse>,
) -> (axum::Router, Arc<RecordingEmbeddingTransport>) {
    // Compile the synthetic mixed-operation registry and inject isolated test credentials/transport.
    let bootstrap = parse_bootstrap_config(bootstrap_document).unwrap();
    let registry = Arc::new(build_registry(bootstrap, embedding_registry_definition()).unwrap());
    let (users, credentials) =
        users_and_credentials(DOWNSTREAM_KEY, &registry, "upstream-test-key");
    let transport = Arc::new(RecordingEmbeddingTransport::new(responses));
    let state = GatewayState::new(registry, transport.clone(), users, credentials);
    (build_router(state), transport)
}

fn app_with_transport_and_credentials(
    bootstrap_document: &str,
    transport: Arc<dyn UpstreamTransport>,
    upstream_secrets: &[&str],
) -> axum::Router {
    // Compile one isolated registry and bind the requested synthetic credential pool.
    let bootstrap = parse_bootstrap_config(bootstrap_document).unwrap();
    let registry = Arc::new(build_registry(bootstrap, embedding_registry_definition()).unwrap());
    let (users, credentials) =
        users_and_credential_pool(DOWNSTREAM_KEY, &registry, upstream_secrets);

    // Build the production router with only the injected transport varying by cancellation case.
    build_router(GatewayState::new(registry, transport, users, credentials))
}

fn embedding_request(body: Value) -> Request<Body> {
    embedding_raw_request(serde_json::to_vec(&body).unwrap(), Some("application/json"))
}

fn embedding_raw_request(body: impl Into<Body>, content_type: Option<&str>) -> Request<Body> {
    // Build one authenticated endpoint request with an optional media type for error cases.
    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("authorization", format!("Bearer {DOWNSTREAM_KEY}"))
        .header("x-upstream-credential", "client-controlled-value");
    if let Some(content_type) = content_type {
        request = request.header(CONTENT_TYPE, content_type);
    }
    request.body(body.into()).unwrap()
}

async fn assert_embedding_error(
    response: axum::response::Response,
    status: StatusCode,
    error_type: &str,
    code: &str,
    param: Option<&str>,
) -> Value {
    // Read the complete small gateway-owned envelope and require the expected HTTP status.
    assert_eq!(response.status(), status);
    let body = to_bytes(response.into_body(), 4_096).await.unwrap();
    let document: Value = serde_json::from_slice(&body).expect("error response must be JSON");

    // Verify exactly the four OpenAI-compatible error fields and their stable classifications.
    let error = document["error"]
        .as_object()
        .expect("error response must contain an object");
    assert_eq!(error.len(), 4);
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(error["type"], error_type);
    assert_eq!(error["code"], code);
    assert_eq!(
        error["param"],
        param.map_or(Value::Null, |value| json!(value))
    );
    document
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
async fn bounded_success_response_projects_model_and_preserves_embedding_values() {
    let float_response = SyntheticEmbeddingResponse::json(json!({
        "id": "emb-bailian-synthetic",
        "object": "list",
        "data": [
            {"object":"embedding","embedding":[0.25,-0.5],"index":0},
            {"object":"embedding","embedding":[1.25,2.5],"index":1}
        ],
        "model": "embedding-upstream",
        "usage": {"prompt_tokens":7,"total_tokens":7}
    }));
    let base64_response = SyntheticEmbeddingResponse::json(json!({
        "object": "list",
        "data": [
            {"object":"embedding","embedding":"AQIDBAUGBwg=","index":0}
        ],
        "model": "embedding-upstream",
        "usage": {"prompt_tokens":3,"total_tokens":3}
    }));
    let (app, transport) = app_with_responses([float_response, base64_response]);

    // Validate and project a float response without changing vector values or input order.
    let response = app
        .clone()
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":["alpha","beta"],
            "encoding_format":"float",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    let actual: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4_096).await.unwrap()).unwrap();
    assert_eq!(actual["object"], "list");
    assert_eq!(actual["model"], "embedding-test");
    assert_eq!(actual["data"][0]["embedding"], json!([0.25, -0.5]));
    assert_eq!(actual["data"][0]["index"], 0);
    assert_eq!(actual["data"][1]["embedding"], json!([1.25, 2.5]));
    assert_eq!(actual["data"][1]["index"], 1);
    assert_eq!(actual["usage"], json!({"prompt_tokens":7,"total_tokens":7}));
    assert!(actual.get("id").is_none());

    // Preserve an already encoded base64 vector byte-for-byte while projecting only the model.
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":"alpha",
            "encoding_format":"base64",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let actual: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4_096).await.unwrap()).unwrap();
    assert_eq!(actual["model"], "embedding-test");
    assert_eq!(actual["data"][0]["embedding"], "AQIDBAUGBwg=");
    assert_eq!(actual["usage"], json!({"prompt_tokens":3,"total_tokens":3}));
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn invalid_or_oversized_success_responses_fail_before_downstream_commit() {
    let valid_body = || {
        json!({
            "object":"list",
            "data":[{"object":"embedding","embedding":[0.25,-0.5],"index":0}],
            "model":"embedding-upstream",
            "usage":{"prompt_tokens":1,"total_tokens":1}
        })
    };
    let invalid_cases = [
        SyntheticEmbeddingResponse::raw(
            Some("text/plain"),
            serde_json::to_vec(&valid_body()).unwrap(),
        ),
        SyntheticEmbeddingResponse::raw(Some("application/json"), b"{".to_vec()),
        SyntheticEmbeddingResponse::json(json!({
            "object":"collection","data":[{"object":"embedding","embedding":[0.25,-0.5],"index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":[0.25,-0.5],"index":0}],
            "model":"different-upstream-model","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[],"model":"embedding-upstream",
            "usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":[0.25,-0.5],"index":1}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"vector","embedding":[0.25,-0.5],"index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":"not-float","index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":[0.25],"index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":[0.25,-0.5],"index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":"one","total_tokens":1}
        })),
        SyntheticEmbeddingResponse::json(json!({
            "object":"list","data":[{"object":"embedding","embedding":[0.25,-0.5],"index":0}],
            "model":"embedding-upstream","usage":{"prompt_tokens":1,"total_tokens":1},
            "internal_target":"must-not-leak"
        })),
    ];

    // Reject every invalid success body after exactly one upstream call without echoing its contents.
    for response in invalid_cases {
        let (app, transport) = app_with_responses([response]);
        let response = app
            .oneshot(embedding_request(json!({
                "model":"embedding-test",
                "input":"alpha",
                "encoding_format":"float",
                "dimensions":2
            })))
            .await
            .unwrap();
        let document = assert_embedding_error(
            response,
            StatusCode::BAD_GATEWAY,
            "server_error",
            "invalid_upstream_response",
            None,
        )
        .await;
        assert!(!document.to_string().contains("must-not-leak"));
        assert_eq!(transport.requests.lock().unwrap().len(), 1);
    }

    // Enforce the response budget before parsing and never truncate or pass through an oversized body.
    let constrained_bootstrap = BOOTSTRAP.replace(
        "max_json_response_body = \"16MiB\"",
        "max_json_response_body = \"512B\"",
    );
    let oversized = SyntheticEmbeddingResponse::raw(
        Some("application/json"),
        format!(r#"{{"sentinel":"{}"}}"#, "x".repeat(600)).into_bytes(),
    );
    let (app, transport) = app_with_bootstrap_and_responses(&constrained_bootstrap, [oversized]);
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":"alpha",
            "encoding_format":"float",
            "dimensions":2
        })))
        .await
        .unwrap();
    let document = assert_embedding_error(
        response,
        StatusCode::BAD_GATEWAY,
        "server_error",
        "invalid_upstream_response",
        None,
    )
    .await;
    assert!(!document.to_string().contains(&"x".repeat(32)));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_base64_length_is_rejected_for_the_effective_dimension() {
    let response = SyntheticEmbeddingResponse::json(json!({
        "object":"list",
        "data":[{"object":"embedding","embedding":"AQ==","index":0}],
        "model":"embedding-upstream",
        "usage":{"prompt_tokens":1,"total_tokens":1}
    }));
    let (app, transport) = app_with_responses([response]);
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":"alpha",
            "encoding_format":"base64",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_embedding_error(
        response,
        StatusCode::BAD_GATEWAY,
        "server_error",
        "invalid_upstream_response",
        None,
    )
    .await;
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn compiler_derived_response_batch_limit_is_enforced_before_egress() {
    let constrained_bootstrap = BOOTSTRAP.replace(
        "max_json_response_body = \"16MiB\"",
        "max_json_response_body = \"400B\"",
    );
    let (app, transport) = app_with_bootstrap_and_responses(&constrained_bootstrap, []);

    // The declared max of two is narrowed to one by the worst-case response serialization bound.
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":["alpha","beta"],
            "encoding_format":"float",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn replayable_rate_limit_rotates_credentials_within_the_single_route() {
    // Arrange one 429 followed by the transport's request-derived valid response.
    let (app, transport) = app_with_credentials_and_responses(
        &["upstream-test-key-a", "upstream-test-key-b"],
        [SyntheticEmbeddingResponse::status(
            StatusCode::TOO_MANY_REQUESTS,
        )],
    );
    let request = embedding_request(json!({
        "model": "embedding-test",
        "input": "replayable input"
    }));

    // Execute the replayable request through the production attempt path.
    let response = app.oneshot(request).await.unwrap();

    // Verify one credential rotation without changing the fixed target or upstream model.
    assert_eq!(response.status(), StatusCode::OK);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].target_id, "embedding-target");
    assert_eq!(requests[1].target_id, "embedding-target");
    assert_eq!(requests[0].body["model"], "embedding-upstream");
    assert_eq!(requests[1].body["model"], "embedding-upstream");
    assert_eq!(requests[0].authorization, "Bearer upstream-test-key-a");
    assert_eq!(requests[1].authorization, "Bearer upstream-test-key-b");
}

#[tokio::test]
async fn replayable_server_failure_retries_once_with_the_same_credential() {
    // Arrange one retryable server response followed by a valid response.
    let (app, transport) = app_with_responses([SyntheticEmbeddingResponse::status(
        StatusCode::SERVICE_UNAVAILABLE,
    )]);
    let request = embedding_request(json!({
        "model": "embedding-test",
        "input": "replayable input"
    }));

    // Execute one candidate-local replay before response commit.
    let response = app.oneshot(request).await.unwrap();

    // Verify the existing finite retry policy retained the same credential and Route.
    assert_eq!(response.status(), StatusCode::OK);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].authorization, requests[1].authorization);
    assert_eq!(requests[0].target_id, requests[1].target_id);
}

#[tokio::test]
async fn replayable_transport_timeout_retries_once_before_success() {
    // Inject a transport that times out once and succeeds only if replay is allowed.
    let transport = Arc::new(TimeoutThenSuccessEmbeddingTransport {
        attempts: AtomicUsize::new(0),
    });
    let app =
        app_with_transport_and_credentials(BOOTSTRAP, transport.clone(), &["upstream-test-key"]);
    let request = embedding_request(json!({
        "model": "embedding-test",
        "input": "replayable input"
    }));

    // Execute the request and require the second transport outcome to complete it.
    let response = app.oneshot(request).await.unwrap();

    // Verify transport failure uses the same two-attempt candidate budget.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retryable_failures_stop_at_the_existing_candidate_attempt_limit() {
    // Queue more retryable responses than one candidate may consume.
    let (app, transport) = app_with_responses([
        SyntheticEmbeddingResponse::status(StatusCode::SERVICE_UNAVAILABLE),
        SyntheticEmbeddingResponse::status(StatusCode::SERVICE_UNAVAILABLE),
        SyntheticEmbeddingResponse::status(StatusCode::SERVICE_UNAVAILABLE),
    ]);
    let request = embedding_request(json!({
        "model": "embedding-test",
        "input": "replayable input"
    }));

    // Execute until the shared candidate-local budget returns the current failure.
    let response = app.oneshot(request).await.unwrap();

    // Verify no third attempt or cross-Route fallback occurs.
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn valid_body_over_replay_budget_executes_exactly_one_attempt() {
    // Build a valid request above replay eligibility but below the downstream request hard limit.
    let large_input = "r".repeat(262_200);
    let (app, transport) = app_with_responses([SyntheticEmbeddingResponse::status(
        StatusCode::SERVICE_UNAVAILABLE,
    )]);
    let request = embedding_request(json!({
        "model": "embedding-test",
        "input": large_input
    }));

    // Execute the large request without turning replay optimization into a local rejection.
    let response = app.oneshot(request).await.unwrap();

    // Verify the first retryable response is returned and no second egress occurs.
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn downstream_cancellation_drops_a_pending_embedding_send() {
    // Build an upstream send that remains pending before response headers.
    let started = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(PendingEmbeddingRequestTransport {
        attempts: AtomicUsize::new(0),
        started: started.clone(),
        dropped: dropped.clone(),
    });
    let app =
        app_with_transport_and_credentials(BOOTSTRAP, transport.clone(), &["upstream-test-key"]);
    let task = tokio::spawn(app.oneshot(embedding_request(json!({
        "model": "embedding-test",
        "input": "cancel pending send"
    }))));
    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .expect("the first upstream send must start");

    // Abort the downstream task and wait until its upstream future is destroyed.
    task.abort();
    let error = task.await.unwrap_err();

    // Verify cancellation drops the current send and cannot start another attempt.
    assert!(error.is_cancelled());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn downstream_cancellation_drops_a_pending_embedding_success_body() {
    // Build a successful upstream response whose body never produces bytes before validation.
    let body_started = Arc::new(tokio::sync::Notify::new());
    let body_dropped = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(PendingEmbeddingResponseTransport {
        attempts: AtomicUsize::new(0),
        body_started: body_started.clone(),
        body_dropped: body_dropped.clone(),
    });
    let app =
        app_with_transport_and_credentials(BOOTSTRAP, transport.clone(), &["upstream-test-key"]);
    let task = tokio::spawn(app.oneshot(embedding_request(json!({
        "model": "embedding-test",
        "input": "cancel pending receive"
    }))));
    tokio::time::timeout(Duration::from_secs(1), body_started.notified())
        .await
        .expect("the upstream success body must be polled");

    // Abort before the bounded success validator can commit a downstream response.
    task.abort();
    let error = task.await.unwrap_err();

    // Verify the pending response source is dropped and no replay begins after cancellation.
    assert!(error.is_cancelled());
    assert!(body_dropped.load(Ordering::SeqCst));
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn downstream_cancellation_during_backoff_prevents_embedding_replay() {
    // Build a retryable first response with explicit attempt notifications.
    let first_attempt = Arc::new(tokio::sync::Notify::new());
    let second_attempt = Arc::new(tokio::sync::Notify::new());
    let transport = Arc::new(BackoffEmbeddingTransport {
        attempts: AtomicUsize::new(0),
        first_attempt: first_attempt.clone(),
        second_attempt: second_attempt.clone(),
    });
    let app =
        app_with_transport_and_credentials(BOOTSTRAP, transport.clone(), &["upstream-test-key"]);
    let task = tokio::spawn(app.oneshot(embedding_request(json!({
        "model": "embedding-test",
        "input": "cancel backoff"
    }))));
    tokio::time::timeout(Duration::from_secs(1), first_attempt.notified())
        .await
        .expect("the first upstream attempt must start");

    // Abort while the handler is waiting on its cancellable retry timer.
    task.abort();
    let error = task.await.unwrap_err();
    let second = tokio::time::timeout(Duration::from_millis(100), second_attempt.notified()).await;

    // Verify cancellation owns the timer and no detached replay survives the request.
    assert!(error.is_cancelled());
    assert!(second.is_err());
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
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
async fn embedding_error_matrix_fixes_local_status_type_code_param_and_zero_egress() {
    let (app, transport) = app();
    let cases = vec![
        (
            embedding_raw_request(b"{".to_vec(), Some("application/json")),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            None,
        ),
        (
            embedding_raw_request(b"[]".to_vec(), Some("application/json")),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            None,
        ),
        (
            embedding_request(json!({"input":"alpha"})),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("model"),
        ),
        (
            embedding_request(json!({"model":"","input":"alpha"})),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("model"),
        ),
        (
            embedding_request(json!({"model":"embedding-test"})),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("input"),
        ),
        (
            embedding_request(json!({"model":"embedding-test","input":null})),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("input"),
        ),
        (
            embedding_request(json!({"model":"embedding-test","input":["alpha",1]})),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("input"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":"alpha","encoding_format":"hex"
            })),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("encoding_format"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":"alpha","dimensions":0
            })),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("dimensions"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":"alpha","user":false
            })),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("user"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":"alpha","unknown":true
            })),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            None,
        ),
        (
            embedding_request(json!({"model":"missing-test","input":"alpha"})),
            StatusCode::NOT_FOUND,
            "model_not_found",
            Some("model"),
        ),
        (
            embedding_request(json!({"model":"generation-test","input":"alpha"})),
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            Some("model"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":["a","b","c"]
            })),
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            Some("input"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":[1,2,3,4]
            })),
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            Some("input"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":[[1,2,3],[4,5]]
            })),
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            Some("input"),
        ),
        (
            embedding_request(json!({
                "model":"embedding-test","input":"alpha","dimensions":3
            })),
            StatusCode::BAD_REQUEST,
            "unsupported_model_capability",
            Some("dimensions"),
        ),
    ];

    // Exercise representative syntax, model, and fixed-capability failures independently.
    for (request, status, code, param) in cases {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_embedding_error(response, status, "invalid_request_error", code, param).await;
        assert!(transport.requests.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn embedding_media_and_request_limits_use_exact_zero_egress_errors() {
    // Reject the JSON-only endpoint media contract with its endpoint-specific stable code.
    let (app, transport) = app();
    let response = app
        .oneshot(embedding_raw_request(
            br#"{"model":"embedding-test","input":"alpha"}"#.to_vec(),
            Some("text/plain"),
        ))
        .await
        .unwrap();
    assert_embedding_error(
        response,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_request_error",
        "unsupported_media_type",
        None,
    )
    .await;
    assert!(transport.requests.lock().unwrap().is_empty());

    // Lower request and replay limits together and exceed the hard request limit before authentication parsing.
    let constrained_bootstrap = BOOTSTRAP
        .replace("max_request_body = \"1MiB\"", "max_request_body = \"128B\"")
        .replace("max_replay_body = \"256KiB\"", "max_replay_body = \"128B\"");
    let (app, transport) = app_with_bootstrap_and_responses(&constrained_bootstrap, []);
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test",
            "input":"x".repeat(256)
        })))
        .await
        .unwrap();
    assert_embedding_error(
        response,
        StatusCode::PAYLOAD_TOO_LARGE,
        "invalid_request_error",
        "request_too_large",
        None,
    )
    .await;
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn embedding_transport_errors_use_server_envelopes_and_bounded_attempts() {
    let cases = [
        (
            FixedEmbeddingTransportFailure::Timeout,
            StatusCode::GATEWAY_TIMEOUT,
            "upstream_timeout",
            2,
        ),
        (
            FixedEmbeddingTransportFailure::InvalidTarget,
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            1,
        ),
    ];

    // Exhaust retryable timeout and non-retryable transport categories through isolated transports.
    for (failure, status, code, expected_attempts) in cases {
        let transport = Arc::new(FailingEmbeddingTransport {
            attempts: AtomicUsize::new(0),
            failure,
        });
        let app = app_with_transport_and_credentials(
            BOOTSTRAP,
            transport.clone(),
            &["upstream-test-key"],
        );
        let response = app
            .oneshot(embedding_request(json!({
                "model":"embedding-test","input":"transport error"
            })))
            .await
            .unwrap();
        assert_embedding_error(response, status, "server_error", code, None).await;
        assert_eq!(transport.attempts.load(Ordering::SeqCst), expected_attempts);
    }
}

#[tokio::test]
async fn embedding_upstream_http_errors_discard_body_and_private_headers() {
    // Return one non-retryable Provider status carrying private diagnostics and topology sentinels.
    let transport = Arc::new(UnsafeUpstreamFailureTransport {
        attempts: AtomicUsize::new(0),
    });
    let app =
        app_with_transport_and_credentials(BOOTSTRAP, transport.clone(), &["upstream-test-key"]);
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-test","input":"safe downstream input"
        })))
        .await
        .unwrap();

    // Preserve only the safe status and allowlisted request/rate-limit headers.
    assert_eq!(response.headers()["openai-request-id"], "req_synthetic");
    assert_eq!(response.headers()["x-ratelimit-limit-requests"], "100");
    assert!(response.headers().get("set-cookie").is_none());
    assert!(response.headers().get("x-internal-target").is_none());
    let document = assert_embedding_error(
        response,
        StatusCode::BAD_REQUEST,
        "server_error",
        "upstream_error",
        None,
    )
    .await;

    // Verify no upstream body, model, endpoint, or credential sentinel reaches the client.
    let serialized = document.to_string();
    for sentinel in [
        "UPSTREAM_BODY_SENTINEL",
        "embedding-upstream",
        "private.invalid",
        "private-key",
        "private-target",
    ] {
        assert!(!serialized.contains(sentinel));
    }
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
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
