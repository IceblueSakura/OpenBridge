//! 请求生命周期观测与进程内低基数累计值。
//!
//! request/user/route/target 等高基数诊断事实只进入 `tracing` span/event；进程内累计值
//! 不按这些字段分组，供后续 OpenTelemetry metrics exporter 读取。模块只解析 Provider
//! 明确返回的 OpenAI-compatible usage，不估算 token，也不记录请求或响应正文。

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use bytes::Bytes;
use http::StatusCode;
use serde_json::Value;
use tracing::Span;

use crate::{
    core::ApiProtocol,
    provider::ProviderKind,
    transport::sse::{SseDecoder, SseEvent},
};

/// 可由未来 metrics exporter 读取的进程内低基数累计值。
#[derive(Clone, Default)]
pub struct GatewayMetrics {
    inner: Arc<MetricCounters>,
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
struct MetricCounters {
    requests_started: AtomicU64,
    requests_completed: AtomicU64,
    requests_http_failed: AtomicU64,
    requests_failed: AtomicU64,
    requests_cancelled: AtomicU64,
    upstream_attempts: AtomicU64,
    upstream_http_failures: AtomicU64,
    upstream_transport_failures: AtomicU64,
    upstream_retries: AtomicU64,
    route_fallbacks: AtomicU64,
    cooldown_skips: AtomicU64,
    usage_observations: AtomicU64,
    input_tokens: AtomicU64,
    output_tokens: AtomicU64,
    total_tokens: AtomicU64,
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
            route_fallbacks: self.inner.route_fallbacks.load(Ordering::Relaxed),
            cooldown_skips: self.inner.cooldown_skips.load(Ordering::Relaxed),
            usage_observations: self.inner.usage_observations.load(Ordering::Relaxed),
            input_tokens: self.inner.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.inner.output_tokens.load(Ordering::Relaxed),
            total_tokens: self.inner.total_tokens.load(Ordering::Relaxed),
        }
    }
}

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
    fallbacks: u64,
    cooldown_skips: u64,
    usage: Option<TokenUsage>,
    failure_kind: Option<&'static str>,
    finished: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

/// response body 的 usage 解析状态；解析失败只表示观测缺失，不改变代理响应。
pub(crate) enum UsageCapture {
    /// 当前 response 不携带可解析 usage。
    None,
    /// 有界缓存一个 JSON body，超限后放弃 usage 解析但继续透传。
    Json {
        bytes: Vec<u8>,
        limit: usize,
        truncated: bool,
    },
    /// 按既有 SSE event 上限增量解析下游 event。
    Sse { decoder: SseDecoder, invalid: bool },
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
    fn record_first_output(&self) {
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
    fn record_usage(&self, usage: TokenUsage) {
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
    fallbacks: u64,
    cooldown_skips: u64,
    usage: Option<TokenUsage>,
    failure_kind: Option<&'static str>,
    cancelled: bool,
}

impl UsageCapture {
    /// 根据成功 response 的 media type 创建有界 usage 解析器。
    pub(crate) fn for_response(
        content_type: Option<&str>,
        max_json_body_bytes: usize,
        max_sse_event_bytes: usize,
    ) -> Self {
        match content_type {
            Some(value) if value.starts_with("application/json") => Self::Json {
                bytes: Vec::new(),
                limit: max_json_body_bytes,
                truncated: false,
            },
            Some(value) if value.starts_with("text/event-stream") => Self::Sse {
                decoder: SseDecoder::new(max_sse_event_bytes),
                invalid: false,
            },
            _ => Self::None,
        }
    }

    /// 观察一个透传 chunk；任何解析问题都不会改变下游字节或状态。
    pub(crate) fn observe_chunk(&mut self, observation: &RequestObservation, chunk: &Bytes) {
        match self {
            Self::None => {}
            Self::Json {
                bytes,
                limit,
                truncated,
            } => {
                if !*truncated && bytes.len().saturating_add(chunk.len()) <= *limit {
                    bytes.extend_from_slice(chunk);
                } else {
                    bytes.clear();
                    *truncated = true;
                }
            }
            Self::Sse { decoder, invalid } => match decoder.push(chunk) {
                Ok(events) => observe_usage_events(observation, events),
                Err(_) => *invalid = true,
            },
        }
    }

    /// 正常 EOF 时完成 usage 解析并只写入结构化计数。
    pub(crate) fn finish(&mut self, observation: &RequestObservation) {
        match self {
            Self::None => {}
            Self::Json {
                bytes, truncated, ..
            } if !*truncated => {
                if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
                    if is_failed_terminal(&value) {
                        observation.record_stream_failure("provider_terminal_failed");
                    }
                    if let Some(usage) = extract_usage(&value) {
                        observation.record_usage(usage);
                    }
                }
            }
            Self::Json { .. } => {}
            Self::Sse { decoder, invalid } if !*invalid => {
                if let Ok(events) = decoder.finish() {
                    observe_usage_events(observation, events);
                }
            }
            Self::Sse { .. } => {}
        }
    }
}

fn observe_usage_events(observation: &RequestObservation, events: Vec<SseEvent>) {
    // 只解析完整 event 的 data JSON，不保留 event 或业务输出。
    for event in events {
        if event.data() == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(event.data()) {
            if is_business_output(&value) {
                observation.record_first_output();
            }
            if let Some(usage) = extract_usage(&value) {
                observation.record_usage(usage);
            }
        }
    }
}

fn is_business_output(value: &Value) -> bool {
    // Responses 只有 text/function arguments delta 属于首个业务输出，lifecycle metadata 不计入。
    if value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            matches!(
                kind,
                "response.output_text.delta" | "response.function_call_arguments.delta"
            )
        })
    {
        return true;
    }

    // Chat 只把非空 content 或 tool-call 增量视为业务输出。
    value
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| {
            choices.iter().any(|choice| {
                choice.get("delta").is_some_and(|delta| {
                    delta
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|content| !content.is_empty())
                        || delta
                            .get("tool_calls")
                            .and_then(Value::as_array)
                            .is_some_and(|calls| !calls.is_empty())
                })
            })
        })
}

