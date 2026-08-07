//! Startup-owned OpenTelemetry trace and metrics lifecycle over OTLP/HTTP.
//!
//! Both optional signals share one fixed process resource. Exporters accept only validated
//! bootstrap collector bases and use fixed protobuf paths, cumulative metrics, empty configured
//! headers, bounded queues, intervals, timeouts, cardinality, and shutdown. They never read
//! business requests or request-selected exporter policy.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use opentelemetry::{KeyValue, metrics::MeterProvider as _, trace::TracerProvider as _};
use opentelemetry_http::{Bytes, HttpClient, HttpError, Request, Response};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    metrics::{PeriodicReader, SdkMeterProvider, Stream, Temporality},
    trace::{
        BatchConfigBuilder, BatchSpanProcessor, Sampler, SdkTracer, SdkTracerProvider, SpanData,
        SpanExporter,
    },
};
use thiserror::Error;
use tracing::Subscriber;
use tracing_subscriber::{Layer, filter::filter_fn, registry::LookupSpan};

use crate::config::BootstrapConfig;

use super::GatewayMetrics;

const SERVICE_NAME: &str = "openbridge";
const REQUEST_SPAN_NAME: &str = "downstream_request";
const ATTEMPT_SPAN_NAME: &str = "provider_attempt";
const EXPORT_TIMEOUT: Duration = Duration::from_millis(500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TASK_TIMEOUT: Duration = Duration::from_millis(2_250);
const BATCH_DELAY: Duration = Duration::from_millis(200);
const METRIC_EXPORT_INTERVAL: Duration = Duration::from_secs(60);
const MAX_QUEUE_SIZE: usize = 1_024;
const MAX_EXPORT_BATCH_SIZE: usize = 128;
const MAX_METRIC_CARDINALITY: usize = 1_024;

/// Failure while creating or stopping startup-owned OpenTelemetry providers.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// A fixed OTLP/HTTP exporter or SDK provider could not be created.
    #[error("failed to initialize OpenTelemetry OTLP/HTTP export")]
    Initialization,
    /// One or more telemetry workers could not stop within the fixed shutdown boundary.
    #[error("OpenTelemetry export did not stop cleanly within its shutdown boundary")]
    Shutdown,
}

/// Optional process-owned tracer and meter providers retained until the Axum service stops.
pub struct TelemetryRuntime {
    trace_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    metrics: GatewayMetrics,
}

impl TelemetryRuntime {
    /// Builds disabled signals or fixed OTLP/HTTP exporters from validated bootstrap policy.
    pub fn from_bootstrap(bootstrap: &BootstrapConfig) -> Result<Self, TelemetryError> {
        // Freeze one resource identity before constructing either independently optional signal.
        let resource = process_resource();

        // Build tracing only when its startup-owned collector base is configured.
        let trace_provider = bootstrap
            .otlp_http_trace_export()
            .map(|config| build_trace_provider(config.endpoint().as_str(), resource.clone()))
            .transpose()?;

        // Build metrics only when configured and create no-op instruments otherwise.
        let meter_provider = bootstrap
            .otlp_http_metrics_export()
            .map(|config| build_meter_provider(config.endpoint().as_str(), resource))
            .transpose()?;
        let metrics = meter_provider
            .as_ref()
            .map(|provider| GatewayMetrics::from_meter(provider.meter(SERVICE_NAME)))
            .unwrap_or_default();

        Ok(Self {
            trace_provider,
            meter_provider,
            metrics,
        })
    }

    /// Returns a tracer for the reviewed OpenBridge span layer when trace export is enabled.
    pub fn tracer(&self) -> Option<SdkTracer> {
        self.trace_provider
            .as_ref()
            .map(|provider| provider.tracer(SERVICE_NAME))
    }

    /// Returns the process meter instruments or no-op instruments when metrics are disabled.
    pub fn metrics(&self) -> GatewayMetrics {
        self.metrics.clone()
    }

