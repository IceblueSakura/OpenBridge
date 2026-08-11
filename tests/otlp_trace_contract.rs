//! Verifies OTLP/HTTP trace shape, redaction, and exporter-failure isolation with a loopback collector.

mod support;

use std::{
    collections::BTreeSet,
    future::pending,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, Request, StatusCode, Uri, header::CONTENT_TYPE},
    response::Response,
    routing::post,
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use openbridge::{
    config::{BootstrapConfig, parse_bootstrap_config},
    ingress::{GatewayState, build_router},
    observability::{GatewayMetrics, TelemetryRuntime, otlp_trace_layer},
    provider::PreparedUpstreamRequest,
    registry::{UpstreamTarget, build_registry},
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{KeyValue, any_value},
    resource::v1::Resource,
    trace::v1::Span as OtlpSpan,
};
use prost::Message;
use tokio::sync::Notify;
use tower::ServiceExt;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::layer::SubscriberExt;

const DOWNSTREAM_TOKEN: &str = "downstream-test-token-00000000000";
const UPSTREAM_TOKEN: &str = "upstream-synthetic-secret";
const REQUEST_HEADER_MARKER: &str = "safe-request-header-marker";
const REQUEST_MARKER: &str = "sensitive-business-request-marker";
const RESPONSE_HEADER_MARKER: &str = "safe-response-header-marker";
const RESPONSE_MARKER: &str = "sensitive-business-response-marker";

struct SuccessfulTransport;

impl UpstreamTransport for SuccessfulTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(
                "openai-request-id",
                HeaderValue::from_static(RESPONSE_HEADER_MARKER),
            );
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(format!(
                    r#"{{"id":"chatcmpl-otlp","choices":[{{"message":{{"role":"assistant","content":"{RESPONSE_MARKER}"}}}}],"usage":{{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}}}"#
                )),
            ))
        })
    }
}

#[derive(Clone, Default)]
struct CollectorState {
    payloads: Arc<Mutex<Vec<CapturedPayload>>>,
    calls: Arc<AtomicUsize>,
    payload_ready: Arc<Notify>,
}

#[derive(Clone)]
struct CapturedPayload {
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
}

async fn capture_trace_export(
    State(state): State<CollectorState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> (StatusCode, [(&'static str, &'static str); 1], Bytes) {
    // Capture the complete OTLP request without interpreting it inside the collector handler.
    state.calls.fetch_add(1, Ordering::SeqCst);
    state
        .payloads
        .lock()
        .unwrap()
        .push(CapturedPayload { headers, uri, body });
    state.payload_ready.notify_waiters();

    // Return the empty successful protobuf response defined by the OTLP trace service.
    (
        StatusCode::OK,
        [("content-type", "application/x-protobuf")],
        Bytes::new(),
    )
}

async fn block_trace_export(State(state): State<CollectorState>, _body: Bytes) -> Response {
    // Confirm the export reached the collector, then leave the response pending to exercise timeout isolation.
    state.calls.fetch_add(1, Ordering::SeqCst);
    pending::<Response>().await
}

async fn start_collector(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    // Give each test its own random loopback port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    // Serve the supplied collector behavior until the owning test aborts the task.
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), server)
}

async fn start_capturing_collector() -> (String, CollectorState, tokio::task::JoinHandle<()>) {
    let state = CollectorState::default();
    let app = Router::new()
        .route("/v1/traces", post(capture_trace_export))
        .fallback(capture_trace_export)
        .with_state(state.clone());
    let (endpoint, server) = start_collector(app).await;
    (endpoint, state, server)
}

async fn start_blocking_collector() -> (String, CollectorState, tokio::task::JoinHandle<()>) {
    let state = CollectorState::default();
    let app = Router::new()
        .route("/v1/traces", post(block_trace_export))
        .with_state(state.clone());
    let (endpoint, server) = start_collector(app).await;
    (endpoint, state, server)
}

fn bootstrap_with_trace_export(endpoint: &str) -> BootstrapConfig {
    // Extend the canonical bootstrap fixture with the one approved trace-export signal.
    parse_bootstrap_config(&format!(
        "{}\n[telemetry.traces]\notlp_http_endpoint = \"{endpoint}\"\n",
        support::BOOTSTRAP
    ))
    .unwrap()
}

