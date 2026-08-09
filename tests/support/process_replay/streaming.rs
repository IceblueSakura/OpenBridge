//! Replays canonical Responses stream abort and downstream-cancellation lifecycles.

use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use futures_util::{StreamExt as _, stream};
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;
use tokio::net::TcpListener;

use super::{GatewayHarness, ReplayObservation, read_json, spawn_server, start_gateway};

struct CanonicalStreamAbortCase {
    client_request: Value,
    expected_upstream_request: Value,
    upstream_stream: Bytes,
    expected_client_stream: Bytes,
}

struct CanonicalCancellationCase {
    client_request: Value,
    expected_upstream_request: Value,
    upstream_stream: Bytes,
    upstream_events: Vec<Bytes>,
}

#[derive(Clone)]
struct MockStreamAbortState {
    expected_request: Value,
    stream_body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
    abort_after_output: Arc<tokio::sync::Notify>,
}

#[derive(Clone)]
struct MockCancellationState {
    expected_request: Value,
    stream_events: Vec<Bytes>,
    observations: Arc<Mutex<Vec<bool>>>,
    body_dropped: Arc<tokio::sync::Notify>,
}

#[derive(Clone)]
struct MockFiniteStreamState {
    expected_request: Value,
    stream_body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
}

struct PendingEventStream {
    events: VecDeque<Bytes>,
    body_dropped: Arc<tokio::sync::Notify>,
}

impl futures_util::Stream for PendingEventStream {
    type Item = Result<Bytes, std::io::Error>;

    /// Emits each complete event and then remains pending until the downstream body is dropped.
    fn poll_next(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.events
            .pop_front()
            .map_or(Poll::Pending, |event| Poll::Ready(Some(Ok(event))))
    }
}

impl Drop for PendingEventStream {
    fn drop(&mut self) {
        // Notify the test only when Hyper drops the pending upstream response body.
        self.body_dropped.notify_one();
    }
}

/// Replays one canonical Responses stream that aborts after visible output.
pub async fn replay_transport_error_after_output_case(case_id: &str) -> ReplayObservation {
    // Load the canonical request, partial SSE body, and downstream byte oracle.
    let CanonicalStreamAbortCase {
        client_request,
        expected_upstream_request,
        upstream_stream,
        expected_client_stream,
    } = load_transport_error_after_output_case(case_id);
    let expected_transparent_stream = upstream_stream.clone();

    // Start a mock upstream that records the request, emits the canonical bytes, and then aborts.
    let observations = Arc::new(Mutex::new(Vec::new()));
    let abort_after_output = Arc::new(tokio::sync::Notify::new());
    let upstream_state = MockStreamAbortState {
        expected_request: expected_upstream_request,
        stream_body: upstream_stream,
        observations: observations.clone(),
        abort_after_output: abort_after_output.clone(),
    };
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("mock upstream address must exist");
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(mock_stream_then_abort))
            .with_state(upstream_state),
    );

    // Start the production Router with the mock origin and in-memory metric exporter.
    let GatewayHarness {
        address: gateway_address,
        task: gateway_task,
        metrics,
    } = start_gateway(upstream_address).await;

    // Consume every visible downstream chunk until the real HTTP connection reports the abort.
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}/v1/responses"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge replay response headers must arrive");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut downstream_body = Vec::new();
    let mut downstream_transport_error = false;
    let mut downstream_stream = response.bytes_stream();
    let mut abort_released = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(chunk) = downstream_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    downstream_body.extend_from_slice(&chunk);
                    if !abort_released && !chunk.is_empty() {
                        abort_after_output.notify_one();
                        abort_released = true;
                    }
                }
                Err(_) => {
                    downstream_transport_error = true;
                    break;
                }
            }
        }
    })
    .await
    .expect("canonical downstream abort must complete within the test timeout");

    // Stop listeners and return only byte comparisons, counters, and safe response metadata.
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let gateway_metrics = metrics.snapshot();
    let provider_metrics = metrics.provider_snapshots();
    ReplayObservation {
        status,
        content_type,
        retry_after: None,
        rate_limit_remaining_requests: None,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches: downstream_body.as_slice() == expected_client_stream.as_ref(),
        downstream_stream_matches_upstream: downstream_body.as_slice()
            == expected_transparent_stream.as_ref(),
        downstream_transport_error,
        upstream_cancelled: false,
        gateway_metrics,
        provider_metrics,
    }
}

