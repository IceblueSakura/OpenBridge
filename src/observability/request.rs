//! Tracing, terminal-state, and low-cardinality counter submission for one authenticated request.
//!
//! Request, user, credential, and endpoint facts enter only the current span. Validated Route,
//! target, upstream operation, Provider, and Public Model dimensions are used separately by Provider
//! attempt snapshots. Shared state stores only terminal diagnostics, usage, and bounded counters,
//! and guarantees that finish/cancel is submitted at most once.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use http::StatusCode;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{core::OperationKind, provider::ProviderKind};

use super::{
    metrics::GatewayMetrics,
    provider::{
        AttemptOutcome, ProviderAttemptObservation, ProviderMetricExecution, ProviderMetricKey,
        observe_json_body,
    },
    usage::{TokenUsage, is_failed_terminal, is_generation_output},
};

/// Shared span, terminal-state, and usage observation handle for one authenticated request.
#[derive(Clone)]
pub(crate) struct RequestObservation {
    inner: Arc<RequestObservationInner>,
}

struct RequestObservationInner {
    metrics: GatewayMetrics,
    span: Span,
    started: Instant,
    first_body_byte_recorded: AtomicBool,
    first_output_recorded: AtomicBool,
    upstream_first_byte_pending: AtomicBool,
    upstream_first_output_pending: AtomicBool,
    state: Mutex<RequestState>,
}