fn bootstrap_with_trace_export_and_local_content(endpoint: &str) -> BootstrapConfig {
    // Enable every local content event alongside traces so OTLP exclusion is exercised explicitly.
    parse_bootstrap_config(&format!(
        "{}\n[logging]\nrequest_headers = true\nrequest_body = true\nresponse_headers = true\nresponse_body = true\n\n[telemetry.traces]\notlp_http_endpoint = \"{endpoint}\"\n",
        support::BOOTSTRAP
    ))
    .unwrap()
}

fn app_with_metrics(metrics: GatewayMetrics) -> Router {
    // Build the ordinary synthetic registry and bind only synthetic credentials.
    let registry = support::registry("otlp-test", "code-primary", "test-model");
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_TOKEN, &registry, UPSTREAM_TOKEN);
    let state = GatewayState::new(
        Arc::new(registry),
        Arc::new(SuccessfulTransport),
        users,
        credentials,
    )
    .with_metrics(metrics);
    build_router(state)
}

fn app_with_bootstrap_and_metrics(bootstrap: BootstrapConfig, metrics: GatewayMetrics) -> Router {
    // Compile the ordinary synthetic registry with the exact logging policy under test.
    let registry = build_registry(
        bootstrap,
        support::definition("otlp-test", "code-primary", "test-model"),
    )
    .unwrap();
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_TOKEN, &registry, UPSTREAM_TOKEN);
    let state = GatewayState::new(
        Arc::new(registry),
        Arc::new(SuccessfulTransport),
        users,
        credentials,
    )
    .with_metrics(metrics);
    build_router(state)
}

async fn execute_business_request(app: Router) -> (StatusCode, Bytes) {
    // Execute one authenticated non-streaming Chat Completions request through the real router layers.
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", format!("Bearer {DOWNSTREAM_TOKEN}"))
                .header(CONTENT_TYPE, "application/json")
                .header("x-request-debug", REQUEST_HEADER_MARKER)
                .body(Body::from(format!(
                    r#"{{"model":"code-primary","messages":[{{"role":"user","content":"{REQUEST_MARKER}"}}]}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();

    // Consume the full body so request and attempt observations reach their unique terminal boundaries.
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    (status, body)
}

fn decode_exports(payloads: &[CapturedPayload]) -> (Vec<Resource>, Vec<OtlpSpan>) {
    let mut resources = Vec::new();
    let mut spans = Vec::new();

    // Decode every batch because force-flush may legally split two spans across OTLP requests.
    for payload in payloads {
        let request = ExportTraceServiceRequest::decode(payload.body.as_ref()).unwrap();
        for resource_spans in request.resource_spans {
            if let Some(resource) = resource_spans.resource {
                resources.push(resource);
            }
            for scope_spans in resource_spans.scope_spans {
                spans.extend(scope_spans.spans);
            }
        }
    }
    (resources, spans)
}

async fn wait_for_exported_spans(state: &CollectorState, expected: usize) -> Vec<CapturedPayload> {
    let result = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            // Register before checking so an export cannot complete between the check and await.
            let notified = state.payload_ready.notified();
            let payloads = state.payloads.lock().unwrap().clone();
            if decode_exports(&payloads).1.len() >= expected {
                return payloads;
            }
            notified.await;
        }
    })
    .await;
    match result {
        Ok(payloads) => payloads,
        Err(_) => {
            let payloads = state.payloads.lock().unwrap().clone();
            let names = decode_exports(&payloads)
                .1
                .into_iter()
                .map(|span| span.name)
                .collect::<Vec<_>>();
            panic!("expected {expected} exported spans, observed {names:?}");
        }
    }
}

fn string_value<'a>(attributes: &'a [KeyValue], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.key == key)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| value.value.as_ref())
        .and_then(|value| match value {
            any_value::Value::StringValue(value) => Some(value.as_str()),
            _ => None,
        })
}

fn bool_value(attributes: &[KeyValue], key: &str) -> Option<bool> {
    attributes
        .iter()
        .find(|attribute| attribute.key == key)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| value.value.as_ref())
        .and_then(|value| match value {
            any_value::Value::BoolValue(value) => Some(*value),
            _ => None,
        })
}

fn integer_value(attributes: &[KeyValue], key: &str) -> Option<i64> {
    attributes
        .iter()
        .find(|attribute| attribute.key == key)
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| value.value.as_ref())
        .and_then(|value| match value {
            any_value::Value::IntValue(value) => Some(*value),
            _ => None,
        })
}

