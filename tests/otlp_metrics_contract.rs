//! Verifies OTLP/HTTP metric shape, native SDK aggregation, redaction, and failure isolation.

mod support;

use std::{
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
    observability::TelemetryRuntime,
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use opentelemetry_proto::tonic::{
    collector::metrics::v1::ExportMetricsServiceRequest,
    common::v1::{KeyValue, any_value},
    metrics::v1::{HistogramDataPoint, Metric, NumberDataPoint, metric, number_data_point},
    resource::v1::Resource,
};
use prost::Message;
use tower::ServiceExt;

const DOWNSTREAM_TOKEN: &str = "downstream-test-token-00000000000";
const UPSTREAM_TOKEN: &str = "upstream-synthetic-secret";
const REQUEST_MARKER: &str = "sensitive-business-request-marker";
const RESPONSE_MARKER: &str = "sensitive-business-response-marker";
const TOKEN_BOUNDARIES: &[f64] = &[
    1.0,
    4.0,
    16.0,
    64.0,
    256.0,
    1_024.0,
    4_096.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    16_777_216.0,
    67_108_864.0,
];

struct SuccessfulTransport;

struct RetryThenSuccessTransport {
    attempts: AtomicUsize,
}

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
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(format!(
                    r#"{{"id":"chatcmpl-otlp-metrics","choices":[{{"message":{{"role":"assistant","content":"{RESPONSE_MARKER}"}}}}],"usage":{{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5,"prompt_tokens_details":{{"cached_tokens":1}},"completion_tokens_details":{{"reasoning_tokens":1}}}}}}"#
                )),
            ))
        })
    }
}

impl UpstreamTransport for RetryThenSuccessTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
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
                    r#"{"id":"chatcmpl-retry-metrics","choices":[{"message":{"role":"assistant","content":"ok"}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#,
                ),
            ))
        })
    }
}

#[derive(Clone, Default)]
struct CollectorState {
    payloads: Arc<Mutex<Vec<CapturedPayload>>>,
    calls: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct CapturedPayload {
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
}

async fn capture_metrics_export(
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

    // Return the empty successful protobuf response defined by the OTLP metrics service.
    (
        StatusCode::OK,
        [("content-type", "application/x-protobuf")],
        Bytes::new(),
    )
}

async fn block_metrics_export(State(state): State<CollectorState>, _body: Bytes) -> Response {
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
        .route("/v1/metrics", post(capture_metrics_export))
        .fallback(capture_metrics_export)
        .with_state(state.clone());
    let (endpoint, server) = start_collector(app).await;
    (endpoint, state, server)
}

async fn start_blocking_collector() -> (String, CollectorState, tokio::task::JoinHandle<()>) {
    let state = CollectorState::default();
    let app = Router::new()
        .route("/v1/metrics", post(block_metrics_export))
        .with_state(state.clone());
    let (endpoint, server) = start_collector(app).await;
    (endpoint, state, server)
}

fn bootstrap_with_metrics_export(endpoint: &str) -> BootstrapConfig {
    // Extend the canonical bootstrap fixture with the one approved metrics-export signal.
    parse_bootstrap_config(&format!(
        "{}\n[telemetry.metrics]\notlp_http_endpoint = \"{endpoint}\"\n",
        support::BOOTSTRAP
    ))
    .unwrap()
}

fn app_with_metrics(metrics: openbridge::observability::GatewayMetrics) -> Router {
    app_with_transport(metrics, Arc::new(SuccessfulTransport))
}

fn app_with_transport(
    metrics: openbridge::observability::GatewayMetrics,
    transport: Arc<dyn UpstreamTransport>,
) -> Router {
    // Build the ordinary synthetic registry and bind only synthetic credentials.
    let registry = support::registry("otlp-metrics-test", "code-primary", "test-model");
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_TOKEN, &registry, UPSTREAM_TOKEN);
    let state =
        GatewayState::new(Arc::new(registry), transport, users, credentials).with_metrics(metrics);
    build_router(state)
}

