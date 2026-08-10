//! Verifies request-terminal observation, usage extraction, and exclusion of sensitive business content from diagnostics.

mod support;

use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Request, StatusCode, header::CONTENT_TYPE},
};
use futures_util::{StreamExt, future::BoxFuture, stream};
use openbridge::{
    config::parse_bootstrap_config,
    core::{
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        OperationKind,
    },
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::{
        CanonicalModelTask, EmbeddingModelProfile, InputModality, ModelConfig, ModelLifecycle,
        PublicModelConfig, RouteConfig, RouteMode, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTarget, UpstreamTargetConfig, build_registry,
    },
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use support::metrics::{GatewayMetricsSnapshot, TestMetrics};
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;

struct RetryThenUsageTransport {
    attempts: AtomicUsize,
}

impl UpstreamTransport for RetryThenUsageTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if attempt == 0 {
                return Ok(UpstreamResponse::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    headers,
                    Body::from(r#"{"error":{"message":"retry"}}"#),
                ));
            }
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    r#"{"id":"chatcmpl-observed","choices":[],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}"#,
                ),
            ))
        })
    }
}

struct PendingStreamTransport;

struct EofWithoutTerminalTransport;

struct PendingRequestTransport {
    started: tokio::sync::Notify,
}

struct FailedTerminalTransport;

struct FailedJsonTerminalTransport;

struct ProviderMetricsJsonTransport;

struct ProviderMetricsStreamingTransport;

struct JsonResponseAboveRequestLimitTransport;

struct ContentLoggingTransport;

struct EmbeddingMetricsTransport {
    attempts: AtomicUsize,
}

struct ReplayLimitEmbeddingTransport {
    attempts: AtomicUsize,
}

const OBSERVED_EMBEDDING_FORMS: &[EmbeddingInputForm] =
    &[EmbeddingInputForm::String, EmbeddingInputForm::TokenArray];
const OBSERVED_EMBEDDING_ENCODINGS: &[EmbeddingEncoding] =
    &[EmbeddingEncoding::Float, EmbeddingEncoding::Base64];
const OBSERVED_EMBEDDING_DIMENSIONS: &[u32] = &[2];
const OBSERVED_EMBEDDING_PARAMETERS: &[&str] = &["dimensions", "encoding_format", "user"];

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

struct BufferWriter(LogBuffer);

impl Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogBuffer {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        BufferWriter(self.clone())
    }
}

impl UpstreamTransport for PendingStreamTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let body = Body::from_stream(stream::once(async {
                Ok::<_, std::io::Error>(bytes::Bytes::from_static(
                    b"data: {\"id\":\"chatcmpl-observed\",\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
                ))
            })
                .chain(stream::pending()));
            Ok(UpstreamResponse::new(StatusCode::OK, headers, body))
        })
    }
}

impl UpstreamTransport for EofWithoutTerminalTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    "data: {\"id\":\"chatcmpl-observed\",\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
                ),
            ))
        })
    }
}

impl UpstreamTransport for PendingRequestTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.started.notify_one();
        Box::pin(std::future::pending())
    }
}

impl UpstreamTransport for FailedTerminalTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n",
                ),
            ))
        })
    }
}

impl UpstreamTransport for FailedJsonTerminalTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(r#"{"id":"resp_failed","object":"response","status":"failed"}"#),
            ))
        })
    }
}

impl UpstreamTransport for ProviderMetricsJsonTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    r#"{"id":"chatcmpl-provider-metrics","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":6,"total_tokens":16,"prompt_tokens_details":{"cached_tokens":4}}}"#,
                ),
            ))
        })
    }
}

impl UpstreamTransport for ProviderMetricsStreamingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let chunks = stream::iter(vec![
                Ok::<_, std::io::Error>(bytes::Bytes::from_static(
                    b"data: {\"id\":\"chatcmpl-provider-stream\",\"choices\":[{\"delta\":{\"reasoning_content\":\"reasoning\"}}]}\n\n",
                )),
                Ok(bytes::Bytes::from_static(
                    b"data: {\"choices\":[{\"delta\":{}}],\"usage\":{\"prompt_tokens\":8,\"completion_tokens\":5,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\n",
                )),
                Ok(bytes::Bytes::from_static(b"data: [DONE]\n\n")),
            ]);
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from_stream(chunks),
            ))
        })
    }
}