/// Replays one canonical Responses stream cancelled after its declared visible event boundary.
pub async fn replay_cancel_after_output_case(case_id: &str) -> ReplayObservation {
    // Load the canonical request and complete SSE events up to the cancellation boundary.
    let CanonicalCancellationCase {
        client_request,
        expected_upstream_request,
        upstream_stream,
        upstream_events,
    } = load_cancel_after_output_case(case_id);

    // Start a mock upstream whose response remains pending after the canonical event prefix.
    let observations = Arc::new(Mutex::new(Vec::new()));
    let body_dropped = Arc::new(tokio::sync::Notify::new());
    let upstream_state = MockCancellationState {
        expected_request: expected_upstream_request,
        stream_events: upstream_events,
        observations: observations.clone(),
        body_dropped: body_dropped.clone(),
    };
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("mock upstream address must exist");
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(mock_pending_stream))
            .with_state(upstream_state),
    );

    // Start the production Router with the mock origin and in-memory metric exporter.
    let GatewayHarness {
        address: gateway_address,
        task: gateway_task,
        metrics,
    } = start_gateway(upstream_address).await;

    // Read through the declared event boundary without depending on network chunk segmentation.
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}/v1/responses"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge cancellation replay headers must arrive");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut downstream_body = Vec::new();
    let mut downstream_stream = response.bytes_stream();
    tokio::time::timeout(Duration::from_secs(5), async {
        while downstream_body.len() < upstream_stream.len() {
            let chunk = downstream_stream
                .next()
                .await
                .expect("canonical cancellation stream must remain open")
                .expect("canonical cancellation prefix must be readable");
            downstream_body.extend_from_slice(&chunk);
        }
    })
    .await
    .expect("canonical cancellation prefix must arrive within the test timeout");

    // Drop the downstream body and wait for that cancellation to release the upstream response.
    drop(downstream_stream);
    tokio::time::timeout(Duration::from_secs(5), body_dropped.notified())
        .await
        .expect("downstream cancellation must drop the upstream body within the test timeout");

    // Stop listeners and return only byte comparisons, counters, and safe response metadata.
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let gateway_metrics = metrics.snapshot();
    let provider_metrics = metrics.provider_snapshots();
    ReplayObservation {
        status,
        content_type,
        retry_after: None,
        rate_limit_remaining_requests: None,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches: false,
        downstream_stream_matches_upstream: downstream_body.as_slice() == upstream_stream.as_ref(),
        downstream_transport_error: false,
        upstream_cancelled: true,
        gateway_metrics,
        provider_metrics,
    }
}

