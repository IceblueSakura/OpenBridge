//! Facade for request-lifecycle observability and low-cardinality in-process counters.
//!
//! `metrics` maintains counters without high-cardinality labels, `provider` maintains performance
//! and usage snapshots by compile-time attempt, `request` maintains per-request terminal state and tracing,
//! and `usage` parses only usage explicitly returned by the Provider. No module estimates tokens or records
//! request or response bodies.

mod metrics;
mod provider;
mod request;
mod usage;

pub use metrics::{GatewayMetrics, GatewayMetricsSnapshot};
pub use provider::{ProviderMetricKey, ProviderMetricSnapshot, RateSnapshot, TimingSnapshot};
pub(crate) use request::RequestObservation;
pub(crate) use usage::FirstOutputCapture;

#[cfg(test)]
mod tests;