impl UpstreamTransport for JsonResponseAboveRequestLimitTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Place usage after a body section larger than the independent downstream request limit.
        let body = format!(
            r#"{{"padding":"{}","usage":{{"prompt_tokens":13,"completion_tokens":5,"total_tokens":18}}}}"#,
            "x".repeat(1_100_000)
        );
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(body),
            ))
        })
    }
}

impl UpstreamTransport for EmbeddingMetricsTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Select a deterministic valid response from only the requested public encoding.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let request: Value = serde_json::from_slice(request.body()).unwrap();
        let base64 = request["encoding_format"] == "base64";
        let response = if base64 {
            json!({
                "object":"list",
                "data":[{"object":"embedding","embedding":"U0VDUkVUUyE=","index":0}],
                "model":"embedding-observed-upstream",
                "usage":{"prompt_tokens":3,"total_tokens":3}
            })
        } else {
            json!({
                "object":"list",
                "data":[{"object":"embedding","embedding":[12345.678,-98765.432],"index":0}],
                "model":"embedding-observed-upstream",
                "usage":{"prompt_tokens":5,"total_tokens":5}
            })
        };

        // Return the complete body without logging request or vector values.
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(serde_json::to_vec(&response).unwrap()),
            ))
        })
    }
}

impl UpstreamTransport for ReplayLimitEmbeddingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Count every actual egress and always return a status that would retry a smaller body.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::SERVICE_UNAVAILABLE,
                headers,
                Body::from(r#"{"error":{"message":"retryable synthetic failure"}}"#),
            ))
        })
    }
}

impl UpstreamTransport for ContentLoggingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Return synthetic header and body sentinels so the local HTTP boundary can be asserted exactly.
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(
                "openai-request-id",
                HeaderValue::from_static("RESPONSE_HEADER_SENTINEL_7D41"),
            );
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(
                    r#"{"id":"chatcmpl-log","choices":[{"message":{"role":"assistant","content":"RESPONSE_BODY_SENTINEL_6B32"}}]}"#,
                ),
            ))
        })
    }
}

fn app_with_transport(transport: Arc<dyn UpstreamTransport>) -> (axum::Router, TestMetrics) {
    let registry = support::registry("observability-test", "code-primary", "test-model");
    let (users, credentials) = support::users_and_credentials(
        "downstream-test-token-00000000000",
        &registry,
        "upstream-test-token",
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials)
        .with_metrics(metrics.instruments());
    (build_router(state), metrics)
}

fn app_with_transport_and_bootstrap(
    transport: Arc<dyn UpstreamTransport>,
    bootstrap: &str,
) -> (axum::Router, TestMetrics) {
    // Compile the ordinary synthetic registry under the caller-selected startup logging policy.
    let registry = build_registry(
        parse_bootstrap_config(bootstrap).unwrap(),
        support::definition("observability-test", "code-primary", "test-model"),
    )
    .unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-test-token-00000000000",
        &registry,
        "upstream-test-token",
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials)
        .with_metrics(metrics.instruments());
    (build_router(state), metrics)
}

