//! Performance, usage, and cache telemetry for Provider attempts.
//!
//! This module binds each actual upstream call to compile-time Route, target, Upstream API,
//! Provider, and operation dimensions. It records timing and explicit usage at raw upstream
//! body/SSE boundaries, then writes a process snapshot at attempt termination. Metrics store no
//! business bodies, credentials, endpoint URLs, or downstream identities.

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

use crate::{core::OperationKind, provider::ProviderKind};

use super::{request::RequestObservation, usage::TokenUsage};

/// Bounded, non-sensitive dimensions used by Provider performance snapshots.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderMetricKey {
    /// Compile-time Provider name.
    pub provider: String,
    /// Compile-time Route identifier.
    pub route_id: String,
    /// Compile-time Upstream Target identifier.
    pub upstream_target: String,
    /// Compile-time Upstream API identifier.
    pub upstream_api: String,
    /// Public Model name used downstream.
    pub public_model: String,
    /// Stable downstream operation name.
    pub operation: String,
    /// Native or Bridged execution mode.
    pub route_mode: String,
    /// Whether the request requires a streaming response.
    pub streaming: bool,
}

impl ProviderMetricKey {
    /// Builds Provider performance dimensions from trusted compile-time identifiers.
    pub(super) fn new(
        provider: ProviderKind,
        route_id: &str,
        upstream_target: &str,
        upstream_api: &str,
        public_model: &str,
        operation: Option<OperationKind>,
        execution: ProviderMetricExecution,
    ) -> Self {
        Self {
            provider: provider_name(provider).to_owned(),
            route_id: route_id.to_owned(),
            upstream_target: upstream_target.to_owned(),
            upstream_api: upstream_api.to_owned(),
            public_model: public_model.to_owned(),
            operation: operation
                .map(OperationKind::as_str)
                .unwrap_or("unknown")
                .to_owned(),
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

/// Execution-mode context for a Provider attempt.
#[derive(Clone, Copy)]
pub(super) struct ProviderMetricExecution {
    /// Whether the request requires a streaming response.
    pub(super) streaming: bool,
    /// Whether the current Route uses the Protocol Bridge.
    pub(super) bridged: bool,
}

/// Count/sum/min/max aggregate for one timing field.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TimingSnapshot {
    /// Number of valid observations.
    pub count: u64,
    /// Sum of all valid observations in milliseconds.
    pub sum_ms: u64,
    /// Minimum valid observation in milliseconds.
    pub min_ms: Option<u64>,
    /// Maximum valid observation in milliseconds.
    pub max_ms: Option<u64>,
}

impl TimingSnapshot {
    /// Adds a bounded millisecond observation.
    fn record(&mut self, value: u64) {
        self.count = self.count.saturating_add(1);
        self.sum_ms = self.sum_ms.saturating_add(value);
        self.min_ms = Some(self.min_ms.map_or(value, |current| current.min(value)));
        self.max_ms = Some(self.max_ms.map_or(value, |current| current.max(value)));
    }
}

/// Fixed-point aggregate for output tokens/sec; every milli field equals the real value multiplied by 1,000.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RateSnapshot {
    /// Number of valid speed observations.
    pub count: u64,
    /// Sum of milli tokens/sec.
    pub sum_milli_tokens_per_second: u64,
    /// Minimum milli tokens/sec.
    pub min_milli_tokens_per_second: Option<u64>,
    /// Maximum milli tokens/sec.
    pub max_milli_tokens_per_second: Option<u64>,
}

impl RateSnapshot {
    /// Adds a speed observation expressed in milli tokens/sec.
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

/// Performance and usage snapshot for one Provider/Route dimension.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProviderMetricSnapshot {
    /// Trusted dimensions of this snapshot.
    pub key: ProviderMetricKey,
    /// Actual upstream attempts fully finalized.
    pub attempts_started: u64,
    /// Attempts whose upstream body ended normally.
    pub attempts_completed: u64,
    /// Attempts returning a non-2xx status.
    pub attempts_http_failed: u64,
    /// Transport failures without an HTTP response.
    pub attempts_transport_failed: u64,
    /// Attempts failing at the upstream body, SSE, or protocol boundary.
    pub attempts_stream_failed: u64,
    /// Attempts cancelled before the upstream body completed.
    pub attempts_cancelled: u64,
    /// Timing aggregate until upstream response headers are ready.
    pub response_ready_ms: TimingSnapshot,
    /// Timing aggregate until the first non-empty upstream body chunk.
    pub upstream_first_byte_ms: TimingSnapshot,
    /// Timing aggregate until the first upstream text/tool business output.
    pub upstream_ttft_ms: TimingSnapshot,
    /// Timing aggregate until downstream observes the first text/tool business output.
    pub gateway_ttft_ms: TimingSnapshot,
    /// Timing aggregate for the upstream body lifetime.
    pub duration_ms: TimingSnapshot,
    /// Timing aggregate from first upstream business output to upstream body completion.
    pub generation_duration_ms: TimingSnapshot,
    /// Speed aggregate calculated from explicit output usage and generation duration.
    pub output_speed: RateSnapshot,
    /// Attempts with explicit usage.
    pub usage_observations: u64,
    /// Attempts explicitly returning input tokens, used for average input tokens per request.
    pub input_token_observations: u64,
    /// Attempts explicitly returning output tokens, used for average output tokens per request.
    pub output_token_observations: u64,
    /// Attempts explicitly returning total tokens, used for average total tokens per request.
    pub total_token_observations: u64,
    /// Cumulative input tokens from explicit usage.
    pub input_tokens: u64,
    /// Cumulative output tokens from explicit usage.
    pub output_tokens: u64,
    /// Cumulative total tokens from explicit usage.
    pub total_tokens: u64,
    /// Attempts whose usage includes explicit cache fields.
    pub cache_observations: u64,
    /// Attempts whose usage explicitly returns cache-read tokens, used as the hit-rate denominator.
    pub cache_read_observations: u64,
    /// Attempts explicitly reporting cache-read tokens.
    pub cache_hit_requests: u64,
    /// Cumulative explicitly reported cache-read tokens.
    pub cached_input_tokens: u64,
    /// Cumulative explicitly reported cache-write tokens.
    pub cache_write_input_tokens: u64,
}

/// Final result category for a Provider attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptOutcome {
    /// The upstream body completed normally.
    Completed,
    /// The upstream returned a non-2xx HTTP status.
    HttpFailed,
    /// No HTTP response was received.
    TransportFailed,
    /// The body, SSE framing, or protocol terminal failed.
    StreamFailed,
    /// The upstream body was cancelled before completion.
    Cancelled,
}

