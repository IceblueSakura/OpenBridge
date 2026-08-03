//! Provider attempt 的性能、usage 与 cache 遥测。
//!
//! 本模块把每次实际上游调用绑定到编译期 route、target、Upstream API、Provider 与协议，
//! 在原始 upstream body/SSE 边界记录时间和明确 usage，再在 attempt 终态写入进程内快照。
//! 指标不保存业务正文、credential、endpoint URL 或下游身份。

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::body::Body;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde::Serialize;
use serde_json::Value;

use crate::{core::ApiProtocol, provider::ProviderKind};

use super::{request::RequestObservation, usage::TokenUsage};

/// Provider 性能快照使用的有限、非敏感维度。
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderMetricKey {
    /// 编译期 Provider 名称。
    pub provider: String,
    /// 编译期 Route 标识。
    pub route_id: String,
    /// 编译期 Upstream Target 标识。
    pub upstream_target: String,
    /// 编译期 Upstream API 标识。
    pub upstream_api: String,
    /// 下游使用的 Public Model 名称。
    pub public_model: String,
    /// 下游请求协议。
    pub protocol: String,
    /// Native 或 Bridged 执行模式。
    pub route_mode: String,
    /// 请求是否要求 streaming response。
    pub streaming: bool,
}

impl ProviderMetricKey {
    /// 从受信编译期标识构造 Provider 性能维度。
    pub(super) fn new(
        provider: ProviderKind,
        route_id: &str,
        upstream_target: &str,
        upstream_api: &str,
        public_model: &str,
        protocol: Option<ApiProtocol>,
        execution: ProviderMetricExecution,
    ) -> Self {
        Self {
            provider: provider_name(provider).to_owned(),
            route_id: route_id.to_owned(),
            upstream_target: upstream_target.to_owned(),
            upstream_api: upstream_api.to_owned(),
            public_model: public_model.to_owned(),
            protocol: protocol.map(protocol_name).unwrap_or("unknown").to_owned(),
            route_mode: if execution.bridged {
                "bridged"
            } else {
                "native"
            }
            .to_owned(),
            streaming: execution.streaming,
        }
    }
}

/// Provider attempt 的执行模式上下文。
#[derive(Clone, Copy)]
pub(super) struct ProviderMetricExecution {
    /// 请求是否要求 streaming response。
    pub(super) streaming: bool,
    /// 当前 route 是否使用 Protocol Bridge。
    pub(super) bridged: bool,
}

/// 一个时间字段的 count/sum/min/max 聚合。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TimingSnapshot {
    /// 有效观测数量。
    pub count: u64,
    /// 所有有效观测的毫秒总和。
    pub sum_ms: u64,
    /// 有效观测的最小毫秒数。
    pub min_ms: Option<u64>,
    /// 有效观测的最大毫秒数。
    pub max_ms: Option<u64>,
}

impl TimingSnapshot {
    /// 加入一个有界的毫秒观测。
    fn record(&mut self, value: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_ms = self.sum_ms.saturating_add(value);
        self.min_ms = Some(self.min_ms.map_or(value, |current| current.min(value)));
        self.max_ms = Some(self.max_ms.map_or(value, |current| current.max(value)));
    }
}

/// output tokens/sec 的定点聚合；所有 milli 字段等于真实值乘以 1000。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RateSnapshot {
    /// 有效速度观测数量。
    pub count: u64,
    /// milli tokens/sec 的总和。
    pub sum_milli_tokens_per_second: u64,
    /// 最小 milli tokens/sec。
    pub min_milli_tokens_per_second: Option<u64>,
    /// 最大 milli tokens/sec。
    pub max_milli_tokens_per_second: Option<u64>,
}

impl RateSnapshot {
    /// 加入一个以 milli tokens/sec 表示的速度观测。
    fn record(&mut self, value: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_milli_tokens_per_second = self.sum_milli_tokens_per_second.saturating_add(value);
        self.min_milli_tokens_per_second = Some(
            self.min_milli_tokens_per_second
                .map_or(value, |current| current.min(value)),
        );
        self.max_milli_tokens_per_second = Some(
            self.max_milli_tokens_per_second
                .map_or(value, |current| current.max(value)),
        );
    }
}