fn embedding_observability_app(
    transport: Arc<dyn UpstreamTransport>,
) -> (axum::Router, TestMetrics) {
    // Extend the ordinary synthetic registry with one independent Embeddings model and target.
    let mut definition = support::definition(
        "embedding-observability",
        "generation-observed",
        "generation-observed-upstream",
    );
    definition.models.push(ModelConfig {
        id: "openai/embedding-observed".to_owned(),
        name: "Observed embedding model".to_owned(),
        description: Some("Synthetic observability contract model.".to_owned()),
        tokenizer: Some("synthetic-tokenizer".to_owned()),
        knowledge_cutoff: None,
        task: CanonicalModelTask::Embedding(EmbeddingModelProfile {
            max_input_tokens: Some(8_192),
            input_modalities: Some(vec![InputModality::Text]),
            supported_parameters: OBSERVED_EMBEDDING_PARAMETERS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        }),
    });
    definition.upstream_targets.push(UpstreamTargetConfig {
        id: "embedding-observed-target".to_owned(),
        provider_instance: "openai".to_owned(),
        canonical_model: "openai/embedding-observed".to_owned(),
        provider_model: "openai/embedding-observed".to_owned(),
        credential_pool: "openai-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(30),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: "embedding-observed-upstream".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(EmbeddingsCapabilities {
                input_forms: OBSERVED_EMBEDDING_FORMS,
                default_encoding: EmbeddingEncoding::Float,
                allowed_encodings: Some(OBSERVED_EMBEDDING_ENCODINGS),
                default_dimensions: 2,
                allowed_dimensions: Some(EmbeddingDimensionDomain::Values {
                    values: OBSERVED_EMBEDDING_DIMENSIONS,
                }),
                max_inputs: 1,
                max_tokens_per_input: Some(8),
                max_total_tokens: Some(8),
                locally_counted_input_forms: &[EmbeddingInputForm::TokenArray],
                supported_parameters: OBSERVED_EMBEDDING_PARAMETERS,
            }),
            streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
        }],
    });
    definition.routes.push(RouteConfig {
        id: "embedding-observed-route".to_owned(),
        upstream_target: "embedding-observed-target".to_owned(),
        upstream_operation: OperationKind::EmbeddingsCreate,
        downstream_operation: OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    });
    definition.public_models.push(PublicModelConfig {
        id: "embedding-observed".to_owned(),
        created: 1_785_715_200,
        display_name: "Observed embeddings".to_owned(),
        description: Some("Synthetic Embeddings observability model.".to_owned()),
        lifecycle: ModelLifecycle::active(),
        reasoning_level_policy: openbridge::registry::ReasoningLevelPolicy::Strict,
        routes: vec!["embedding-observed-route".to_owned()],
    });

    // Compile the mixed registry and bind only synthetic user/upstream credentials.
    let registry = build_registry(
        parse_bootstrap_config(support::BOOTSTRAP).unwrap(),
        definition,
    )
    .unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-test-token-00000000000",
        &registry,
        "upstream-test-token",
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials)
        .with_metrics(metrics.instruments());
    (build_router(state), metrics)
}

async fn serve_loopback(app: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    // Bind a separate random port to each test to avoid shared network state during parallel execution.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("loopback server should run");
    });
    (format!("http://{address}"), server)
}

async fn wait_for_request_terminal(metrics: &TestMetrics) -> GatewayMetricsSnapshot {
    // Use bounded polling for the server terminal state without scheduling-dependent sleeps.
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let snapshot = metrics.snapshot();
            let terminal_count = snapshot.requests_completed
                + snapshot.requests_http_failed
                + snapshot.requests_failed
                + snapshot.requests_cancelled;
            if terminal_count > 0 {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("request observation should reach a terminal state")
}

fn request(body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", "Bearer downstream-test-token-00000000000")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn responses_request(body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header("authorization", "Bearer downstream-test-token-00000000000")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn embedding_request(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("authorization", "Bearer downstream-test-token-00000000000")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn real_http_known_length_response_is_not_misclassified_as_cancelled() {
    // Start a real Axum/Hyper loopback to cover transport body-drop paths that an in-memory oneshot does not trigger.
    let (app, metrics) = app_with_transport(Arc::new(ProviderMetricsJsonTransport));
    let (base_url, server) = serve_loopback(app).await;

    // Read the known-length model-list response completely through a real HTTP client.
    let response = reqwest::Client::new()
        .get(format!("{base_url}/v1/models"))
        .header("authorization", "Bearer downstream-test-token-00000000000")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.bytes().await.unwrap().is_empty());

    // Wait for the server to commit one terminal state and distinguish normal completion from false cancellation.
    let snapshot = wait_for_request_terminal(&metrics).await;
    server.abort();
    let _ = server.await;

    assert_eq!(snapshot.requests_completed, 1);
    assert_eq!(snapshot.requests_cancelled, 0);
}