impl AttemptOutcome {
    /// Returns the stable trace outcome name.
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

/// Shared Provider snapshot storage.
#[derive(Clone, Default)]
pub(super) struct ProviderMetrics {
    inner: Arc<Mutex<BTreeMap<ProviderMetricKey, ProviderMetricSnapshot>>>,
}

impl ProviderMetrics {
    /// Creates an open Provider-attempt observation handle.
    pub(super) fn start(&self, key: ProviderMetricKey) -> ProviderAttemptObservation {
        ProviderAttemptObservation {
            metrics: self.clone(),
            key,
            started: Instant::now(),
            state: Arc::new(Mutex::new(ProviderAttemptState::default())),
        }
    }

    /// Returns Provider snapshots ordered by dimension.
    pub(super) fn snapshots(&self) -> Vec<ProviderMetricSnapshot> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// Merges a finalized attempt summary into its corresponding dimension.
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

/// Lifecycle observation handle for one actual Provider attempt.
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
    /// Records the attempt-relative time when upstream response headers are ready.
    pub(super) fn record_response_ready(&self) {
        self.with_state(|state| {
            state
                .response_ready_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records the first non-empty chunk of the raw upstream body.
    pub(super) fn record_first_byte(&self) {
        self.with_state(|state| {
            state
                .upstream_first_byte_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records the first text/tool business output in raw upstream SSE.
    pub(super) fn record_upstream_ttft(&self) {
        self.with_state(|state| {
            state
                .upstream_ttft_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records when downstream observes the first business output.
    pub(super) fn record_gateway_ttft(&self, elapsed_ms: u64) {
        self.with_state(|state| {
            state.gateway_ttft_ms.get_or_insert(elapsed_ms);
        });
    }

    /// Records normal EOF of the raw upstream body.
    pub(super) fn record_upstream_complete(&self) {
        self.with_state(|state| {
            state
                .upstream_completed_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records an upstream body/SSE/protocol failure while leaving finalization to the request lifecycle.
    pub(super) fn record_stream_failure(&self) {
        self.with_state(|state| state.stream_failed = true);
    }

    /// Merges explicit usage while preserving cache fields already collected.
    pub(super) fn record_usage(&self, usage: TokenUsage) {
        self.with_state(|state| {
            if let Some(current) = state.usage.as_mut() {
                current.merge(usage);
            } else {
                state.usage = Some(usage);
            }
        });
    }

    /// Finalizes the attempt with the given result and writes its snapshot at most once.
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
            operation = %self.key.operation,
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

    /// Performs a small update while holding the state lock.
    fn with_state(&self, update: impl FnOnce(&mut ProviderAttemptState)) {
        update(&mut self.lock_state());
    }

    /// Acquires the attempt-state lock and continues using locally poisoned state.
    fn lock_state(&self) -> std::sync::MutexGuard<'_, ProviderAttemptState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Transparently observes a non-SSE upstream body and parses bounded JSON usage.
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

impl ProviderBodyObserver {
    /// Parses usage and submits the Provider-attempt terminal when the raw upstream body completes.
    fn complete(&mut self) {
        // Prevent the final frame and later EOF from submitting the same attempt twice.
        if self.finished {
            return;
        }
        if !self.truncated
            && let Ok(value) = serde_json::from_slice::<Value>(&self.bytes)
        {
            self.observation.record_upstream_value(&value);
        }
        self.observation.record_upstream_complete();
        self.finished = true;
    }

    /// Submits a failure terminal at the upstream body-error boundary.
    fn fail(&mut self) {
        if self.finished {
            return;
        }
        self.observation.record_upstream_failure();
        self.finished = true;
    }
}

impl HttpBody for ProviderBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    /// Forwards raw frames and records the upstream first byte and bounded JSON usage.
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
                // A known-length upstream body can complete after the final frame without waiting for another EOF through nested wrappers.
                if observer.body.is_end_stream() {
                    observer.complete();
                }
                std::task::Poll::Ready(Some(Ok(frame)))
            }
            std::task::Poll::Ready(Some(Err(error))) => {
                observer.fail();
                std::task::Poll::Ready(Some(Err(error)))
            }
            std::task::Poll::Ready(None) => {
                observer.complete();
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    /// Reports body completion only after real upstream EOF or error.
    fn is_end_stream(&self) -> bool {
        self.finished
    }

    /// Preserves the raw body size hint.
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

fn provider_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::ChatGpt => "chatgpt",
        ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => "longcat",
        ProviderKind::DeepSeek => "deepseek",
        ProviderKind::MiMo => "mimo",
        ProviderKind::OpenRouter => "openrouter",
    }
}

fn add_saturated(destination: &mut u64, value: u64) {
    *destination = destination.saturating_add(value);
}