    /// Flushes pending metrics and spans and stops both providers within a fixed outer timeout.
    pub async fn shutdown(self) -> Result<(), TelemetryError> {
        // Drop the runtime's instrument clone before moving SDK providers to the blocking worker.
        let Self {
            trace_provider,
            meter_provider,
            metrics: _,
        } = self;
        if trace_provider.is_none() && meter_provider.is_none() {
            return Ok(());
        }

        // Run both SDK blocking shutdown paths away from Tokio workers under one hard outer bound.
        let shutdown = tokio::task::spawn_blocking(move || {
            let metrics_result = meter_provider
                .map(|provider| provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT))
                .unwrap_or(Ok(()));
            let traces_result = trace_provider
                .map(|provider| provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT))
                .unwrap_or(Ok(()));
            metrics_result.and(traces_result)
        });
        match tokio::time::timeout(SHUTDOWN_TASK_TIMEOUT, shutdown).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => Err(TelemetryError::Shutdown),
        }
    }
}

/// Builds the fixed batch trace provider for one validated collector base.
fn build_trace_provider(
    collector_base: &str,
    resource: Resource,
) -> Result<SdkTracerProvider, TelemetryError> {
    // Build a credential-free protobuf exporter with programmatic transport policy.
    let trace_endpoint = signal_endpoint(collector_base, "/v1/traces")?;
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_http_client(FixedHeaderHttpClient::new()?)
        .with_endpoint(trace_endpoint)
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(EXPORT_TIMEOUT)
        .with_headers(HashMap::new())
        .build()
        .map_err(|_| TelemetryError::Initialization)?;
    let exporter = DiagnosticSpanExporter::new(exporter);

    // Use a bounded background queue so export I/O never executes on the request path.
    let batch = BatchConfigBuilder::default()
        .with_max_queue_size(MAX_QUEUE_SIZE)
        .with_max_export_batch_size(MAX_EXPORT_BATCH_SIZE)
        .with_scheduled_delay(BATCH_DELAY)
        .build();
    let processor = BatchSpanProcessor::new(exporter, batch);

    // Retain the shared resource and fixed sampling policy in the SDK provider.
    Ok(SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(resource)
        .with_span_processor(processor)
        .build())
}

/// Builds the fixed cumulative metrics provider and periodic reader for one collector base.
fn build_meter_provider(
    collector_base: &str,
    resource: Resource,
) -> Result<SdkMeterProvider, TelemetryError> {
    // Build a credential-free protobuf exporter and pin cumulative temporality programmatically.
    let metrics_endpoint = signal_endpoint(collector_base, "/v1/metrics")?;
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_http_client(FixedHeaderHttpClient::new()?)
        .with_endpoint(metrics_endpoint)
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(EXPORT_TIMEOUT)
        .with_headers(HashMap::new())
        .with_temporality(Temporality::Cumulative)
        .build()
        .map_err(|_| TelemetryError::Initialization)?;

    // Own one fixed-interval reader instead of accepting environment-selected collection policy.
    let reader = PeriodicReader::builder(exporter)
        .with_interval(METRIC_EXPORT_INTERVAL)
        .build();

    // Let the SDK aggregate every instrument while bounding each distinct attribute set.
    Ok(SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .with_view(|_| {
            Stream::builder()
                .with_cardinality_limit(MAX_METRIC_CARDINALITY)
                .build()
                .ok()
        })
        .build())
}

/// Appends one fixed signal path to an already validated collector base.
fn signal_endpoint(collector_base: &str, signal_path: &str) -> Result<String, TelemetryError> {
    let mut endpoint =
        url::Url::parse(collector_base).map_err(|_| TelemetryError::Initialization)?;
    endpoint.set_path(signal_path);
    Ok(endpoint.to_string())
}

/// Builds the fixed process resource shared by trace and metrics signals.
fn process_resource() -> Resource {
    Resource::builder_empty()
        .with_service_name(SERVICE_NAME)
        .with_attribute(KeyValue::new("service.instance.id", process_instance_id()))
        .build()
}

/// Blocking exporter client that enforces fixed protocol-only headers and rejects redirects.
#[derive(Debug)]
struct FixedHeaderHttpClient {
    inner: reqwest::blocking::Client,
}