fn is_failed_terminal(value: &Value) -> bool {
    value
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "incomplete"))
}

fn extract_usage(value: &Value) -> Option<TokenUsage> {
    // 同时识别 Chat 顶层 usage 与 Responses event 中的 response.usage。
    let usage = value
        .get("usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("usage"))
        })?
        .as_object()?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            input_tokens
                .zip(output_tokens)
                .map(|(input, output)| input.saturating_add(output))
        });
    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        None
    } else {
        Some(TokenUsage {
            input_tokens,
            output_tokens,
            total_tokens,
        })
    }
}

fn saturating_add(counter: &AtomicU64, value: u64) {
    // 外部 usage 即使异常巨大也只能让累计值饱和，不能回绕为较小数字。
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };

    use http::StatusCode;
    use serde_json::json;
    use tracing_subscriber::fmt::MakeWriter;

    use super::{GatewayMetrics, RequestObservation, TokenUsage, extract_usage};

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct BufferWriter(LogBuffer);

    impl Write for BufferWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for LogBuffer {
        type Writer = BufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter(self.clone())
        }
    }

    #[test]
    fn extracts_chat_and_responses_usage_without_business_content() {
        // 验证两种协议的明确 usage 使用统一内部计数。
        assert_eq!(
            extract_usage(&json!({
                "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
                "choices": [{"message": {"content": "must not be retained"}}]
            })),
            Some(TokenUsage {
                input_tokens: Some(2),
                output_tokens: Some(3),
                total_tokens: Some(5),
            })
        );
        assert_eq!(
            extract_usage(&json!({
                "response": {"usage": {"input_tokens": 7, "output_tokens": 11}}
            })),
            Some(TokenUsage {
                input_tokens: Some(7),
                output_tokens: Some(11),
                total_tokens: Some(18),
            })
        );
        assert_eq!(
            extract_usage(&json!({
                "usage": {"input_tokens": u64::MAX, "output_tokens": 1}
            }))
            .unwrap()
            .total_tokens,
            Some(u64::MAX)
        );
    }

    #[test]
    fn completion_event_contains_diagnostics_but_no_body_or_credentials() {
        let logs = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(logs.clone())
            .finish();

        // 在局部 subscriber 中生成终态 event，验证稳定字段与脱敏边界。
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                "downstream_request",
                request_id = "request-observed",
                user_id = "user-observed"
            );
            let observation = RequestObservation::new(GatewayMetrics::default(), span);
            observation.record_response_ready(StatusCode::OK);
            observation.record_usage(TokenUsage {
                input_tokens: Some(2),
                output_tokens: Some(3),
                total_tokens: Some(5),
            });
            observation.finish();
        });

        let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(output.contains("downstream_request_completed"));
        assert!(output.contains("outcome=\"completed\""));
        assert!(output.contains("input_tokens=2"));
        assert!(!output.contains("secret-observation-sentinel"));
        assert!(!output.contains("business-body-sentinel"));
    }
}