/// 一个 Provider/Route 维度的 attempt 性能与 usage 快照。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProviderMetricSnapshot {
    /// 该快照的受信维度。
    pub key: ProviderMetricKey,
    /// 已完成收口的实际上游 attempt 数。
    pub attempts_started: u64,
    /// 上游 body 正常结束的 attempt 数。
    pub attempts_completed: u64,
    /// 返回非 2xx status 的 attempt 数。
    pub attempts_http_failed: u64,
    /// 未取得 HTTP response 的 transport failure 数。
    pub attempts_transport_failed: u64,
    /// 上游 body/SSE/协议失败的 attempt 数。
    pub attempts_stream_failed: u64,
    /// 上游 body 尚未完成即被取消的 attempt 数。
    pub attempts_cancelled: u64,
    /// 上游 response headers ready 的时间聚合。
    pub response_ready_ms: TimingSnapshot,
    /// 上游首个非空 body chunk 的时间聚合。
    pub upstream_first_byte_ms: TimingSnapshot,
    /// 上游首个 text/tool 业务输出的时间聚合。
    pub upstream_ttft_ms: TimingSnapshot,
    /// 下游观察到首个 text/tool 业务输出的时间聚合。
    pub gateway_ttft_ms: TimingSnapshot,
    /// 上游 body 生命周期时间聚合。
    pub duration_ms: TimingSnapshot,
    /// 从上游首个业务输出到 upstream body 完成的时间聚合。
    pub generation_duration_ms: TimingSnapshot,
    /// 根据明确 output usage 和 generation duration 计算的速度聚合。
    pub output_speed: RateSnapshot,
    /// 有明确 usage 的 attempt 数。
    pub usage_observations: u64,
    /// 明确返回输入 token 的 attempt 数，用于计算每请求平均输入 token。
    pub input_token_observations: u64,
    /// 明确返回输出 token 的 attempt 数，用于计算每请求平均输出 token。
    pub output_token_observations: u64,
    /// 明确返回总 token 的 attempt 数，用于计算每请求平均总 token。
    pub total_token_observations: u64,
    /// 明确 usage 的输入 token 累计值。
    pub input_tokens: u64,
    /// 明确 usage 的输出 token 累计值。
    pub output_tokens: u64,
    /// 明确 usage 的总 token 累计值。
    pub total_tokens: u64,
    /// 具有明确缓存字段的 usage attempt 数。
    pub cache_observations: u64,
    /// 明确返回 cache read token 字段的 usage attempt 数，作为命中率分母。
    pub cache_read_observations: u64,
    /// 明确报告缓存读取 token 的 attempt 数。
    pub cache_hit_requests: u64,
    /// 明确报告的缓存读取 token 累计值。
    pub cached_input_tokens: u64,
    /// 明确报告的缓存写入 token 累计值。
    pub cache_write_input_tokens: u64,
}

/// Provider attempt 的最终结果类别。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptOutcome {
    /// 上游 body 正常完成。
    Completed,
    /// 上游返回非 2xx HTTP status。
    HttpFailed,
    /// 未取得 HTTP response。
    TransportFailed,
    /// body、SSE framing 或协议 terminal 失败。
    StreamFailed,
    /// 上游 body 在完成前被取消。
    Cancelled,
}

impl AttemptOutcome {
    /// 返回稳定的 trace outcome 名称。
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::HttpFailed => "http_failed",
            Self::TransportFailed => "transport_failed",
            Self::StreamFailed => "stream_failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// 共享的 Provider snapshot 存储。
#[derive(Clone, Default)]
pub(super) struct ProviderMetrics {
    inner: Arc<Mutex<BTreeMap<ProviderMetricKey, ProviderMetricSnapshot>>>,
}

impl ProviderMetrics {
    /// 创建一个尚未收口的 Provider attempt 观测句柄。
    pub(super) fn start(&self, key: ProviderMetricKey) -> ProviderAttemptObservation {
        ProviderAttemptObservation {
            metrics: self.clone(),
            key,
            started: Instant::now(),
            state: Arc::new(Mutex::new(ProviderAttemptState::default())),
        }
    }

