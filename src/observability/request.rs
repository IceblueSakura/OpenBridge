//! 单个已认证请求的 tracing、终态与低基数计数提交。
//!
//! request、user、credential 与 endpoint 事实只进入当前 span；已校验的 route、target、Upstream API、
//! Provider 和 Public Model 另由 Provider attempt 快照使用。共享状态只保存终态诊断、usage 与有限计数，
//! 并保证 finish/cancel 至多提交一次。

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

use super::{
    metrics::GatewayMetrics,
    provider::{
        AttemptOutcome, ProviderAttemptObservation, ProviderMetricExecution, ProviderMetricKey,
        observe_json_body,
    },
    usage::{TokenUsage, is_business_output, is_failed_terminal},
};

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
    protocol: Option<ApiProtocol>,
    public_model: Option<String>,
    streaming: bool,
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
    active_attempt: Option<ProviderAttemptObservation>,
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
    pub(crate) fn record_request(
        &self,
        protocol: ApiProtocol,
        public_model: &str,
        streaming: bool,
    ) {
        self.with_state(|state| {
            state.protocol = Some(protocol);
            state.public_model = Some(public_model.to_owned());
            state.streaming = streaming;
        });
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
        upstream_api: &str,
        provider: ProviderKind,
        bridged: bool,
    ) {
        // 创建 Provider 维度的 attempt 句柄，并把路由细节限制在当前 trace 内。
        let (protocol, public_model, streaming) = {
            let state = self.lock_state();
            (state.protocol, state.public_model.clone(), state.streaming)
        };
        let key = ProviderMetricKey::new(
            provider,
            route_id,
            upstream_target,
            upstream_api,
            public_model.as_deref().unwrap_or("unknown"),
            protocol,
            ProviderMetricExecution { streaming, bridged },
        );
        let previous = self.with_state_return(|state| state.active_attempt.take());
        if let Some(previous) = previous {
            previous.finish(AttemptOutcome::Cancelled);
        }
        let provider_attempt = self.inner.metrics.start_provider_attempt(key);
        self.with_state(|state| state.active_attempt = Some(provider_attempt));
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
        // 成功 status 只记录 response-ready；非成功 status 在 headers 边界收口 attempt。
        if status.is_success() {
            if let Some(provider_attempt) = self.active_attempt() {
                provider_attempt.record_response_ready();
            }
        } else if let Some(provider_attempt) = self.take_active_attempt() {
            provider_attempt.record_response_ready();
            provider_attempt.finish(AttemptOutcome::HttpFailed);
        }
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
        // 在没有 HTTP headers 的边界收口 Provider attempt。
        if let Some(provider_attempt) = self.take_active_attempt() {
            provider_attempt.finish(AttemptOutcome::TransportFailed);
        }
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
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_gateway_ttft(elapsed);
        }
    }

    /// 记录 body/SSE 异常；同一请求只保留首个失败类别。
    pub(crate) fn record_stream_failure(&self, kind: &'static str) {
        self.with_state(|state| {
            state.failure_kind.get_or_insert(kind);
        });
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_stream_failure();
        }
    }

    /// 从下游 JSON 或 SSE 中记录一次明确 usage。
    pub(super) fn record_usage(&self, usage: TokenUsage) {
        self.with_state(|state| {
            if let Some(current) = state.usage.as_mut() {
                current.merge(usage);
            } else {
                state.usage = Some(usage);
            }
        });
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_usage(usage);
        }
    }

    /// 记录原始上游 body 的首个非空 chunk。
    pub(crate) fn record_upstream_chunk(&self, chunk: &bytes::Bytes) {
        if !chunk.is_empty()
            && let Some(provider_attempt) = self.active_attempt()
        {
            provider_attempt.record_first_byte();
        }
    }

    /// 记录原始上游 JSON/SSE data 的业务输出、terminal 和 usage。
    pub(crate) fn record_upstream_value(&self, value: &serde_json::Value) {
        if is_business_output(value)
            && let Some(provider_attempt) = self.active_attempt()
        {
            provider_attempt.record_upstream_ttft();
        }
        if is_failed_terminal(value) {
            self.record_stream_failure("provider_terminal_failed");
        }
        if let Some(usage) = super::usage::extract_usage(value) {
            self.record_usage(usage);
        }
    }

    /// 记录一组已经完成 framing 的原始上游 SSE events。
    pub(crate) fn record_upstream_events(&self, events: &[crate::transport::sse::SseEvent]) {
        for event in events {
            if event.data() == "[DONE]" {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(event.data()) {
                self.record_upstream_value(&value);
            }
        }
    }

    /// 记录原始上游 body 到达 EOF。
    pub(crate) fn record_upstream_complete(&self) {
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_upstream_complete();
        }
    }

    /// 记录原始上游 body 或 framing 失败。
    pub(crate) fn record_upstream_failure(&self) {
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_stream_failure();
        }
    }

    /// 用透明 wrapper 观察成功的非 SSE 上游 body。
    pub(crate) fn observe_upstream_json_body(
        &self,
        body: axum::body::Body,
        max_json_body_bytes: usize,
    ) -> axum::body::Body {
        if self.active_attempt().is_some() {
            observe_json_body(body, self.clone(), max_json_body_bytes)
        } else {
            body
        }
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
                active_attempt: state.active_attempt.take(),
            }
        };

        // 累计低基数终态和 usage，再输出一条可由 OpenTelemetry trace 导出的总结 event。
        self.record_completion_metrics(&summary);
        if let Some(provider_attempt) = summary.active_attempt.as_ref() {
            let outcome = if summary.cancelled {
                AttemptOutcome::Cancelled
            } else if summary.failure_kind.is_some() {
                AttemptOutcome::StreamFailed
            } else {
                AttemptOutcome::Completed
            };
            provider_attempt.finish(outcome);
        }
        self.emit_completion(&summary);
    }

    /// 累计一次请求终态及其明确 usage。
    fn record_completion_metrics(&self, summary: &CompletionSummary) {
        // 先按取消、流失败、成功 HTTP 和其他 HTTP failure 归类请求终态。
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
        // 再把 Provider 明确返回的 token usage 以饱和加法写入低基数累计值。
        if let Some(usage) = summary.usage {
            counters.usage_observations.fetch_add(1, Ordering::Relaxed);
            saturating_add(&counters.input_tokens, usage.input_tokens.unwrap_or(0));
            saturating_add(&counters.output_tokens, usage.output_tokens.unwrap_or(0));
            saturating_add(&counters.total_tokens, usage.total_tokens.unwrap_or(0));
        }
    }

    /// 在请求 span 中输出不含业务正文和 credential 的终态 event。
    fn emit_completion(&self, summary: &CompletionSummary) {
        // 将内部状态收敛为稳定 outcome 名称，不把底层错误正文写入 event。
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
        // 只输出时间、尝试次数、终态类别和结构化 usage 计数。
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

    /// 返回请求开始后经过的毫秒数，用于 TTFT/TTFB 和总耗时观测。
    fn elapsed_ms(&self) -> u64 {
        self.inner.started.elapsed().as_millis() as u64
    }

    /// 在短暂持有状态锁的范围内更新本请求的观测状态。
    fn with_state(&self, update: impl FnOnce(&mut RequestState)) {
        update(&mut self.lock_state());
    }

    /// 获取请求状态锁，并将 poisoned mutex 视为可继续读取的本地状态。
    fn lock_state(&self) -> std::sync::MutexGuard<'_, RequestState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Clone)]
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
    active_attempt: Option<ProviderAttemptObservation>,
}

/// 以饱和加法累计不可信的外部 usage 值。
fn saturating_add(counter: &AtomicU64, value: u64) {
    // 外部 usage 即使异常巨大也只能让累计值饱和，不能回绕为较小数字。
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

impl RequestObservation {
    /// 读取当前尚未收口的 Provider attempt。
    fn active_attempt(&self) -> Option<ProviderAttemptObservation> {
        self.lock_state().active_attempt.clone()
    }

    /// 取出当前 Provider attempt，避免 HTTP/transport failure 重复收口。
    fn take_active_attempt(&self) -> Option<ProviderAttemptObservation> {
        self.with_state_return(|state| state.active_attempt.take())
    }

    /// 在状态锁内执行更新并返回结果。
    fn with_state_return<T>(&self, update: impl FnOnce(&mut RequestState) -> T) -> T {
        update(&mut self.lock_state())
    }
}