async fn execute_business_request(app: Router) -> (StatusCode, Bytes) {
    // Execute one authenticated non-streaming Chat Completions request through the real router layers.
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", format!("Bearer {DOWNSTREAM_TOKEN}"))
                .header(CONTENT_TYPE, "application/json")
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

fn decode_exports(payloads: &[CapturedPayload]) -> (Vec<Resource>, Vec<(String, Metric)>) {
    let mut resources = Vec::new();
    let mut metrics = Vec::new();

    // Decode every batch because force-flush may legally split scopes across OTLP requests.
    for payload in payloads {
        let request = ExportMetricsServiceRequest::decode(payload.body.as_ref()).unwrap();
        for resource_metrics in request.resource_metrics {
            if let Some(resource) = resource_metrics.resource {
                resources.push(resource);
            }
            for scope_metrics in resource_metrics.scope_metrics {
                let scope = scope_metrics
                    .scope
                    .map(|scope| scope.name)
                    .unwrap_or_default();
                metrics.extend(
                    scope_metrics
                        .metrics
                        .into_iter()
                        .map(|metric| (scope.clone(), metric)),
                );
            }
        }
    }
    (resources, metrics)
}

fn metric_named<'a>(metrics: &'a [(String, Metric)], name: &str) -> &'a Metric {
    metrics
        .iter()
        .find_map(|(_, metric)| (metric.name == name).then_some(metric))
        .unwrap_or_else(|| panic!("missing OTLP metric {name}"))
}

fn sum_points(metric: &Metric) -> &[NumberDataPoint] {
    match metric.data.as_ref() {
        Some(metric::Data::Sum(sum)) => &sum.data_points,
        _ => panic!("metric {} is not a sum", metric.name),
    }
}

fn histogram_points(metric: &Metric) -> &[HistogramDataPoint] {
    match metric.data.as_ref() {
        Some(metric::Data::Histogram(histogram)) => &histogram.data_points,
        _ => panic!("metric {} is not a histogram", metric.name),
    }
}

