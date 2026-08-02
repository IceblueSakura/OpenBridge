//! 进程内低基数累计值及其只读快照。
//!
//! 计数器不按 request、user、route 或 target 分组，供未来 metrics exporter 读取。

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// 可由未来 metrics exporter 读取的进程内低基数累计值。
#[derive(Clone, Default)]
pub struct GatewayMetrics {
    pub(super) inner: Arc<MetricCounters>,
}

/// `GatewayMetrics` 在同一时刻的只读快照。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GatewayMetricsSnapshot {
    /// 已认证请求开始总数。
    pub requests_started: u64,
    /// 2xx response body 正常结束总数。
    pub requests_completed: u64,
    /// 非 2xx response body 正常结束总数。
    pub requests_http_failed: u64,
    /// 2xx body、SSE framing 或协议 terminal 异常总数。
    pub requests_failed: u64,
    /// 下游在 body 结束前丢弃请求总数。
    pub requests_cancelled: u64,
    /// 实际发起的上游 attempt 总数。
    pub upstream_attempts: u64,
    /// 返回非 2xx HTTP 状态的上游 attempt 总数。
    pub upstream_http_failures: u64,
    /// 未取得 HTTP response 的上游 transport failure 总数。
    pub upstream_transport_failures: u64,
    /// 在同一候选内执行的 retry 总数。
    pub upstream_retries: u64,
    /// 429 后切换 credential pool 成员的总数。
    pub credential_rotations: u64,
    /// 进入下一 Route 候选的 fallback 总数。
    pub route_fallbacks: u64,
    /// 因 cooldown 跳过的候选总数。
    pub cooldown_skips: u64,
    /// 明确解析到 usage 的请求总数。
    pub usage_observations: u64,
    /// Provider 明确返回的输入 token 累计值。
    pub input_tokens: u64,
    /// Provider 明确返回的输出 token 累计值。
    pub output_tokens: u64,
    /// Provider 明确返回或可由输入输出相加得到的总 token 累计值。
    pub total_tokens: u64,
}

#[derive(Default)]
pub(super) struct MetricCounters {
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
    /// 返回不带高基数标签的累计值快照。
    pub fn snapshot(&self) -> GatewayMetricsSnapshot {
        // 使用 relaxed 读取独立单调计数；快照不承诺跨字段事务一致性。
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
