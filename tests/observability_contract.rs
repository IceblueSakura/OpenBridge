//! Verifies request-terminal observation, usage extraction, and exclusion of sensitive business content from diagnostics.

mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Request, StatusCode, header::CONTENT_TYPE},
};
use futures_util::{StreamExt, future::BoxFuture, stream};
use openbridge::{
    ingress::{GatewayState, build_router},
    observability::{GatewayMetrics, GatewayMetricsSnapshot},
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use tower::ServiceExt;

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
                    b"data: {\"id\":\"chatcmpl-provider-stream\",\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
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

fn app_with_transport(transport: Arc<dyn UpstreamTransport>) -> (axum::Router, GatewayMetrics) {
    let registry = support::registry("observability-test", "code-primary", "test-model");
    let (users, credentials) = support::users_and_credentials(
        "downstream-test-token-00000000000",
        &registry,
        "upstream-test-token",
    );
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials);
    let metrics = state.metrics();
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

async fn wait_for_request_terminal(metrics: &GatewayMetrics) -> GatewayMetricsSnapshot {
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
async fn provider_snapshot_records_dimensions_usage_and_cache_observation() {
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
    assert_eq!(snapshot.key.protocol, "chat_completions");
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
    assert_eq!(snapshot.duration_ms.count, 1);
}

#[tokio::test]
async fn provider_snapshot_separates_upstream_and_gateway_ttft_for_streaming() {
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
}