/// Replays one canonical Responses stream that reaches clean EOF without a terminal event.
pub async fn replay_eof_before_terminal_case(case_id: &str) -> ReplayObservation {
    // Load the canonical request and partial SSE body that intentionally omits a terminal.
    let CanonicalStreamAbortCase {
        client_request,
        expected_upstream_request,
        upstream_stream,
        expected_client_stream,
    } = load_eof_before_terminal_case(case_id);
    let expected_transparent_stream = upstream_stream.clone();

    // Start a mock upstream that records the request and ends cleanly after the partial SSE body.
    let observations = Arc::new(Mutex::new(Vec::new()));
    let upstream_state = MockFiniteStreamState {
        expected_request: expected_upstream_request,
        stream_body: upstream_stream,
        observations: observations.clone(),
    };
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("mock upstream address must exist");
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(mock_finite_stream))
            .with_state(upstream_state),
    );

    // Start the production Router with the mock origin and in-memory metric exporter.
    let GatewayHarness {
        address: gateway_address,
        task: gateway_task,
        metrics,
    } = start_gateway(upstream_address).await;

    // Read the complete downstream body through its clean EOF boundary.
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}/v1/responses"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge EOF replay headers must arrive");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let downstream_body = tokio::time::timeout(Duration::from_secs(5), response.bytes())
        .await
        .expect("canonical EOF must arrive within the test timeout")
        .expect("canonical EOF response body must remain readable");

    // Stop listeners and return only byte comparisons, counters, and safe response metadata.
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let gateway_metrics = metrics.snapshot();
    let provider_metrics = metrics.provider_snapshots();
    ReplayObservation {
        status,
        content_type,
        retry_after: None,
        rate_limit_remaining_requests: None,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches: downstream_body.as_ref() == expected_client_stream.as_ref(),
        downstream_stream_matches_upstream: downstream_body.as_ref()
            == expected_transparent_stream.as_ref(),
        downstream_transport_error: false,
        upstream_cancelled: false,
        gateway_metrics,
        provider_metrics,
    }
}