fn point_integer(point: &NumberDataPoint) -> i64 {
    match point.value {
        Some(number_data_point::Value::AsInt(value)) => value,
        _ => panic!("expected integer metric point"),
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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[tokio::test]
async fn otlp_http_exports_native_gateway_and_gen_ai_metrics() {
    // Start a loopback collector and inject the runtime-owned meter into the real gateway state.
    let (endpoint, collector, server) = start_capturing_collector().await;
    let bootstrap = bootstrap_with_metrics_export(&endpoint);
    let runtime = TelemetryRuntime::from_bootstrap(&bootstrap).unwrap();
    let app = app_with_metrics(runtime.metrics());

    // Complete one request whose explicit Provider usage includes input, output, and cache-read tokens.
    let (status, body) = execute_business_request(app).await;
    assert_eq!(status, StatusCode::OK);
    assert!(contains_bytes(&body, RESPONSE_MARKER.as_bytes()));

    // Flush and stop the periodic reader while the fake collector can still accept the final export.
    runtime.shutdown().await.unwrap();
    server.abort();
    let _ = server.await;
    let payloads = collector.payloads.lock().unwrap().clone();
    assert!(!payloads.is_empty());
    assert!(payloads.iter().all(|payload| {
        payload.uri.path() == "/v1/metrics"
            && payload.headers.get("authorization").is_none()
            && payload
                .headers
                .get(CONTENT_TYPE)
                .is_some_and(|value| value == "application/x-protobuf")
    }));

    // Verify the process resource and one fixed instrumentation scope shared with tracing.
    let (resources, metrics) = decode_exports(&payloads);
    assert!(!resources.is_empty());
    assert!(metrics.iter().all(|(scope, _)| scope == "openbridge"));
    for resource in &resources {
        assert_eq!(
            string_value(&resource.attributes, "service.name"),
            Some("openbridge")
        );
        assert!(string_value(&resource.attributes, "service.instance.id").is_some());
    }

    // Verify started counters remain while terminal counts come from their duration histograms.
    let request_started = metric_named(&metrics, "openbridge.downstream.request.started");
    assert_eq!(request_started.unit, "{request}");
    assert_eq!(sum_points(request_started).len(), 1);
    assert_eq!(point_integer(&sum_points(request_started)[0]), 1);
    assert_eq!(
        string_value(
            &sum_points(request_started)[0].attributes,
            "openbridge.request.kind"
        ),
        Some("generation")
    );
    let request_duration = metric_named(&metrics, "openbridge.downstream.request.duration");
    let completed_point = &histogram_points(request_duration)[0];
    assert_eq!(completed_point.count, 1);
    assert_eq!(
        string_value(&completed_point.attributes, "openbridge.request.outcome"),
        Some("completed")
    );
    assert_eq!(
        string_value(&completed_point.attributes, "openbridge.request.kind"),
        Some("generation")
    );

    let attempt_started = metric_named(&metrics, "openbridge.provider.attempt.started");
    assert_eq!(attempt_started.unit, "{attempt}");
    assert_eq!(point_integer(&sum_points(attempt_started)[0]), 1);
    let attempt_duration = metric_named(&metrics, "openbridge.provider.attempt.duration");
    let attempt_point = &histogram_points(attempt_duration)[0];
    assert_eq!(attempt_point.count, 1);
    for (key, value) in [
        ("gen_ai.provider.name", "openai"),
        ("gen_ai.operation.name", "chat"),
        ("gen_ai.request.model", "test-model"),
        ("openbridge.public_model", "code-primary"),
        ("openbridge.route.id", "public-chat"),
        ("openbridge.upstream.target", "openai-main"),
        ("openbridge.upstream.operation", "chat_completions"),
        ("openbridge.downstream.operation", "chat_completions"),
        ("openbridge.route.mode", "native"),
        ("openbridge.attempt.outcome", "completed"),
    ] {
        assert_eq!(string_value(&attempt_point.attributes, key), Some(value));
    }
    assert_eq!(
        bool_value(&attempt_point.attributes, "gen_ai.request.stream"),
        Some(false)
    );

    for removed in [
        "openbridge.downstream.request.completed",
        "openbridge.provider.attempt.completed",
        "gen_ai.client.operation.duration",
        "openbridge.gateway.time_to_first_output",
    ] {
        assert!(
            metrics.iter().all(|(_, metric)| metric.name != removed),
            "removed metric {removed} was still exported"
        );
    }

    // Verify standard token usage and the explicit reasoning subset use token histograms.
    let token_usage = metric_named(&metrics, "gen_ai.client.token.usage");
    assert_eq!(token_usage.unit, "{token}");
    assert_eq!(histogram_points(token_usage).len(), 2);
    for (token_type, expected) in [("input", 3.0), ("output", 2.0)] {
        let point = histogram_points(token_usage)
            .iter()
            .find(|point| string_value(&point.attributes, "gen_ai.token.type") == Some(token_type))
            .unwrap();
        assert_eq!(point.count, 1);
        assert_eq!(point.sum, Some(expected));
        assert_eq!(point.explicit_bounds, TOKEN_BOUNDARIES);
    }
    let reasoning_usage =
        metric_named(&metrics, "openbridge.provider.reasoning.output.token.usage");
    assert_eq!(reasoning_usage.unit, "{token}");
    assert_eq!(histogram_points(reasoning_usage)[0].sum, Some(1.0));

    // Verify cache and gateway timing observations are delegated to native histogram/counter aggregation.
    let cache_read = metric_named(&metrics, "openbridge.provider.cache.read.token.usage");
    assert_eq!(cache_read.unit, "{token}");
    assert_eq!(histogram_points(cache_read)[0].sum, Some(1.0));
    let cache_requests = metric_named(&metrics, "openbridge.provider.cache.requests");
    let cache_point = &sum_points(cache_requests)[0];
    assert_eq!(point_integer(cache_point), 1);
    assert_eq!(
        string_value(&cache_point.attributes, "openbridge.cache.result"),
        Some("hit")
    );
    for name in [
        "openbridge.downstream.response_ready.duration",
        "openbridge.downstream.time_to_first_output",
        "openbridge.provider.response_ready.duration",
        "openbridge.provider.first_byte.duration",
    ] {
        let metric = metric_named(&metrics, name);
        assert_eq!(metric.unit, "s");
        assert_eq!(histogram_points(metric)[0].count, 1);
    }

    // Search raw protobuf strings as a final guard against exporting identities, secrets, URLs, or bodies.
    let wire = payloads
        .iter()
        .flat_map(|payload| payload.body.iter().copied())
        .collect::<Vec<_>>();
    for forbidden in [
        DOWNSTREAM_TOKEN,
        UPSTREAM_TOKEN,
        "test-user",
        REQUEST_MARKER,
        RESPONSE_MARKER,
        "/v1/chat/completions",
        "https://api.openai.com",
    ] {
        assert!(
            !contains_bytes(&wire, forbidden.as_bytes()),
            "metrics export leaked {forbidden}"
        );
    }
}

#[tokio::test]
async fn retry_metrics_retain_cause_action_and_recovery_without_duplicate_counters() {
    let (endpoint, collector, server) = start_capturing_collector().await;
    let bootstrap = bootstrap_with_metrics_export(&endpoint);
    let runtime = TelemetryRuntime::from_bootstrap(&bootstrap).unwrap();
    let app = app_with_transport(
        runtime.metrics(),
        Arc::new(RetryThenSuccessTransport {
            attempts: AtomicUsize::new(0),
        }),
    );

    let (status, _) = execute_business_request(app).await;
    assert_eq!(status, StatusCode::OK);
    runtime.shutdown().await.unwrap();
    server.abort();
    let _ = server.await;
    let payloads = collector.payloads.lock().unwrap().clone();
    let (_, metrics) = decode_exports(&payloads);

    let request_duration = metric_named(&metrics, "openbridge.downstream.request.duration");
    let request_point = histogram_points(request_duration)
        .iter()
        .find(|point| {
            string_value(&point.attributes, "openbridge.request.outcome") == Some("completed")
        })
        .unwrap();
    assert_eq!(
        string_value(&request_point.attributes, "openbridge.request.recovery"),
        Some("retry")
    );
    assert_eq!(
        string_value(&request_point.attributes, "openbridge.http.status_class"),
        Some("2xx")
    );

    let attempt_duration = metric_named(&metrics, "openbridge.provider.attempt.duration");
    let failed = histogram_points(attempt_duration)
        .iter()
        .find(|point| {
            string_value(&point.attributes, "openbridge.attempt.outcome") == Some("http_failed")
        })
        .unwrap();
    assert_eq!(
        string_value(&failed.attributes, "error.type"),
        Some("upstream_unavailable")
    );
    assert_eq!(
        string_value(&failed.attributes, "openbridge.http.status_class"),
        Some("5xx")
    );
    assert_eq!(
        bool_value(&failed.attributes, "openbridge.retryable"),
        Some(true)
    );
    assert_eq!(
        string_value(&failed.attributes, "openbridge.next_action"),
        Some("retry_candidate")
    );
    let completed = histogram_points(attempt_duration)
        .iter()
        .find(|point| {
            string_value(&point.attributes, "openbridge.attempt.outcome") == Some("completed")
        })
        .unwrap();
    assert_eq!(
        string_value(&completed.attributes, "openbridge.http.status_class"),
        Some("2xx")
    );
    assert!(string_value(&completed.attributes, "error.type").is_none());

    let routing = metric_named(&metrics, "openbridge.routing.events");
    let retry = sum_points(routing)
        .iter()
        .find(|point| string_value(&point.attributes, "openbridge.routing.event") == Some("retry"))
        .unwrap();
    assert_eq!(point_integer(retry), 1);
    assert_eq!(
        string_value(&retry.attributes, "openbridge.routing.reason"),
        Some("upstream_unavailable")
    );
}

#[tokio::test]
async fn disabled_or_unavailable_metrics_collector_does_not_change_gateway_response() {
    // Keep the absent signal fully disabled and prove an unrelated live collector receives no egress.
    let (_unused_endpoint, disabled_collector, disabled_server) = start_capturing_collector().await;
    let disabled_bootstrap = parse_bootstrap_config(support::BOOTSTRAP).unwrap();
    let disabled_runtime = TelemetryRuntime::from_bootstrap(&disabled_bootstrap).unwrap();
    let disabled = execute_business_request(app_with_metrics(disabled_runtime.metrics())).await;
    disabled_runtime.shutdown().await.unwrap();
    assert_eq!(disabled_collector.calls.load(Ordering::SeqCst), 0);
    disabled_server.abort();
    let _ = disabled_server.await;

    // Enable a collector that never responds and keep the complete business response independent of export I/O.
    let (endpoint, blocked_collector, blocked_server) = start_blocking_collector().await;
    let bootstrap = bootstrap_with_metrics_export(&endpoint);
    let runtime = TelemetryRuntime::from_bootstrap(&bootstrap).unwrap();
    let blocked = tokio::time::timeout(
        Duration::from_secs(1),
        execute_business_request(app_with_metrics(runtime.metrics())),
    )
    .await
    .expect("collector backpressure must not delay the gateway response");
    assert_eq!(blocked, disabled);

    // Keep final collection and exporter shutdown bounded despite the blocked response.
    let _shutdown_result = tokio::time::timeout(Duration::from_secs(2), runtime.shutdown())
        .await
        .expect("metrics shutdown must be bounded");
    assert!(blocked_collector.calls.load(Ordering::SeqCst) > 0);
    blocked_server.abort();
    let _ = blocked_server.await;
}