    /// 返回按维度排序的 Provider 快照。
    pub(super) fn snapshots(&self) -> Vec<ProviderMetricSnapshot> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// 将一个已收口的 attempt summary 合并到对应维度。
    fn record(&self, key: &ProviderMetricKey, summary: AttemptSummary) {
        let mut snapshots = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let snapshot = snapshots
            .entry(key.clone())
            .or_insert_with(|| ProviderMetricSnapshot {
                key: key.clone(),
                ..ProviderMetricSnapshot::default()
            });
        snapshot.attempts_started = snapshot.attempts_started.saturating_add(1);
        match summary.outcome {
            AttemptOutcome::Completed => {
                snapshot.attempts_completed = snapshot.attempts_completed.saturating_add(1)
            }
            AttemptOutcome::HttpFailed => {
                snapshot.attempts_http_failed = snapshot.attempts_http_failed.saturating_add(1)
            }
            AttemptOutcome::TransportFailed => {
                snapshot.attempts_transport_failed =
                    snapshot.attempts_transport_failed.saturating_add(1)
            }
            AttemptOutcome::StreamFailed => {
                snapshot.attempts_stream_failed = snapshot.attempts_stream_failed.saturating_add(1)
            }
            AttemptOutcome::Cancelled => {
                snapshot.attempts_cancelled = snapshot.attempts_cancelled.saturating_add(1)
            }
        }
        if let Some(value) = summary.response_ready_ms {
            snapshot.response_ready_ms.record(value);
        }
        if let Some(value) = summary.upstream_first_byte_ms {
            snapshot.upstream_first_byte_ms.record(value);
        }
        if let Some(value) = summary.upstream_ttft_ms {
            snapshot.upstream_ttft_ms.record(value);
        }
        if let Some(value) = summary.gateway_ttft_ms {
            snapshot.gateway_ttft_ms.record(value);
        }
        snapshot.duration_ms.record(summary.duration_ms);
        if let Some(value) = summary.generation_duration_ms {
            snapshot.generation_duration_ms.record(value);
        }
        if let Some(value) = summary.output_speed_milli_tokens_per_second {
            snapshot.output_speed.record(value);
        }
        if let Some(usage) = summary.usage {
            snapshot.usage_observations = snapshot.usage_observations.saturating_add(1);
            if let Some(input_tokens) = usage.input_tokens {
                snapshot.input_token_observations =
                    snapshot.input_token_observations.saturating_add(1);
                add_saturated(&mut snapshot.input_tokens, input_tokens);
            }
            if let Some(output_tokens) = usage.output_tokens {
                snapshot.output_token_observations =
                    snapshot.output_token_observations.saturating_add(1);
                add_saturated(&mut snapshot.output_tokens, output_tokens);
            }
            if let Some(total_tokens) = usage.total_tokens {
                snapshot.total_token_observations =
                    snapshot.total_token_observations.saturating_add(1);
                add_saturated(&mut snapshot.total_tokens, total_tokens);
            }
            if usage.cached_input_tokens.is_some() || usage.cache_write_input_tokens.is_some() {
                snapshot.cache_observations = snapshot.cache_observations.saturating_add(1);
            }
            if usage.cached_input_tokens.is_some() {
                snapshot.cache_read_observations =
                    snapshot.cache_read_observations.saturating_add(1);
            }
            if usage.cached_input_tokens.is_some_and(|value| value > 0) {
                snapshot.cache_hit_requests = snapshot.cache_hit_requests.saturating_add(1);
            }
            add_saturated(
                &mut snapshot.cached_input_tokens,
                usage.cached_input_tokens.unwrap_or(0),
            );
            add_saturated(
                &mut snapshot.cache_write_input_tokens,
                usage.cache_write_input_tokens.unwrap_or(0),
            );
        }
    }
}

/// 一个实际 Provider attempt 的生命周期观测句柄。
#[derive(Clone)]
pub(super) struct ProviderAttemptObservation {
    metrics: ProviderMetrics,
    key: ProviderMetricKey,
    started: Instant,
    state: Arc<Mutex<ProviderAttemptState>>,
}

#[derive(Default)]
struct ProviderAttemptState {
    response_ready_ms: Option<u64>,
    upstream_first_byte_ms: Option<u64>,
    upstream_ttft_ms: Option<u64>,
    gateway_ttft_ms: Option<u64>,
    upstream_completed_ms: Option<u64>,
    usage: Option<TokenUsage>,
    stream_failed: bool,
    finished: bool,
}

#[derive(Clone, Copy)]
struct AttemptSummary {
    outcome: AttemptOutcome,
    response_ready_ms: Option<u64>,
    upstream_first_byte_ms: Option<u64>,
    upstream_ttft_ms: Option<u64>,
    gateway_ttft_ms: Option<u64>,
    duration_ms: u64,
    generation_duration_ms: Option<u64>,
    output_speed_milli_tokens_per_second: Option<u64>,
    usage: Option<TokenUsage>,
}

impl ProviderAttemptObservation {
    /// 记录上游 response headers ready 的 attempt 相对时间。
    pub(super) fn record_response_ready(&self) {
        self.with_state(|state| {
            state
                .response_ready_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// 记录原始上游 body 的首个非空 chunk。
    pub(super) fn record_first_byte(&self) {
        self.with_state(|state| {
            state
                .upstream_first_byte_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// 记录原始上游 SSE 中首个 text/tool 业务输出。
    pub(super) fn record_upstream_ttft(&self) {
        self.with_state(|state| {
            state
                .upstream_ttft_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// 记录下游观察到的首个业务输出时间。
    pub(super) fn record_gateway_ttft(&self, elapsed_ms: u64) {
        self.with_state(|state| {
            state.gateway_ttft_ms.get_or_insert(elapsed_ms);
        });
    }

    /// 记录原始上游 body 正常到达 EOF。
    pub(super) fn record_upstream_complete(&self) {
        self.with_state(|state| {
            state
                .upstream_completed_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// 记录上游 body/SSE/协议失败，但把最终收口交给请求生命周期。
    pub(super) fn record_stream_failure(&self) {
        self.with_state(|state| state.stream_failed = true);
    }

    /// 合并一份明确 usage，保留已经得到的 cache 字段。
    pub(super) fn record_usage(&self, usage: TokenUsage) {
        self.with_state(|state| {
            if let Some(current) = state.usage.as_mut() {
                current.merge(usage);
            } else {
                state.usage = Some(usage);
            }
        });
    }

    /// 以指定结果收口 attempt，并保证同一 attempt 只写入一次 snapshot。
    pub(super) fn finish(&self, requested_outcome: AttemptOutcome) {
        let summary = {
            let mut state = self.lock_state();
            if state.finished {
                return;
            }
            state.finished = true;
            let outcome = if state.stream_failed {
                AttemptOutcome::StreamFailed
            } else if requested_outcome == AttemptOutcome::Cancelled
                && state.upstream_completed_ms.is_some()
            {
                AttemptOutcome::Completed
            } else {
                requested_outcome
            };
            let duration_ms = state
                .upstream_completed_ms
                .unwrap_or_else(|| self.started.elapsed().as_millis() as u64);
            let generation_duration_ms = state
                .upstream_ttft_ms
                .zip(state.upstream_completed_ms)
                .map(|(first_output, completed)| completed.saturating_sub(first_output));
            let output_speed_milli_tokens_per_second = state
                .usage
                .and_then(|usage| usage.output_tokens)
                .zip(generation_duration_ms)
                .and_then(|(output_tokens, generation_ms)| {
                    (generation_ms > 0).then(|| {
                        let scaled =
                            (u128::from(output_tokens) * 1_000_000) / u128::from(generation_ms);
                        scaled.min(u128::from(u64::MAX)) as u64
                    })
                });
            AttemptSummary {
                outcome,
                response_ready_ms: state.response_ready_ms,
                upstream_first_byte_ms: state.upstream_first_byte_ms,
                upstream_ttft_ms: state.upstream_ttft_ms,
                gateway_ttft_ms: state.gateway_ttft_ms,
                duration_ms,
                generation_duration_ms,
                output_speed_milli_tokens_per_second,
                usage: state.usage,
            }
        };
        self.metrics.record(&self.key, summary);
        tracing::info!(
            provider = %self.key.provider,
            route_id = %self.key.route_id,
            upstream_target = %self.key.upstream_target,
            upstream_api = %self.key.upstream_api,
            public_model = %self.key.public_model,
            protocol = %self.key.protocol,
            route_mode = %self.key.route_mode,
            streaming = self.key.streaming,
            outcome = summary.outcome.as_str(),
            response_ready_ms = summary.response_ready_ms,
            upstream_first_byte_ms = summary.upstream_first_byte_ms,
            upstream_ttft_ms = summary.upstream_ttft_ms,
            gateway_ttft_ms = summary.gateway_ttft_ms,
            duration_ms = summary.duration_ms,
            input_tokens = summary.usage.and_then(|usage| usage.input_tokens),
            output_tokens = summary.usage.and_then(|usage| usage.output_tokens),
            total_tokens = summary.usage.and_then(|usage| usage.total_tokens),
            cached_input_tokens = summary
                .usage
                .and_then(|usage| usage.cached_input_tokens),
            "provider_attempt_completed"
        );
    }

    /// 在状态锁内执行一个短小的更新。
    fn with_state(&self, update: impl FnOnce(&mut ProviderAttemptState)) {
        update(&mut self.lock_state());
    }

    /// 获取 attempt 状态锁，并允许本地继续处理 poisoned 状态。
    fn lock_state(&self) -> std::sync::MutexGuard<'_, ProviderAttemptState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 透明观察非 SSE 上游 body，并解析有界 JSON usage。
pub(super) fn observe_json_body(
    body: Body,
    observation: RequestObservation,
    max_json_body_bytes: usize,
) -> Body {
    Body::new(ProviderBodyObserver {
        body,
        observation,
        bytes: Vec::new(),
        limit: max_json_body_bytes,
        truncated: false,
        finished: false,
    })
}

struct ProviderBodyObserver {
    body: Body,
    observation: RequestObservation,
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    finished: bool,
}

impl HttpBody for ProviderBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    /// 透传原始 frame，并记录上游首字节与有界 JSON usage。
    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.as_mut().get_mut();
        match std::pin::Pin::new(&mut observer.body).poll_frame(context) {
            std::task::Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    observer.observation.record_upstream_chunk(chunk);
                    if !observer.truncated
                        && observer.bytes.len().saturating_add(chunk.len()) <= observer.limit
                    {
                        observer.bytes.extend_from_slice(chunk);
                    } else {
                        observer.bytes.clear();
                        observer.truncated = true;
                    }
                }
                std::task::Poll::Ready(Some(Ok(frame)))
            }
            std::task::Poll::Ready(Some(Err(error))) => {
                observer.observation.record_upstream_failure();
                observer.finished = true;
                std::task::Poll::Ready(Some(Err(error)))
            }
            std::task::Poll::Ready(None) => {
                if !observer.truncated
                    && let Ok(value) = serde_json::from_slice::<Value>(&observer.bytes)
                {
                    observer.observation.record_upstream_value(&value);
                }
                observer.observation.record_upstream_complete();
                observer.finished = true;
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    /// 只有真实 upstream EOF 或 error 后才报告 body 结束。
    fn is_end_stream(&self) -> bool {
        self.finished
    }

    /// 保留原始 body 的大小提示。
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

fn provider_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => "longcat",
        ProviderKind::DeepSeek => "deepseek",
        ProviderKind::MiMo => "mimo",
        ProviderKind::OpenRouter => "openrouter",
    }
}

fn protocol_name(protocol: ApiProtocol) -> &'static str {
    match protocol {
        ApiProtocol::ChatCompletions => "chat_completions",
        ApiProtocol::Responses => "responses",
    }
}

fn add_saturated(destination: &mut u64, value: u64) {
    *destination = destination.saturating_add(value);
}