#[tokio::test]
async fn real_http_native_json_completes_both_provider_and_downstream_observers() {
    // Use Native JSON passthrough with Provider and downstream body observers.
    let (app, metrics) = app_with_transport(Arc::new(ProviderMetricsJsonTransport));
    let (base_url, server) = serve_loopback(app).await;
    let response = reqwest::Client::new()
        .post(format!("{base_url}/v1/chat/completions"))
        .header("authorization", "Bearer downstream-test-token-00000000000")
        .header(CONTENT_TYPE, "application/json")
        .body(r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.bytes().await.unwrap().is_empty());

    // Both observers must finish on the last frame, and usage may be submitted only once.
    let snapshot = wait_for_request_terminal(&metrics).await;
    server.abort();
    let _ = server.await;
    assert_eq!(snapshot.requests_completed, 1);
    assert_eq!(snapshot.requests_cancelled, 0);
    assert_eq!(snapshot.usage_observations, 1);

    let provider_snapshots = metrics.provider_snapshots();
    assert_eq!(provider_snapshots.len(), 1);
    assert_eq!(provider_snapshots[0].attempts_completed, 1);
    assert_eq!(provider_snapshots[0].attempts_cancelled, 0);
    assert_eq!(provider_snapshots[0].usage_observations, 1);
}

#[tokio::test]
async fn completed_request_records_attempt_retry_and_confirmed_usage_once() {
    let (app, metrics) = app_with_transport(Arc::new(RetryThenUsageTransport {
        attempts: AtomicUsize::new(0),
    }));

    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_started, 1);
    assert_eq!(snapshot.requests_completed, 1);
    assert_eq!(snapshot.upstream_attempts, 2);
    assert_eq!(snapshot.upstream_http_failures, 1);
    assert_eq!(snapshot.upstream_transport_failures, 0);
    assert_eq!(snapshot.upstream_retries, 1);
    assert_eq!(snapshot.credential_rotations, 0);
    assert_eq!(snapshot.route_fallbacks, 0);
    assert_eq!(snapshot.usage_observations, 1);
    assert_eq!(snapshot.input_tokens, 11);
    assert_eq!(snapshot.output_tokens, 7);
    assert_eq!(snapshot.total_tokens, 18);

    let provider_snapshot = &metrics.provider_snapshots()[0];
    assert_eq!(provider_snapshot.attempts_started, 2);
    assert_eq!(provider_snapshot.attempts_completed, 1);
    assert_eq!(provider_snapshot.attempts_http_failed, 1);
}

#[tokio::test]
async fn bootstrap_switches_emit_complete_local_http_boundaries_with_sensitive_headers_redacted() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let bootstrap = format!(
        "{}\n[logging]\nrequest_headers = true\nrequest_body = true\nresponse_headers = true\nresponse_body = true\n",
        support::BOOTSTRAP
    );
    let (app, _) = app_with_transport_and_bootstrap(Arc::new(ContentLoggingTransport), &bootstrap);

    // Complete one request so both transparent body observers reach EOF and emit one snapshot each.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header(
                    "authorization",
                    "Bearer downstream-test-token-00000000000",
                )
                .header(CONTENT_TYPE, "application/json")
                .header("x-request-debug", "REQUEST_HEADER_SENTINEL_4A19")
                .body(Body::from(
                    r#"{"model":"code-primary","messages":[{"role":"user","content":"REQUEST_BODY_SENTINEL_5C20"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4_096).await.unwrap();

    // Preserve opted-in safe values and both bodies while keeping authentication material absent.
    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(output.contains("downstream_request_headers"));
    assert!(output.contains("REQUEST_HEADER_SENTINEL_4A19"));
    assert!(output.contains("downstream_request_body"));
    assert!(output.contains("REQUEST_BODY_SENTINEL_5C20"));
    let response_headers = output
        .lines()
        .find(|line| line.contains("downstream_response_headers"))
        .expect("response header snapshot must be emitted");
    assert!(response_headers.contains("RESPONSE_HEADER_SENTINEL_7D41"));
    assert!(response_headers.contains("x-request-id"));
    assert!(output.contains("downstream_response_body"));
    assert!(output.contains("RESPONSE_BODY_SENTINEL_6B32"));
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("downstream-test-token-00000000000"));
}

