//! In-process low-cardinality counters and read-only snapshots.
//!
//! Counters are not grouped by request, user, Route, or target and are intended for future metrics exporters.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde::Serialize;

use super::provider::{
    ProviderAttemptObservation, ProviderMetricKey, ProviderMetricSnapshot, ProviderMetrics,
};

/// In-process low-cardinality counters available to future metrics exporters.
#[derive(Clone, Default)]
pub struct GatewayMetrics {
    pub(super) inner: Arc<MetricCounters>,
}

/// Read-only snapshot of `GatewayMetrics` at one point in time.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GatewayMetricsSnapshot {
    /// Total observed authenticated requests started; metrics reads are excluded.
    pub requests_started: u64,
    /// Total 2xx response bodies completed normally.
    pub requests_completed: u64,
    /// Total non-2xx response bodies completed normally.
    pub requests_http_failed: u64,
    /// Total 2xx body, SSE-framing, or protocol-terminal failures.
    pub requests_failed: u64,
    /// Total requests dropped downstream before body completion.
    pub requests_cancelled: u64,
    /// Total upstream attempts actually started.
    pub upstream_attempts: u64,
    /// Total upstream attempts returning non-2xx HTTP status.
    pub upstream_http_failures: u64,
    /// Total upstream transport failures without an HTTP response.
    pub upstream_transport_failures: u64,
    /// Total retries performed within a candidate.
    pub upstream_retries: u64,
    /// Total credential-pool member rotations after 429.
    pub credential_rotations: u64,
    /// Total fallbacks to the next Route candidate.
    pub route_fallbacks: u64,
    /// Total candidates skipped because of cooldown.
    pub cooldown_skips: u64,
    /// Total requests with explicitly parsed usage.
    pub usage_observations: u64,
    /// Cumulative input tokens explicitly returned by Providers.
    pub input_tokens: u64,
    /// Cumulative output tokens explicitly returned by Providers.
    pub output_tokens: u64,
    /// Cumulative total tokens explicitly returned by Providers or derived from input plus output.
    pub total_tokens: u64,
}

#[derive(Default)]
pub(super) struct MetricCounters {
    pub(super) provider: ProviderMetrics,
    pub(super) requests_started: AtomicU64,
    pub(super) requests_completed: AtomicU64,
    pub(super) requests_http_failed: AtomicU64,
    pub(super) requests_failed: AtomicU64,
    pub(super) requests_cancelled: AtomicU64,
    pub(super) upstream_attempts: AtomicU64,
    pub(super) upstream_http_failures: AtomicU64,
    pub(super) upstream_transport_failures: AtomicU64,
    pub(super) upstream_retries: AtomicU64,
    pub(super) credential_rotations: AtomicU64,
    pub(super) route_fallbacks: AtomicU64,
    pub(super) cooldown_skips: AtomicU64,
    pub(super) usage_observations: AtomicU64,
    pub(super) input_tokens: AtomicU64,
    pub(super) output_tokens: AtomicU64,
    pub(super) total_tokens: AtomicU64,
}

impl GatewayMetrics {
    /// Returns performance snapshots ordered by Provider-attempt dimensions.
    pub fn provider_snapshots(&self) -> Vec<ProviderMetricSnapshot> {
        self.inner.provider.snapshots()
    }

    /// Creates an attempt-observation handle bound to Provider dimensions.
    pub(super) fn start_provider_attempt(
        &self,
        key: ProviderMetricKey,
    ) -> ProviderAttemptObservation {
        self.inner.provider.start(key)
    }

    /// Returns a counter snapshot without high-cardinality labels.
    pub fn snapshot(&self) -> GatewayMetricsSnapshot {
        // Read independent monotonic counters with relaxed ordering; the snapshot is not transactionally consistent across fields.
        GatewayMetricsSnapshot {
            requests_started: self.inner.requests_started.load(Ordering::Relaxed),
            requests_completed: self.inner.requests_completed.load(Ordering::Relaxed),
            requests_http_failed: self.inner.requests_http_failed.load(Ordering::Relaxed),
            requests_failed: self.inner.requests_failed.load(Ordering::Relaxed),
            requests_cancelled: self.inner.requests_cancelled.load(Ordering::Relaxed),
            upstream_attempts: self.inner.upstream_attempts.load(Ordering::Relaxed),
            upstream_http_failures: self.inner.upstream_http_failures.load(Ordering::Relaxed),
            upstream_transport_failures: self
                .inner
                .upstream_transport_failures
                .load(Ordering::Relaxed),
            upstream_retries: self.inner.upstream_retries.load(Ordering::Relaxed),
            credential_rotations: self.inner.credential_rotations.load(Ordering::Relaxed),
            route_fallbacks: self.inner.route_fallbacks.load(Ordering::Relaxed),
            cooldown_skips: self.inner.cooldown_skips.load(Ordering::Relaxed),
            usage_observations: self.inner.usage_observations.load(Ordering::Relaxed),
            input_tokens: self.inner.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.inner.output_tokens.load(Ordering::Relaxed),
            total_tokens: self.inner.total_tokens.load(Ordering::Relaxed),
        }
    }
}
