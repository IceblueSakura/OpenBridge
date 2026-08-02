//! 验证请求终态观测、usage 提取和敏感业务内容不进入诊断输出。

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

fn app_with_transport(
    transport: Arc<dyn UpstreamTransport>,
) -> (axum::Router, openbridge::observability::GatewayMetrics) {
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