#[tokio::test]
async fn local_http_logging_switches_do_not_enable_adjacent_dimensions() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let bootstrap = format!(
        "{}\n[logging]\nrequest_headers = true\nrequest_body = false\nresponse_headers = false\nresponse_body = true\n",
        support::BOOTSTRAP
    );
    let (app, _) = app_with_transport_and_bootstrap(Arc::new(ContentLoggingTransport), &bootstrap);

    // Exercise one mixed policy so the runtime wiring cannot couple adjacent header/body switches.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header(
                    "authorization",
                    "Bearer downstream-test-token-00000000000",
                )
                .header(CONTENT_TYPE, "application/json")
                .header("x-request-debug", "REQUEST_HEADER_SENTINEL_4A19")
                .body(Body::from(
                    r#"{"model":"code-primary","messages":[{"role":"user","content":"REQUEST_BODY_SENTINEL_5C20"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let _ = to_bytes(response.into_body(), 4_096).await.unwrap();

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(output.contains("downstream_request_headers"));
    assert!(output.contains("REQUEST_HEADER_SENTINEL_4A19"));
    assert!(!output.contains("downstream_request_body"));
    assert!(!output.contains("REQUEST_BODY_SENTINEL_5C20"));
    assert!(!output.contains("downstream_response_headers"));
    assert!(output.contains("downstream_response_body"));
    assert!(output.contains("RESPONSE_BODY_SENTINEL_6B32"));
}

#[tokio::test]
async fn json_observation_uses_response_budget_not_downstream_request_limit() {
    let (app, metrics) = app_with_transport(Arc::new(JsonResponseAboveRequestLimitTransport));

    // Read a response larger than the 1 MiB request limit but below the 16 MiB response budget.
    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 2_000_000).await.unwrap();
    assert!(body.len() > 1_048_576);

    // Capture usage located after the request-limit boundary exactly once.
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.usage_observations, 1);
    assert_eq!(snapshot.input_tokens, 13);
    assert_eq!(snapshot.output_tokens, 5);
    assert_eq!(snapshot.total_tokens, 18);
}

#[tokio::test]
async fn non_streaming_provider_snapshot_records_only_observable_generation_latency() {
    let (app, metrics) = app_with_transport(Arc::new(ProviderMetricsJsonTransport));

    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let snapshots = metrics.provider_snapshots();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.key.provider, "openai");
    assert_eq!(snapshot.key.public_model, "code-primary");
    assert_eq!(snapshot.key.upstream_operation, "chat_completions");
    assert_eq!(snapshot.key.operation, "chat_completions");
    assert_eq!(snapshot.key.route_mode, "native");
    assert!(!snapshot.key.streaming);
    assert_eq!(snapshot.attempts_started, 1);
    assert_eq!(snapshot.attempts_completed, 1);
    assert_eq!(snapshot.attempts_http_failed, 0);
    assert_eq!(snapshot.usage_observations, 1);
    assert_eq!(snapshot.input_token_observations, 1);
    assert_eq!(snapshot.output_token_observations, 1);
    assert_eq!(snapshot.total_token_observations, 1);
    assert_eq!(snapshot.input_tokens, 10);
    assert_eq!(snapshot.output_tokens, 6);
    assert_eq!(snapshot.total_tokens, 16);
    assert_eq!(snapshot.cached_input_tokens, 4);
    assert_eq!(snapshot.cache_observations, 1);
    assert_eq!(snapshot.cache_read_observations, 1);
    assert_eq!(snapshot.cache_hit_requests, 1);
    assert_eq!(snapshot.upstream_first_byte_ms.count, 1);
    assert_eq!(snapshot.upstream_ttft_ms.count, 0);
    assert_eq!(snapshot.gateway_ttft_ms.count, 1);
    assert_eq!(snapshot.duration_ms.count, 1);
    assert_eq!(snapshot.generation_duration_ms.count, 0);
    assert_eq!(snapshot.output_speed.count, 0);
}