async fn mock_stream_then_abort(
    State(state): State<MockStreamAbortState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Compare request JSON while observations retain only a boolean result.
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // Emit the canonical SSE prefix, then wait for downstream output before closing the connection.
    let abort_after_output = state.abort_after_output.clone();
    let body = Body::from_stream(
        stream::once(async move { Ok::<_, std::io::Error>(state.stream_body) }).chain(
            stream::once(async move {
                abort_after_output.notified().await;
                Err(std::io::Error::other("canonical upstream transport abort"))
            }),
        ),
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(body)
        .expect("canonical abort response must build")
}

async fn mock_pending_stream(
    State(state): State<MockCancellationState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Compare request JSON while observations retain only a boolean result.
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // Send complete canonical events and retain the drop notification while the body stays pending.
    let body = Body::from_stream(PendingEventStream {
        events: VecDeque::from(state.stream_events),
        body_dropped: state.body_dropped,
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(body)
        .expect("canonical pending response must build")
}

async fn mock_finite_stream(
    State(state): State<MockFiniteStreamState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Compare request JSON while observations retain only a boolean result.
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // Return the canonical partial SSE body and let Hyper terminate it with clean EOF.
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from(state.stream_body))
        .expect("canonical finite response must build")
}

fn load_transport_error_after_output_case(case_id: &str) -> CanonicalStreamAbortCase {
    // Keep this lifecycle helper bound to the one approved canonical stream-abort behavior.
    assert_eq!(
        case_id, "responses_native.transport_error.after_output",
        "stream-abort replay must use the approved canonical case"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/faults")
        .join(case_id);
    let case = read_json(root.join("case.json"));
    assert_eq!(case["id"].as_str(), Some(case_id));

    // Verify the fixture declares a successful SSE response that aborts only after visible output.
    let transport = &case["transport"];
    assert_eq!(transport["upstream_http_status"].as_u64(), Some(200));
    assert_eq!(
        transport["upstream_content_type"].as_str(),
        Some("text/event-stream")
    );
    assert_eq!(
        transport["failure_phase"].as_str(),
        Some("after_first_output")
    );
    assert_eq!(transport["upstream_end"].as_str(), Some("transport_error"));
    assert_eq!(transport["client_end"].as_str(), Some("transport_error"));
    assert_eq!(
        transport["downstream_output_observed"].as_bool(),
        Some(true)
    );

    // Load the fixed request and partial-stream artifacts without exposing their contents.
    CanonicalStreamAbortCase {
        client_request: read_json(root.join("client-request.json")),
        expected_upstream_request: read_json(root.join("expected-upstream-request.json")),
        upstream_stream: Bytes::from(
            std::fs::read(root.join("upstream-stream.sse"))
                .expect("canonical upstream stream must be readable"),
        ),
        expected_client_stream: Bytes::from(
            std::fs::read(root.join("expected-client-stream.sse"))
                .expect("canonical client stream must be readable"),
        ),
    }
}

fn load_cancel_after_output_case(case_id: &str) -> CanonicalCancellationCase {
    // Keep this lifecycle helper bound to the one approved canonical cancellation behavior.
    assert_eq!(
        case_id, "responses_native.cancel.after_output",
        "cancellation replay must use the approved canonical case"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/faults")
        .join(case_id);
    let case = read_json(root.join("case.json"));
    assert_eq!(case["id"].as_str(), Some(case_id));

    // Verify the fixture declares cancellation immediately after its two visible SSE events.
    let transport = &case["transport"];
    assert_eq!(transport["upstream_http_status"].as_u64(), Some(200));
    assert_eq!(
        transport["upstream_content_type"].as_str(),
        Some("text/event-stream")
    );
    assert_eq!(transport["cancellation_after_event"].as_u64(), Some(2));
    assert_eq!(transport["upstream_end"].as_str(), Some("cancelled"));
    assert_eq!(transport["client_end"].as_str(), Some("cancelled"));
    assert_eq!(
        transport["downstream_output_observed"].as_bool(),
        Some(true)
    );

    // Split the checked-in wire only at complete LF event boundaries and retain every original byte.
    let upstream_stream = Bytes::from(
        std::fs::read(root.join("upstream-stream.sse"))
            .expect("canonical upstream stream must be readable"),
    );
    let upstream_events = split_lf_sse_events(&upstream_stream);
    assert_eq!(
        upstream_events.len(),
        2,
        "canonical cancellation fixture must contain two events"
    );

    // Load the fixed request artifacts without exposing their contents.
    CanonicalCancellationCase {
        client_request: read_json(root.join("client-request.json")),
        expected_upstream_request: read_json(root.join("expected-upstream-request.json")),
        upstream_stream,
        upstream_events,
    }
}

fn load_eof_before_terminal_case(case_id: &str) -> CanonicalStreamAbortCase {
    // Keep this lifecycle helper bound to the one approved canonical EOF behavior.
    assert_eq!(
        case_id, "responses_native.eof_before_terminal",
        "EOF replay must use the approved canonical case"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/faults")
        .join(case_id);
    let case = read_json(root.join("case.json"));
    assert_eq!(case["id"].as_str(), Some(case_id));

    // Verify the accepted fixture's original oracle declares clean EOF and no terminal.
    assert_eq!(case["expectation"]["outcome"].as_str(), Some("eof"));
    assert_eq!(case["expectation"]["terminal"].as_str(), Some("none"));
    assert_eq!(case["expectation"]["terminal_count"].as_u64(), Some(0));
    assert_eq!(case["expectation"]["upstream_attempts"].as_u64(), Some(1));

    // Load the fixed request and partial-stream artifacts without exposing their contents.
    CanonicalStreamAbortCase {
        client_request: read_json(root.join("client-request.json")),
        expected_upstream_request: read_json(root.join("expected-upstream-request.json")),
        upstream_stream: Bytes::from(
            std::fs::read(root.join("upstream-stream.sse"))
                .expect("canonical upstream stream must be readable"),
        ),
        expected_client_stream: Bytes::from(
            std::fs::read(root.join("expected-client-stream.sse"))
                .expect("canonical client stream must be readable"),
        ),
    }
}

fn split_lf_sse_events(stream: &Bytes) -> Vec<Bytes> {
    // Preserve each double-LF delimiter so concatenating the event chunks reproduces the wire exactly.
    let mut events = Vec::new();
    let mut start = 0;
    while let Some(relative_end) = stream[start..]
        .windows(2)
        .position(|bytes| bytes == b"\n\n")
    {
        let end = start + relative_end + 2;
        events.push(stream.slice(start..end));
        start = end;
    }
    assert_eq!(
        start,
        stream.len(),
        "canonical LF SSE stream must end at an event boundary"
    );
    events
}
