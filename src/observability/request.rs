//! Tracing, terminal-state, and low-cardinality counter submission for one authenticated request.
//!
//! Request, user, credential, and endpoint facts enter only the current span. Validated Route,
//! target, upstream operation, Provider, and Public Model dimensions are used separately by Provider
//! attempt snapshots. Shared state stores only terminal diagnostics, usage, and bounded counters,
//! and guarantees that finish/cancel is submitted at most once.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use http::StatusCode;
use tracing::Span;

use crate::{core::OperationKind, provider::ProviderKind};

use super::{
    metrics::GatewayMetrics,
    provider::{
        AttemptOutcome, ProviderAttemptObservation, ProviderMetricExecution, ProviderMetricKey,
        observe_json_body,
    },
    usage::{TokenUsage, is_business_output, is_failed_terminal},
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
        self.inner.span.record("operation", operation.as_str());
        self.inner.span.record("public_model", public_model);
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
        self.inner.span.record("status", status.as_u16());
    }

    /// Marks the first non-empty downstream body chunk.
    pub(crate) fn record_first_body_byte(&self) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.first_body_byte_ms.get_or_insert(elapsed);
        });
    }

    /// Marks the first text/tool increment in SSE without treating metadata events as TTFT.
    pub(super) fn record_first_output(&self) {
        let elapsed = self.elapsed_ms();
        self.with_state(|state| {
            state.first_output_ms.get_or_insert(elapsed);
        });
        if let Some(provider_attempt) = self.active_attempt() {
            provider_attempt.record_gateway_ttft(elapsed);
        }
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

    /// Records the first non-empty chunk of the raw upstream body.
    pub(crate) fn record_upstream_chunk(&self, chunk: &bytes::Bytes) {
        if !chunk.is_empty()
            && let Some(provider_attempt) = self.active_attempt()
        {
            provider_attempt.record_first_byte();
        }
    }

    /// Records business output, terminal, and usage from raw upstream JSON/SSE data.
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