#[tokio::test]
async fn embeddings_use_operation_usage_without_output_or_sensitive_telemetry() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let transport = Arc::new(EmbeddingMetricsTransport {
        attempts: AtomicUsize::new(0),
    });
    let (app, metrics) = embedding_observability_app(transport.clone());

    // Complete one text/base64 request carrying text and user sentinels.
    let response = app
        .clone()
        .oneshot(embedding_request(json!({
            "model":"embedding-observed",
            "input":"TEXT_INPUT_SENTINEL_6F3C",
            "encoding_format":"base64",
            "dimensions":2,
            "user":"USER_SENTINEL_91A7"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4_096).await.unwrap();

    // Complete one token/float request carrying token and vector-number sentinels.
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-observed",
            "input":[42424242,31313131],
            "encoding_format":"float",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4_096).await.unwrap();

    // Verify gateway usage counts input/total only for two completed Embeddings requests.
    let gateway = metrics.snapshot();
    assert_eq!(gateway.requests_started, 2);
    assert_eq!(gateway.requests_completed, 2);
    assert_eq!(gateway.upstream_attempts, 2);
    assert_eq!(gateway.usage_observations, 2);
    assert_eq!(gateway.input_tokens, 8);
    assert_eq!(gateway.output_tokens, 0);
    assert_eq!(gateway.total_tokens, 8);

    // Require one operation-keyed Provider snapshot with no generation-only samples.
    let snapshots = metrics.provider_snapshots();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.key.upstream_operation, "embeddings_create");
    assert_eq!(snapshot.key.operation, "embeddings_create");
    assert_eq!(snapshot.key.route_mode, "native");
    assert!(!snapshot.key.streaming);
    assert_eq!(snapshot.attempts_started, 2);
    assert_eq!(snapshot.attempts_completed, 2);
    assert_eq!(snapshot.usage_observations, 2);
    assert_eq!(snapshot.input_token_observations, 2);
    assert_eq!(snapshot.output_token_observations, 0);
    assert_eq!(snapshot.total_token_observations, 2);
    assert_eq!(snapshot.input_tokens, 8);
    assert_eq!(snapshot.output_tokens, 0);
    assert_eq!(snapshot.total_tokens, 8);
    assert_eq!(snapshot.upstream_ttft_ms.count, 0);
    assert_eq!(snapshot.gateway_ttft_ms.count, 0);
    assert_eq!(snapshot.generation_duration_ms.count, 0);
    assert_eq!(snapshot.output_speed.count, 0);
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 2);

    // Prove the exported schema has one operation field and no body/vector sentinels.
    let exported = serde_json::to_value(&snapshots).unwrap();
    assert_eq!(exported[0]["key"]["operation"], "embeddings_create");
    assert!(exported[0]["key"].get("protocol").is_none());
    let diagnostics = format!(
        "{}\n{}\n{:?}",
        exported,
        String::from_utf8(logs.0.lock().unwrap().clone()).unwrap(),
        gateway
    );
    assert!(diagnostics.contains("embeddings_create"));
    for sentinel in [
        "TEXT_INPUT_SENTINEL_6F3C",
        "USER_SENTINEL_91A7",
        "42424242",
        "31313131",
        "U0VDUkVUUyE=",
        "12345.678",
        "-98765.432",
    ] {
        assert!(!diagnostics.contains(sentinel));
    }
}