fn assert_attribute_allowlist(span: &OtlpSpan, allowed: &[&str]) {
    // Compare exact emitted keys against a reviewed allowlist while permitting absent optional observations.
    let actual = span
        .attributes
        .iter()
        .map(|attribute| attribute.key.as_str())
        .collect::<BTreeSet<_>>();
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    assert!(
        actual.is_subset(&allowed),
        "span {} exported unexpected attributes: {:?}",
        span.name,
        actual.difference(&allowed).collect::<Vec<_>>()
    );
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[tokio::test]
async fn otlp_http_exports_one_redacted_request_and_attempt_trace() {
    // Start a loopback OTLP/HTTP collector and create the fixed batch exporter before serving a request.
    let (endpoint, collector, server) = start_capturing_collector().await;
    let bootstrap = bootstrap_with_trace_export_and_local_content(&endpoint);
    let runtime = TelemetryRuntime::from_bootstrap(&bootstrap).unwrap();
    let subscriber = tracing_subscriber::registry().with(otlp_trace_layer(
        runtime.tracer().expect("trace exporter should be enabled"),
    ));
    let app = app_with_bootstrap_and_metrics(bootstrap, runtime.metrics());

    // Keep the test-local subscriber attached through response-body completion.
    let (status, body) = execute_business_request(app)
        .with_subscriber(subscriber)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(contains_bytes(&body, RESPONSE_MARKER.as_bytes()));

    // Flush and stop the exporter while the fake collector can still accept the final batch.
    runtime.shutdown().await.unwrap();
    let payloads = wait_for_exported_spans(&collector, 2).await;
    server.abort();
    let _ = server.await;
    assert!(!payloads.is_empty());
    let request_shapes = payloads
        .iter()
        .map(|payload| {
            (
                payload.uri.to_string(),
                payload
                    .headers
                    .get(CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        payloads.iter().all(|payload| {
            payload.uri.path() == "/v1/traces"
                && payload.headers.get("authorization").is_none()
                && payload
                    .headers
                    .get(CONTENT_TYPE)
                    .is_some_and(|value| value == "application/x-protobuf")
        }),
        "unexpected OTLP request shape: {request_shapes:?}"
    );

    // Decode the OTLP protobuf and find exactly the approved request root and Provider-attempt child.
    let (resources, spans) = decode_exports(&payloads);
    assert!(!resources.is_empty());
    assert_eq!(spans.len(), 2);
    let root = spans
        .iter()
        .find(|span| span.name == "downstream_request")
        .unwrap();
    let attempt = spans
        .iter()
        .find(|span| span.name == "provider_attempt")
        .unwrap();
    assert!(root.parent_span_id.is_empty());
    assert_eq!(root.trace_id, attempt.trace_id);
    assert_eq!(attempt.parent_span_id, root.span_id);
    assert!(root.events.is_empty());
    assert!(attempt.events.is_empty());

    // Verify the stable operation, model, compiled Route, outcome, timing, and explicit usage attributes.
    assert_eq!(
        string_value(&root.attributes, "operation"),
        Some("chat_completions")
    );
    assert_eq!(
        string_value(&root.attributes, "public_model"),
        Some("code-primary")
    );
    assert_eq!(string_value(&root.attributes, "outcome"), Some("completed"));
    assert_eq!(bool_value(&root.attributes, "streaming"), Some(false));
    assert!(string_value(&root.attributes, "request_id").is_some());
    assert_eq!(
        integer_value(&root.attributes, "upstream_attempts"),
        Some(1)
    );
    assert_eq!(integer_value(&root.attributes, "input_tokens"), Some(3));
    assert_eq!(integer_value(&root.attributes, "output_tokens"), Some(2));
    assert_eq!(integer_value(&root.attributes, "total_tokens"), Some(5));

    assert_eq!(integer_value(&attempt.attributes, "attempt"), Some(1));
    assert_eq!(
        string_value(&attempt.attributes, "provider"),
        Some("openai")
    );
    assert_eq!(
        string_value(&attempt.attributes, "route_id"),
        Some("public-chat")
    );
    assert_eq!(
        string_value(&attempt.attributes, "upstream_target"),
        Some("openai-main")
    );
    assert_eq!(
        string_value(&attempt.attributes, "upstream_operation"),
        Some("chat_completions")
    );
    assert_eq!(
        string_value(&attempt.attributes, "route_mode"),
        Some("native")
    );
    assert_eq!(
        string_value(&attempt.attributes, "outcome"),
        Some("completed")
    );
    assert_eq!(bool_value(&attempt.attributes, "streaming"), Some(false));
    assert_eq!(integer_value(&attempt.attributes, "input_tokens"), Some(3));
    assert_eq!(integer_value(&attempt.attributes, "output_tokens"), Some(2));

    // Keep both span types within their reviewed attribute sets and fixed process resource identity.
    assert_attribute_allowlist(
        root,
        &[
            "request_id",
            "operation",
            "public_model",
            "streaming",
            "outcome",
            "response_ready_ms",
            "first_body_byte_ms",
            "first_output_ms",
            "duration_ms",
            "upstream_attempts",
            "upstream_retries",
            "credential_rotations",
            "route_fallbacks",
            "cooldown_skips",
            "input_tokens",
            "output_tokens",
            "total_tokens",
        ],
    );
    assert_attribute_allowlist(
        attempt,
        &[
            "attempt",
            "provider",
            "route_id",
            "upstream_target",
            "upstream_operation",
            "public_model",
            "operation",
            "route_mode",
            "streaming",
            "outcome",
            "response_ready_ms",
            "upstream_first_byte_ms",
            "upstream_ttft_ms",
            "gateway_ttft_ms",
            "duration_ms",
            "generation_duration_ms",
            "input_tokens",
            "output_tokens",
            "total_tokens",
            "cached_input_tokens",
            "cache_write_input_tokens",
        ],
    );
    for resource in &resources {
        assert_eq!(
            string_value(&resource.attributes, "service.name"),
            Some("openbridge")
        );
        assert!(string_value(&resource.attributes, "service.instance.id").is_some());
        assert_attribute_allowlist(
            &OtlpSpan {
                attributes: resource.attributes.clone(),
                ..OtlpSpan::default()
            },
            &["service.name", "service.instance.id"],
        );
    }

    // Search raw protobuf strings as a final guard against exporting identities, secrets, paths, or bodies.
    let wire = payloads
        .iter()
        .flat_map(|payload| payload.body.iter().copied())
        .collect::<Vec<_>>();
    for forbidden in [
        DOWNSTREAM_TOKEN,
        UPSTREAM_TOKEN,
        "test-user",
        REQUEST_HEADER_MARKER,
        REQUEST_MARKER,
        RESPONSE_HEADER_MARKER,
        RESPONSE_MARKER,
        "/v1/chat/completions",
        "https://api.openai.com",
    ] {
        assert!(
            !contains_bytes(&wire, forbidden.as_bytes()),
            "export leaked {forbidden}"
        );
    }
}

#[tokio::test]
async fn disabled_or_unavailable_collector_does_not_change_gateway_response() {
    // Exercise the missing telemetry table against a live collector and prove it receives no egress.
    let (_unused_endpoint, disabled_collector, disabled_server) = start_capturing_collector().await;
    let disabled_bootstrap = parse_bootstrap_config(support::BOOTSTRAP).unwrap();
    let disabled_runtime = TelemetryRuntime::from_bootstrap(&disabled_bootstrap).unwrap();
    assert!(disabled_runtime.tracer().is_none());
    let disabled_app = app_with_metrics(disabled_runtime.metrics());
    let disabled = execute_business_request(disabled_app).await;
    disabled_runtime.shutdown().await.unwrap();
    assert_eq!(disabled_collector.calls.load(Ordering::SeqCst), 0);
    disabled_server.abort();
    let _ = disabled_server.await;

    // Enable an exporter whose collector accepts the request but never returns a response.
    let (endpoint, blocked_collector, blocked_server) = start_blocking_collector().await;
    let bootstrap = bootstrap_with_trace_export(&endpoint);
    let runtime = TelemetryRuntime::from_bootstrap(&bootstrap).unwrap();
    let subscriber = tracing_subscriber::registry().with(otlp_trace_layer(
        runtime.tracer().expect("trace exporter should be enabled"),
    ));
    let blocked_app = app_with_metrics(runtime.metrics());

    // Bound the complete downstream response independently of the exporter timeout and blocked collector.
    let blocked = tokio::time::timeout(
        Duration::from_secs(1),
        execute_business_request(blocked_app).with_subscriber(subscriber),
    )
    .await
    .expect("collector backpressure must not delay the gateway response");
    assert_eq!(blocked.0, disabled.0);
    assert_eq!(blocked.1, disabled.1);

    // Shutdown must remain bounded even though the exporter reports the blocked collector as failed.
    let _shutdown_result = tokio::time::timeout(Duration::from_secs(2), runtime.shutdown())
        .await
        .expect("trace shutdown must be bounded");
    assert!(blocked_collector.calls.load(Ordering::SeqCst) > 0);
    blocked_server.abort();
    let _ = blocked_server.await;
}
