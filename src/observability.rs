//! Facade for request-lifecycle tracing and OpenTelemetry metrics.
//!
//! `metrics` owns fixed OpenTelemetry instruments, `provider` observes each compiled attempt,
//! `request` maintains per-request terminal state and tracing, and `usage` parses only usage
//! explicitly returned by the Provider. No module estimates tokens or records request/response bodies.

mod metrics;
mod otlp;
mod provider;
mod request;
mod usage;

pub use metrics::GatewayMetrics;
pub use otlp::{TelemetryError, TelemetryRuntime, otlp_trace_layer};
pub(crate) use provider::ProviderAttemptContext;
pub(crate) use request::RequestObservation;
pub(crate) use usage::FirstOutputCapture;

#[cfg(test)]
mod tests;