#[tokio::test]
async fn embedding_body_over_replay_limit_records_exactly_one_attempt() {
    // Build a retryable transport and a valid request between replay and request hard limits.
    let transport = Arc::new(ReplayLimitEmbeddingTransport {
        attempts: AtomicUsize::new(0),
    });
    let (app, metrics) = embedding_observability_app(transport.clone());
    let response = app
        .oneshot(embedding_request(json!({
            "model":"embedding-observed",
            "input":"r".repeat(262_200),
            "encoding_format":"float",
            "dimensions":2
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let _ = to_bytes(response.into_body(), 4_096).await.unwrap();

    // Verify replay ineligibility is visible only as one attempt, never a new client rejection.
    let gateway = metrics.snapshot();
    assert_eq!(gateway.requests_http_failed, 1);
    assert_eq!(gateway.upstream_attempts, 1);
    assert_eq!(gateway.upstream_http_failures, 1);
    assert_eq!(gateway.upstream_retries, 0);
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
    let snapshots = metrics.provider_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].key.operation, "embeddings_create");
    assert_eq!(snapshots[0].attempts_started, 1);
    assert_eq!(snapshots[0].attempts_http_failed, 1);
}

#[tokio::test]
async fn provider_snapshot_starts_ttft_on_reasoning_stream_output() {
    let (app, metrics) = app_with_transport(Arc::new(ProviderMetricsStreamingTransport));

    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","stream":true,"messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let snapshots = metrics.provider_snapshots();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert!(snapshot.key.streaming);
    assert_eq!(snapshot.attempts_completed, 1);
    assert_eq!(snapshot.upstream_first_byte_ms.count, 1);
    assert_eq!(snapshot.upstream_ttft_ms.count, 1);
    assert_eq!(snapshot.gateway_ttft_ms.count, 1);
    assert_eq!(snapshot.usage_observations, 1);
    assert_eq!(snapshot.input_token_observations, 1);
    assert_eq!(snapshot.output_token_observations, 1);
    assert_eq!(snapshot.total_token_observations, 1);
    assert_eq!(snapshot.input_tokens, 8);
    assert_eq!(snapshot.output_tokens, 5);
    assert_eq!(snapshot.cached_input_tokens, 2);
    assert_eq!(snapshot.cache_read_observations, 1);
    assert_eq!(snapshot.cache_hit_requests, 1);
}

#[tokio::test]
async fn dropping_an_unfinished_stream_records_cancellation_not_completion() {
    let (app, metrics) = app_with_transport(Arc::new(PendingStreamTransport));
    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","stream":true,"messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(metrics.snapshot().requests_completed, 0);

    drop(response);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_completed, 0);
    assert_eq!(snapshot.requests_cancelled, 1);
    assert_eq!(snapshot.upstream_attempts, 1);
    assert_eq!(metrics.provider_snapshots()[0].attempts_cancelled, 1);
}

#[tokio::test]
async fn eof_without_sse_terminal_records_stream_failure() {
    let (app, metrics) = app_with_transport(Arc::new(EofWithoutTerminalTransport));
    let response = app
        .oneshot(request(
            r#"{"model":"code-primary","stream":true,"messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .await
        .unwrap();

    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(std::str::from_utf8(&body).unwrap().contains("hello"));

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_completed, 0);
    assert_eq!(snapshot.requests_failed, 1);
    assert_eq!(snapshot.requests_cancelled, 0);
}

#[tokio::test]
async fn cancelling_before_response_headers_still_records_one_terminal() {
    let transport = Arc::new(PendingRequestTransport {
        started: tokio::sync::Notify::new(),
    });
    let (app, metrics) = app_with_transport(transport.clone());
    let task = tokio::spawn(app.oneshot(request(
        r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}"#,
    )));
    transport.started.notified().await;

    task.abort();
    let _ = task.await;
    tokio::task::yield_now().await;

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_started, 1);
    assert_eq!(snapshot.requests_cancelled, 1);
    assert_eq!(snapshot.upstream_attempts, 1);
    assert_eq!(snapshot.requests_completed, 0);
    assert_eq!(metrics.provider_snapshots()[0].attempts_cancelled, 1);
}

#[tokio::test]
async fn failed_sse_terminal_is_not_counted_as_a_successful_request() {
    let (app, metrics) = app_with_transport(Arc::new(FailedTerminalTransport));
    let response = app
        .oneshot(responses_request(
            r#"{"model":"code-primary","stream":true,"input":"hello"}"#,
        ))
        .await
        .unwrap();
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_completed, 0);
    assert_eq!(snapshot.requests_failed, 1);
    let provider = &metrics.provider_snapshots()[0];
    assert_eq!(provider.attempts_stream_failed, 1);
    assert_eq!(provider.gateway_ttft_ms.count, 0);
}

#[tokio::test]
async fn failed_json_terminal_is_not_counted_as_a_successful_request() {
    let (app, metrics) = app_with_transport(Arc::new(FailedJsonTerminalTransport));
    let response = app
        .oneshot(responses_request(
            r#"{"model":"code-primary","input":"hello"}"#,
        ))
        .await
        .unwrap();
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.requests_completed, 0);
    assert_eq!(snapshot.requests_failed, 1);
    let provider = &metrics.provider_snapshots()[0];
    assert_eq!(provider.attempts_stream_failed, 1);
    assert_eq!(provider.gateway_ttft_ms.count, 0);
}
