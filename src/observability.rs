//! Facade for request-lifecycle tracing, local HTTP diagnostics, and OpenTelemetry metrics.
//!
//! `metrics` owns fixed OpenTelemetry instruments, `provider` observes each compiled attempt,
//! `request` maintains per-request terminal state and tracing, `http_jsonl` persists explicitly
//! enabled local snapshots, and `usage` parses only usage explicitly returned by the Provider.
//! Local content events are never eligible for the span-only OTLP layer.

mod http_jsonl;

mod classification;
mod metrics;
mod otlp;
mod provider;
mod request;
mod usage;

pub(crate) use classification::{
    AttemptFailure, ErrorType, FailureStage, NextAction, RequestKind, TimeoutPhase,
};
pub use http_jsonl::HttpJsonlWriter;
pub use metrics::GatewayMetrics;
pub use otlp::{TelemetryError, TelemetryRuntime, otlp_trace_layer};
pub(crate) use provider::ProviderAttemptContext;
pub(crate) use request::RequestObservation;
pub(crate) use usage::FirstOutputCapture;

#[cfg(test)]
mod tests;
