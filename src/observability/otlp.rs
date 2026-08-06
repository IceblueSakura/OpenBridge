//! Startup-owned OTLP/HTTP trace exporter lifecycle.
//!
//! The exporter accepts only the validated bootstrap endpoint, uses fixed protobuf, timeout,
//! sampling, resource, and batch policy, and exports only the two reviewed request-lifecycle span
//! names. It never reads business requests, exporter headers, or environment-selected OTLP policy.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use opentelemetry::{KeyValue, trace::TracerProvider as _};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    trace::{
        BatchConfigBuilder, BatchSpanProcessor, Sampler, SdkTracer, SdkTracerProvider, SpanData,
        SpanExporter,
    },
};
use thiserror::Error;
use tracing::Subscriber;
use tracing_subscriber::{Layer, filter::filter_fn, registry::LookupSpan};

use crate::config::BootstrapConfig;

const SERVICE_NAME: &str = "openbridge";
const REQUEST_SPAN_NAME: &str = "downstream_request";
const ATTEMPT_SPAN_NAME: &str = "provider_attempt";
const EXPORT_TIMEOUT: Duration = Duration::from_millis(500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TASK_TIMEOUT: Duration = Duration::from_millis(1_250);
const BATCH_DELAY: Duration = Duration::from_millis(200);
const MAX_QUEUE_SIZE: usize = 1_024;
const MAX_EXPORT_BATCH_SIZE: usize = 128;

/// Failure while creating or stopping the startup-owned trace exporter.
#[derive(Debug, Error)]
pub enum TraceExportError {
    /// The fixed OTLP/HTTP exporter or SDK provider could not be created.
    #[error("failed to initialize the OTLP/HTTP trace exporter")]
    Initialization,
    /// The exporter worker could not stop within its fixed shutdown boundary.
    #[error("the OTLP/HTTP trace exporter did not stop cleanly within its shutdown boundary")]
    Shutdown,
}

/// Process-owned optional tracer provider retained until the Axum service stops.
pub struct TraceExportRuntime {
    provider: Option<SdkTracerProvider>,
}

impl TraceExportRuntime {
    /// Builds the disabled runtime or a fixed OTLP/HTTP batch exporter from bootstrap.
    pub fn from_bootstrap(bootstrap: &BootstrapConfig) -> Result<Self, TraceExportError> {
        // Return a no-worker runtime when the optional trace signal is absent.
        let Some(config) = bootstrap.otlp_http_trace_export() else {
            return Ok(Self { provider: None });
        };

        // Build a credential-free protobuf exporter with programmatic policy overriding OTLP environment variables.
        let mut trace_endpoint = config.endpoint().clone();
        trace_endpoint.set_path("/v1/traces");
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(trace_endpoint.as_str())
            .with_protocol(Protocol::HttpBinary)
            .with_timeout(EXPORT_TIMEOUT)
            .with_headers(HashMap::new())
            .build()
            .map_err(|_| TraceExportError::Initialization)?;
        let exporter = DiagnosticSpanExporter::new(exporter);

        // Use a bounded background queue so export I/O never executes on the request path.
        let batch = BatchConfigBuilder::default()
            .with_max_queue_size(MAX_QUEUE_SIZE)
            .with_max_export_batch_size(MAX_EXPORT_BATCH_SIZE)
            .with_scheduled_delay(BATCH_DELAY)
            .build();
        let processor = BatchSpanProcessor::new(exporter, batch);

        // Freeze service identity and sampling without accepting environment-provided resource attributes.
        let resource = Resource::builder_empty()
            .with_service_name(SERVICE_NAME)
            .with_attribute(KeyValue::new("service.instance.id", process_instance_id()))
            .build();
        let provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(resource)
            .with_span_processor(processor)
            .build();

        Ok(Self {
            provider: Some(provider),
        })
    }

    /// Returns a tracer for the reviewed OpenBridge span layer when export is enabled.
    pub fn tracer(&self) -> Option<SdkTracer> {
        self.provider
            .as_ref()
            .map(|provider| provider.tracer(SERVICE_NAME))
    }

    /// Flushes pending spans and stops the background exporter within a fixed timeout.
    pub async fn shutdown(self) -> Result<(), TraceExportError> {
        // Skip thread creation when no exporter was configured.
        let Some(provider) = self.provider else {
            return Ok(());
        };

        // Run the SDK's blocking shutdown away from Tokio workers and retain an outer hard bound.
        let shutdown =
            tokio::task::spawn_blocking(move || provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT));
        match tokio::time::timeout(SHUTDOWN_TASK_TIMEOUT, shutdown).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => Err(TraceExportError::Shutdown),
        }
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

/// Wraps an exporter with one fixed local diagnostic for the first runtime failure.
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