impl FixedHeaderHttpClient {
    /// Creates one timeout-bounded client without redirect-based egress expansion.
    fn new() -> Result<Self, TelemetryError> {
        // Build reqwest's blocking runtime outside any caller-owned asynchronous runtime.
        let builder = std::thread::Builder::new()
            .name("openbridge-otlp-http-client-init".into())
            .spawn(|| {
                reqwest::blocking::Client::builder()
                    .timeout(EXPORT_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
            })
            .map_err(|_| TelemetryError::Initialization)?;

        // Normalize thread and client construction failures at the telemetry boundary.
        let inner = builder
            .join()
            .map_err(|_| TelemetryError::Initialization)?
            .map_err(|_| TelemetryError::Initialization)?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl HttpClient for FixedHeaderHttpClient {
    /// Removes environment-derived headers before delegating the protobuf request.
    async fn send_bytes(&self, mut request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        retain_protocol_headers(request.headers_mut());
        self.inner.send_bytes(request).await
    }
}

/// Retains only headers created by the fixed OTLP protocol implementation.
fn retain_protocol_headers(headers: &mut http::HeaderMap) {
    let retained = [
        &http::header::CONTENT_TYPE,
        &http::header::CONTENT_ENCODING,
        &http::header::USER_AGENT,
    ]
    .into_iter()
    .filter_map(|name| headers.remove(name).map(|value| (name.clone(), value)))
    .collect::<Vec<_>>();
    headers.clear();
    for (name, value) in retained {
        headers.insert(name, value);
    }
}

/// Builds the filtered OpenTelemetry layer shared by the process and deterministic tests.
pub fn otlp_trace_layer<S>(tracer: SdkTracer) -> impl Layer<S> + Send + Sync
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup> + Send + Sync,
{
    // Export only explicitly reviewed lifecycle spans and no tracing events or implicit metadata.
    tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_location(false)
        .with_threads(false)
        .with_target(false)
        .with_level(false)
        .with_tracked_inactivity(false)
        .with_error_records_to_exceptions(false)
        .with_error_fields_to_exceptions(false)
        .with_error_events_to_exceptions(false)
        .with_error_events_to_status(false)
        .with_filter(filter_fn(|metadata| {
            metadata.is_span() && matches!(metadata.name(), REQUEST_SPAN_NAME | ATTEMPT_SPAN_NAME)
        }))
}

/// Wraps a span exporter with one fixed local diagnostic for the first runtime failure.
#[derive(Debug)]
struct DiagnosticSpanExporter<E> {
    inner: E,
    failure_reported: AtomicBool,
}

impl<E> DiagnosticSpanExporter<E> {
    /// Creates a diagnostic wrapper that has not reported an export failure.
    fn new(inner: E) -> Self {
        Self {
            inner,
            failure_reported: AtomicBool::new(false),
        }
    }
}

impl<E> SpanExporter for DiagnosticSpanExporter<E>
where
    E: SpanExporter,
{
    /// Delegates the bounded export and emits at most one content-free local failure diagnostic.
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let result = self.inner.export(batch).await;
        if result.is_err() && !self.failure_reported.swap(true, Ordering::Relaxed) {
            tracing::warn!("OTLP/HTTP trace export failed; telemetry was dropped");
        }
        result
    }

    /// Delegates exporter shutdown with the caller-provided timeout.
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    /// Delegates explicit flush requests to the underlying exporter.
    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    /// Propagates the fixed process resource to the underlying exporter.
    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

/// Builds a process-unique, non-secret resource identity without external configuration.
fn process_instance_id() -> String {
    // Combine the process ID with startup wall-clock nanoseconds to avoid reuse across restarts.
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{started}", std::process::id())
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, header};

    use super::retain_protocol_headers;

    #[test]
    fn exporter_client_removes_environment_derived_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-protobuf"),
        );
        headers.insert(header::USER_AGENT, HeaderValue::from_static("otel-test"));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer synthetic-secret"),
        );
        headers.insert("x-tenant", HeaderValue::from_static("synthetic"));

        // Preserve only protocol-owned transport headers and discard all injected policy.
        retain_protocol_headers(&mut headers);
        assert_eq!(headers.len(), 2);
        assert!(headers.contains_key(header::CONTENT_TYPE));
        assert!(headers.contains_key(header::USER_AGENT));
        assert!(!headers.contains_key(header::AUTHORIZATION));
        assert!(!headers.contains_key("x-tenant"));
    }
}