#[derive(Default)]
struct RequestState {
    operation: Option<OperationKind>,
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
    /// Creates request observation and immediately increments started requests.
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
                first_body_byte_recorded: AtomicBool::new(false),
                first_output_recorded: AtomicBool::new(false),
                upstream_first_byte_pending: AtomicBool::new(false),
                upstream_first_output_pending: AtomicBool::new(false),
                state: Mutex::new(RequestState::default()),
            }),
        }
    }

    /// Records the downstream operation and Public Model in the request span.
    pub(crate) fn record_request(
        &self,
        operation: OperationKind,
        public_model: &str,
        streaming: bool,
    ) {
        self.with_state(|state| {
            state.operation = Some(operation);
            state.public_model = Some(public_model.to_owned());
            state.streaming = streaming;
        });
        self.inner
            .span
            .set_attribute("operation", operation.as_str());
        self.inner
            .span
            .set_attribute("public_model", public_model.to_owned());
        self.inner.span.set_attribute("streaming", streaming);
    }

    /// Records one actual upstream attempt and its compiled Route facts.
    pub(crate) fn record_attempt(
        &self,
        attempt: u64,
        route_id: &str,
        upstream_target: &str,
        upstream_operation: OperationKind,
        provider: ProviderKind,
        bridged: bool,
    ) {
        // Create a Provider-dimension attempt handle and keep Route details within the current trace.
        let (operation, public_model, streaming) = {
            let state = self.lock_state();
            (state.operation, state.public_model.clone(), state.streaming)
        };
        let key = ProviderMetricKey::new(
            provider,
            route_id,
            upstream_target,
            upstream_operation,
            public_model.as_deref().unwrap_or("unknown"),
            operation,
            ProviderMetricExecution { streaming, bridged },
        );
        let previous = self.with_state_return(|state| state.active_attempt.take());
        if let Some(previous) = previous {
            previous.finish(AttemptOutcome::Cancelled);
        }
        let attempt_index = attempt.min(i64::MAX as u64) as i64;
        let attempt_span = tracing::info_span!(
            parent: &self.inner.span,
            "provider_attempt",
            attempt = attempt_index,
            provider = %key.provider,
            route_id = %key.route_id,
            upstream_target = %key.upstream_target,
            upstream_operation = %key.upstream_operation,
            public_model = %key.public_model,
            operation = %key.operation,
            route_mode = %key.route_mode,
            streaming = key.streaming,
        );
        let provider_attempt = self.inner.metrics.start_provider_attempt(key, attempt_span);
        self.with_state(|state| state.active_attempt = Some(provider_attempt));
        self.inner
            .upstream_first_byte_pending
            .store(true, Ordering::Relaxed);
        self.inner
            .upstream_first_output_pending
            .store(true, Ordering::Relaxed);
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
                upstream_operation = upstream_operation.as_str(),
                ?provider,
                route_mode = if bridged { "bridged" } else { "native" },
                "upstream_attempt"
            );
        });
    }

    /// Records a redacted HTTP result for an attempt and counts non-success statuses.
    pub(crate) fn record_attempt_http_result(&self, attempt: u64, status: StatusCode) {
        // A successful status records response-ready; a non-success status finalizes the attempt at the headers boundary.
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

    /// Records a safe transport-failure category without an HTTP response.
    pub(crate) fn record_attempt_transport_failure(&self, attempt: u64, kind: &'static str) {
        // Finalize the Provider attempt at the boundary where no HTTP headers exist.
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

    /// Records one retry within the same candidate.
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

    /// Records rotation to another member in the same Provider pool after 429.
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

    /// Records one fallback to the next Route candidate.
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

    /// Records a candidate skipped because of cooldown.
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

    /// Marks that the handler generated response headers but has not completed the body.
    pub(crate) fn record_response_ready(&self, status: StatusCode) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.status = Some(status.as_u16());
            state.response_ready_ms = Some(elapsed);
        });
    }

    /// Marks the first non-empty downstream body chunk.
    pub(crate) fn record_first_body_byte(&self) {
        // Claim the one-time downstream first-byte update before taking the request-state lock.
        if self
            .inner
            .first_body_byte_recorded
            .swap(true, Ordering::Relaxed)
        {
            return;
        }
        let elapsed = self.elapsed_ms();
        self.with_state(|state| state.first_body_byte_ms = Some(elapsed));
    }

    /// Marks the first observable generation output without treating streaming metadata as TTFT.
    pub(super) fn record_first_output(&self) {
        // Claim the one-time downstream TTFT update before taking request and Provider locks.
        if self
            .inner
            .first_output_recorded
            .swap(true, Ordering::Relaxed)
        {
            return;
        }
        let elapsed = self.elapsed_ms();
        self.with_state(|state| state.first_output_ms = Some(elapsed));
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_gateway_ttft(elapsed);
        }
    }

    /// Marks a non-streaming JSON body unless its terminal was already classified as failed.
    pub(super) fn record_non_streaming_first_output(&self) {
        // Exclude an explicit failed or incomplete JSON terminal before claiming the one-shot sample.
        if self.lock_state().failure_kind.is_some() {
            return;
        }

        // Reuse the same downstream/provider one-shot boundary as token-bearing streams.
        self.record_first_output();
    }

    /// Returns whether downstream still needs its first observable generation-output sample.
    pub(super) fn needs_first_output(&self) -> bool {
        !self.inner.first_output_recorded.load(Ordering::Relaxed)
    }

    /// Returns whether a successful non-streaming JSON body exposes the first downstream output boundary.
    pub(crate) fn observes_non_streaming_generation_output(&self) -> bool {
        let state = self.lock_state();
        !state.streaming
            && matches!(
                state.operation,
                Some(OperationKind::ChatCompletions | OperationKind::Responses)
            )
    }

    /// Records a body/SSE failure; one request retains only its first failure category.
    pub(crate) fn record_stream_failure(&self, kind: &'static str) {
        self.with_state(|state| {
            state.failure_kind.get_or_insert(kind);
        });
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_stream_failure();
        }
    }

    /// Records explicit usage from downstream JSON or SSE.
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

    /// Records usage from a fully validated Embeddings success body without retaining its input or vectors.
    pub(crate) fn record_embedding_usage(&self, input_tokens: u64, total_tokens: u64) {
        self.record_usage(TokenUsage {
            input_tokens: Some(input_tokens),
            output_tokens: None,
            total_tokens: Some(total_tokens),
            cached_input_tokens: None,
            cache_write_input_tokens: None,
        });
    }

    /// Records the first non-empty chunk of the raw upstream body.
    pub(crate) fn record_upstream_chunk(&self, chunk: &bytes::Bytes) {
        // Claim the first byte once per Provider attempt and skip request locking for later chunks.
        if !chunk.is_empty()
            && self
                .inner
                .upstream_first_byte_pending
                .swap(false, Ordering::Relaxed)
            && let Some(provider_attempt) = self.active_attempt()
        {
            provider_attempt.record_first_byte();
        }
    }

    /// Records generated output, terminal, and usage from raw upstream JSON/SSE data.
    pub(crate) fn record_upstream_value(&self, value: &serde_json::Value) {
        // Claim TTFT once per Provider attempt while continuing to inspect terminal usage events.
        if self
            .inner
            .upstream_first_output_pending
            .load(Ordering::Relaxed)
            && is_generation_output(value)
            && self
                .inner
                .upstream_first_output_pending
                .swap(false, Ordering::Relaxed)
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

    /// Records a group of fully framed raw upstream SSE events.
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

    /// Records raw upstream body EOF.
    pub(crate) fn record_upstream_complete(&self) {
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_upstream_complete();
        }
    }

    /// Records raw upstream body or framing failure.
    pub(crate) fn record_upstream_failure(&self) {
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_stream_failure();
        }
    }

    /// Observes a successful non-SSE upstream body through a transparent wrapper.
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

    /// Submits the single terminal at normal EOF.
    pub(crate) fn finish(&self) {
        self.finish_with_cancel(false);
    }

    /// Submits the single cancellation terminal when downstream drops the body before EOF.
    pub(crate) fn cancel(&self) {
        self.finish_with_cancel(true);
    }

    /// Submits the single request terminal as normal or cancelled.
    fn finish_with_cancel(&self, cancelled: bool) {
        // Determine the single terminal under the lock and copy fields needed by the event.
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

        // Count the low-cardinality terminal and usage, then emit a summary event exportable by OpenTelemetry tracing.
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

    /// Counts one request terminal and its explicit usage.
    fn record_completion_metrics(&self, summary: &CompletionSummary) {
        // Classify the request terminal as cancellation, stream failure, successful HTTP, or other HTTP failure.
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
        // Add Provider-reported token usage to low-cardinality counters with saturation.
        if let Some(usage) = summary.usage {
            counters.usage_observations.fetch_add(1, Ordering::Relaxed);
            saturating_add(&counters.input_tokens, usage.input_tokens.unwrap_or(0));
            saturating_add(&counters.output_tokens, usage.output_tokens.unwrap_or(0));
            saturating_add(&counters.total_tokens, usage.total_tokens.unwrap_or(0));
        }
    }

    /// Emits a terminal event in the request span without business bodies or credentials.
    fn emit_completion(&self, summary: &CompletionSummary) {
        // Collapse internal state into a stable outcome name without writing underlying error text to the event.
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
        // Emit only timing, attempt counts, terminal category, and structured usage counters.
        let usage = summary.usage.unwrap_or_default();
        self.inner.span.set_attribute("outcome", outcome);
        record_optional_u64(
            &self.inner.span,
            "response_ready_ms",
            summary.response_ready_ms,
        );
        record_optional_u64(
            &self.inner.span,
            "first_body_byte_ms",
            summary.first_body_byte_ms,
        );
        record_optional_u64(&self.inner.span, "first_output_ms", summary.first_output_ms);
        set_u64_attribute(&self.inner.span, "duration_ms", summary.duration_ms);
        set_u64_attribute(&self.inner.span, "upstream_attempts", summary.attempts);
        set_u64_attribute(&self.inner.span, "upstream_retries", summary.retries);
        set_u64_attribute(
            &self.inner.span,
            "credential_rotations",
            summary.credential_rotations,
        );
        set_u64_attribute(&self.inner.span, "route_fallbacks", summary.fallbacks);
        set_u64_attribute(&self.inner.span, "cooldown_skips", summary.cooldown_skips);
        record_optional_u64(&self.inner.span, "input_tokens", usage.input_tokens);
        record_optional_u64(&self.inner.span, "output_tokens", usage.output_tokens);
        record_optional_u64(&self.inner.span, "total_tokens", usage.total_tokens);
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

    /// Returns milliseconds elapsed since request start for TTFT/TTFB and total-duration observation.
    fn elapsed_ms(&self) -> u64 {
        self.inner.started.elapsed().as_millis() as u64
    }

    /// Updates this request's observation state while briefly holding the state lock.
    fn with_state(&self, update: impl FnOnce(&mut RequestState)) {
        update(&mut self.lock_state());
    }

    /// Acquires the request-state lock and treats a poisoned mutex as locally usable state.
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

/// Accumulates untrusted external usage values with saturating addition.
fn saturating_add(counter: &AtomicU64, value: u64) {
    // Extremely large external usage may saturate the counter but cannot wrap to a smaller value.
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

/// Records a directly observed unsigned value only when it exists.
fn record_optional_u64(span: &Span, field: &'static str, value: Option<u64>) {
    if let Some(value) = value {
        set_u64_attribute(span, field, value);
    }
}

/// Records an unsigned observation using OTLP's signed integer domain without wrapping.
fn set_u64_attribute(span: &Span, field: &'static str, value: u64) {
    span.set_attribute(field, value.min(i64::MAX as u64) as i64);
}

impl RequestObservation {
    /// Reads the current open Provider attempt.
    fn active_attempt(&self) -> Option<ProviderAttemptObservation> {
        self.lock_state().active_attempt.clone()
    }

    /// Takes the current Provider attempt so HTTP/transport failure cannot finalize it twice.
    fn take_active_attempt(&self) -> Option<ProviderAttemptObservation> {
        self.with_state_return(|state| state.active_attempt.take())
    }

    /// Applies an update under the state lock and returns its result.
    fn with_state_return<T>(&self, update: impl FnOnce(&mut RequestState) -> T) -> T {
        update(&mut self.lock_state())
    }
}
