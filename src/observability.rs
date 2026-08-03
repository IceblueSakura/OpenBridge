//! 请求生命周期观测与进程内低基数累计值门面。
//!
//! `metrics` 维护无高基数标签的累计值，`provider` 维护按编译期 attempt 维度的性能与 usage 快照，
//! `request` 维护单请求终态与 tracing，`usage` 只解析 Provider 明确返回的 OpenAI-compatible usage。
//! 所有模块都不估算 token，也不记录请求或响应正文。

mod metrics;
mod provider;
mod request;
mod usage;

pub use metrics::{GatewayMetrics, GatewayMetricsSnapshot};
pub use provider::{ProviderMetricKey, ProviderMetricSnapshot, RateSnapshot, TimingSnapshot};
pub(crate) use request::RequestObservation;
pub(crate) use usage::UsageCapture;

#[cfg(test)]
mod tests;
