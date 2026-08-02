//! 单个已认证请求的 tracing、终态与低基数计数提交。
//!
//! 高基数 request、user、route 与 target 事实只进入当前 span；共享状态只保存终态诊断、
//! usage 与有限计数，并保证 finish/cancel 至多提交一次。

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use http::StatusCode;
use tracing::Span;

use crate::{core::ApiProtocol, provider::ProviderKind};

use super::{metrics::GatewayMetrics, usage::TokenUsage};

/// 单个已认证请求共享的 span、终态和 usage 观测句柄。
#[derive(Clone)]
pub(crate) struct RequestObservation {
    inner: Arc<RequestObservationInner>,
}

struct RequestObservationInner {
    metrics: GatewayMetrics,
    span: Span,
    started: Instant,
    state: Mutex<RequestState>,
}

#[derive(Default)]
struct RequestState {
    status: Option<u16>,
    response_ready_ms: Option<u64>,
    first_body_byte_ms: Option<u64>,
    first_output_ms: Option<u64>,
    attempts: u64,
    retries: u64,
    credential_rotations: u64,
    fallbacks: u64,
    cooldown_skips: u64,
    usage: Option<TokenUsage>,
    failure_kind: Option<&'static str>,
    finished: bool,
}

impl RequestObservation {
    /// 创建请求观测并立即累计已开始请求。
    pub(crate) fn new(metrics: GatewayMetrics, span: Span) -> Self {
        metrics
            .inner
            .requests_started
            .fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::new(RequestObservationInner {
                metrics,
                span,
                started: Instant::now(),
                state: Mutex::new(RequestState::default()),
            }),
        }
    }

    /// 把下游协议与 Public Model 记录到请求 span。
    pub(crate) fn record_request(&self, protocol: ApiProtocol, public_model: &str) {
        self.inner
            .span
            .record("protocol", tracing::field::debug(protocol));
        self.inner.span.record("public_model", public_model);
    }

    /// 记录一次真实上游 attempt 及其已编译路由事实。
    pub(crate) fn record_attempt(
        &self,
        attempt: u64,
        route_id: &str,
        upstream_target: &str,
        provider: ProviderKind,
        bridged: bool,
    ) {
        // 累计低基数 attempt，并把路由细节限制在当前 trace 内。
        self.inner
            .metrics
            .inner
            .upstream_attempts
            .fetch_add(1, Ordering::Relaxed);
        self.with_state(|state| state.attempts += 1);
        self.inner.span.in_scope(|| {
            tracing::info!(
                attempt,
                route_id,
                upstream_target,
                ?provider,
                route_mode = if bridged { "bridged" } else { "native" },
                "upstream_attempt"
            );
        });
    }

    /// 记录一次 attempt 的脱敏 HTTP 结果，并累计非成功状态。
    pub(crate) fn record_attempt_http_result(&self, attempt: u64, status: StatusCode) {
        if !status.is_success() {
            self.inner
                .metrics
                .inner
                .upstream_http_failures
                .fetch_add(1, Ordering::Relaxed);
        }
        self.inner.span.in_scope(|| {
            tracing::info!(
                attempt,
                status = status.as_u16(),
                "upstream_attempt_http_result"
            );
        });
    }

    /// 记录一次未取得 HTTP response 的安全 transport 失败类别。
    pub(crate) fn record_attempt_transport_failure(&self, attempt: u64, kind: &'static str) {
        self.inner
            .metrics
            .inner
            .upstream_transport_failures
            .fetch_add(1, Ordering::Relaxed);
        self.inner.span.in_scope(|| {
            tracing::info!(
                attempt,
                failure_kind = kind,
                "upstream_attempt_transport_failure"
            );
        });
    }

    /// 记录同一候选内的一次 retry。
    pub(crate) fn record_retry(&self) {
        self.inner
            .metrics
            .inner
            .upstream_retries
            .fetch_add(1, Ordering::Relaxed);
        self.with_state(|state| state.retries += 1);
        self.inner
            .span
            .in_scope(|| tracing::info!("upstream_retry"));
    }

    /// 记录 429 后在同一 Provider pool 内切换到另一个成员。
    pub(crate) fn record_credential_rotation(&self) {
        self.inner
            .metrics
            .inner
            .credential_rotations
            .fetch_add(1, Ordering::Relaxed);
        self.with_state(|state| state.credential_rotations += 1);
        self.inner
            .span
            .in_scope(|| tracing::info!("credential_rotated"));
    }

    /// 记录进入下一 Route 候选的一次 fallback。
    pub(crate) fn record_fallback(&self) {
        self.inner
            .metrics
            .inner
            .route_fallbacks
            .fetch_add(1, Ordering::Relaxed);
        self.with_state(|state| state.fallbacks += 1);
        self.inner
            .span
            .in_scope(|| tracing::info!("route_fallback"));
    }

    /// 记录因 cooldown 跳过一个候选。
    pub(crate) fn record_cooldown_skip(&self, upstream_target: &str) {
        self.inner
            .metrics
            .inner
            .cooldown_skips
            .fetch_add(1, Ordering::Relaxed);
        self.with_state(|state| state.cooldown_skips += 1);
        self.inner.span.in_scope(|| {
            tracing::info!(upstream_target, "cooldown_skip");
        });
    }

    /// 标记 handler 已生成 response headers，但尚未完成 body。
    pub(crate) fn record_response_ready(&self, status: StatusCode) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.status = Some(status.as_u16());
            state.response_ready_ms = Some(elapsed);
        });
        self.inner.span.record("status", status.as_u16());
    }

    /// 标记首个非空下游 body chunk。
    pub(crate) fn record_first_body_byte(&self) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.first_body_byte_ms.get_or_insert(elapsed);
        });
    }

    /// 标记 SSE 中首个 text/tool 增量，不把 metadata event 误当成 TTFT。
    pub(super) fn record_first_output(&self) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.first_output_ms.get_or_insert(elapsed);
        });
    }

    /// 记录 body/SSE 异常；同一请求只保留首个失败类别。
    pub(crate) fn record_stream_failure(&self, kind: &'static str) {
        self.with_state(|state| {
            state.failure_kind.get_or_insert(kind);
        });
    }

    /// 从下游 JSON 或 SSE 中记录一次明确 usage。
    pub(super) fn record_usage(&self, usage: TokenUsage) {
        self.with_state(|state| state.usage = Some(usage));
    }

    /// 正常 EOF 时提交唯一终态。
    pub(crate) fn finish(&self) {
        self.finish_with_cancel(false);
    }

    /// body 在 EOF 前被下游丢弃时提交唯一取消终态。
    pub(crate) fn cancel(&self) {
        self.finish_with_cancel(true);
    }

    /// 以正常或取消状态提交唯一请求终态。
    fn finish_with_cancel(&self, cancelled: bool) {
        // 在锁内确定唯一终态并复制 event 所需字段。
        let summary = {
            let mut state = self.lock_state();
            if state.finished {
                return;
            }
            state.finished = true;
            CompletionSummary {
                status: state.status,
                response_ready_ms: state.response_ready_ms,
                first_body_byte_ms: state.first_body_byte_ms,
                first_output_ms: state.first_output_ms,
                duration_ms: self.elapsed_ms(),
                attempts: state.attempts,
                retries: state.retries,
                credential_rotations: state.credential_rotations,
                fallbacks: state.fallbacks,
                cooldown_skips: state.cooldown_skips,
                usage: state.usage,
                failure_kind: state.failure_kind,
                cancelled,
            }
        };

        // 累计低基数终态和 usage，再输出一条可由 OpenTelemetry trace 导出的总结 event。
        self.record_completion_metrics(summary);
        self.emit_completion(summary);
    }

    /// 累计一次请求终态及其明确 usage。
    fn record_completion_metrics(&self, summary: CompletionSummary) {
        let counters = &self.inner.metrics.inner;
        if summary.cancelled {
            counters.requests_cancelled.fetch_add(1, Ordering::Relaxed);
        } else if summary.failure_kind.is_some() {
            counters.requests_failed.fetch_add(1, Ordering::Relaxed);
        } else if summary
            .status
            .is_some_and(|status| (200..300).contains(&status))
        {
            counters.requests_completed.fetch_add(1, Ordering::Relaxed);
        } else {
            counters
                .requests_http_failed
                .fetch_add(1, Ordering::Relaxed);
        }
        if let Some(usage) = summary.usage {
            counters.usage_observations.fetch_add(1, Ordering::Relaxed);
            saturating_add(&counters.input_tokens, usage.input_tokens.unwrap_or(0));
            saturating_add(&counters.output_tokens, usage.output_tokens.unwrap_or(0));
            saturating_add(&counters.total_tokens, usage.total_tokens.unwrap_or(0));
        }
    }

    /// 在请求 span 中输出不含业务正文和 credential 的终态 event。
    fn emit_completion(&self, summary: CompletionSummary) {
        let outcome = if summary.cancelled {
            "cancelled"
        } else if summary.failure_kind.is_some() {
            "failed"
        } else if summary
            .status
            .is_some_and(|status| (200..300).contains(&status))
        {
            "completed"
        } else {
            "http_failed"
        };
        let usage = summary.usage.unwrap_or_default();
        self.inner.span.in_scope(|| {
            tracing::info!(
                outcome,
                status = summary.status,
                response_ready_ms = summary.response_ready_ms,
                first_body_byte_ms = summary.first_body_byte_ms,
                first_output_ms = summary.first_output_ms,
                duration_ms = summary.duration_ms,
                upstream_attempts = summary.attempts,
                upstream_retries = summary.retries,
                credential_rotations = summary.credential_rotations,
                route_fallbacks = summary.fallbacks,
                cooldown_skips = summary.cooldown_skips,
                failure_kind = summary.failure_kind,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                total_tokens = usage.total_tokens,
                "downstream_request_completed"
            );
        });
    }

    fn elapsed_ms(&self) -> u64 {
        self.inner.started.elapsed().as_millis() as u64
    }

    fn with_state(&self, update: impl FnOnce(&mut RequestState)) {
        update(&mut self.lock_state());
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RequestState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Clone, Copy)]
struct CompletionSummary {
    status: Option<u16>,
    response_ready_ms: Option<u64>,
    first_body_byte_ms: Option<u64>,
    first_output_ms: Option<u64>,
    duration_ms: u64,
    attempts: u64,
    retries: u64,
    credential_rotations: u64,
    fallbacks: u64,
    cooldown_skips: u64,
    usage: Option<TokenUsage>,
    failure_kind: Option<&'static str>,
    cancelled: bool,
}

/// 以饱和加法累计不可信的外部 usage 值。
fn saturating_add(counter: &AtomicU64, value: u64) {
    // 外部 usage 即使异常巨大也只能让累计值饱和，不能回绕为较小数字。
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}
