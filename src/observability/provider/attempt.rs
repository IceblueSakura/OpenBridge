//! Provider-attempt timing, usage, and terminal observation.
//!
//! This module owns the lifecycle state for one actual upstream call. It receives trusted
//! Provider dimensions from the parent facade and submits only bounded timing, usage, and outcome
//! values; it never retains business bodies or credentials.

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use super::ProviderMetricAttributes;
use crate::observability::{
    classification::{AttemptFailure, ErrorType, NextAction, TimeoutPhase},
    metrics::GatewayMetrics,
    usage::TokenUsage,
};

/// Final result category for a Provider attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptOutcome {
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
    /// Returns the stable trace and metric outcome name.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::HttpFailed => "http_failed",
            Self::TransportFailed => "transport_failed",
            Self::StreamFailed => "stream_failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Lifecycle observation handle for one actual Provider attempt.
#[derive(Clone)]
pub(crate) struct ProviderAttemptObservation {
    metrics: GatewayMetrics,
    attributes: ProviderMetricAttributes,
    span: Span,
    started: Instant,
    state: Arc<Mutex<ProviderAttemptState>>,
}

#[derive(Default)]
struct ProviderAttemptState {
    http_status: Option<u16>,
    response_ready_ms: Option<u64>,
    upstream_first_byte_ms: Option<u64>,
    upstream_ttft_ms: Option<u64>,
    upstream_completed_ms: Option<u64>,
    usage: Option<TokenUsage>,
    failure: Option<AttemptFailure>,
    stream_failed: bool,
    finished: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct AttemptSummary {
    pub(crate) outcome: AttemptOutcome,
    pub(crate) http_status: Option<u16>,
    pub(crate) response_ready_ms: Option<u64>,
    pub(crate) upstream_first_byte_ms: Option<u64>,
    pub(crate) upstream_ttft_ms: Option<u64>,
    pub(crate) duration_ms: u64,
    pub(crate) generation_duration_ms: Option<u64>,
    pub(crate) output_tokens_per_second: Option<f64>,
    pub(crate) usage: Option<TokenUsage>,
    pub(crate) failure: Option<AttemptFailure>,
}

impl ProviderAttemptObservation {
    /// Creates one lifecycle handle after the attempt-start counter was recorded.
    pub(crate) fn new(
        metrics: GatewayMetrics,
        attributes: ProviderMetricAttributes,
        span: Span,
    ) -> Self {
        Self {
            metrics,
            attributes,
            span,
            started: Instant::now(),
            state: Arc::new(Mutex::new(ProviderAttemptState::default())),
        }
    }

    /// Records the exact upstream HTTP status whenever response headers exist.
    pub(crate) fn record_http_status(&self, status: u16) {
        self.with_state(|state| state.http_status = Some(status));
    }

    /// Records the attempt-relative time when upstream response headers are ready.
    pub(crate) fn record_response_ready(&self) {
        self.with_state(|state| {
            state
                .response_ready_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records the first non-empty chunk of the raw upstream body.
    pub(crate) fn record_first_byte(&self) {
        self.with_state(|state| {
            state
                .upstream_first_byte_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records the first token-bearing text/tool/reasoning output in raw upstream SSE.
    pub(crate) fn record_upstream_ttft(&self) {
        self.with_state(|state| {
            state
                .upstream_ttft_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records normal EOF of the raw upstream body.
    pub(crate) fn record_upstream_complete(&self) {
        self.with_state(|state| {
            state
                .upstream_completed_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records an upstream body/SSE/protocol failure while leaving finalization to the request lifecycle.
    pub(crate) fn record_stream_failure(&self, error_type: ErrorType) {
        self.with_state(|state| {
            if !state.stream_failed {
                state.stream_failed = true;
                state.failure.get_or_insert(AttemptFailure::new(
                    error_type,
                    false,
                    NextAction::Finish,
                ));
            }
        });
    }

    /// Attaches bounded timeout context to this attempt without storing transport details.
    pub(crate) fn record_timeout_context(
        &self,
        phase: TimeoutPhase,
        committed: bool,
        last_event_ms: Option<u64>,
    ) {
        self.span
            .set_attribute("openbridge.timeout.phase", phase.as_str());
        self.span
            .set_attribute("openbridge.timeout.committed", committed);
        if let Some(last_event_ms) = last_event_ms {
            self.span
                .set_attribute("openbridge.upstream.last_event_ms", last_event_ms as i64);
        }
    }

    /// Records the closed cause and action before finalizing a failed attempt.
    pub(crate) fn record_failure(&self, failure: AttemptFailure) {
        self.with_state(|state| {
            state.failure.get_or_insert(failure);
        });
    }

    #[cfg(test)]
    pub(crate) fn failure_for_test(&self) -> Option<ErrorType> {
        self.lock_state().failure.map(|failure| failure.error_type)
    }

    /// Merges explicit usage while preserving cache fields already collected.
    pub(crate) fn record_usage(&self, usage: TokenUsage) {
        self.with_state(|state| {
            if let Some(current) = state.usage.as_mut() {
                current.merge(usage);
            } else {
                state.usage = Some(usage);
            }
        });
    }

    /// Finalizes the attempt and submits its OpenTelemetry measurements at most once.
    pub(crate) fn finish(&self, requested_outcome: AttemptOutcome) {
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
            let output_tokens_per_second = state
                .usage
                .and_then(|usage| usage.output_tokens)
                .zip(generation_duration_ms)
                .and_then(|(output_tokens, generation_ms)| {
                    (generation_ms > 0)
                        .then(|| output_tokens as f64 * 1_000.0 / generation_ms as f64)
                });
            let failure = if outcome == AttemptOutcome::Completed {
                None
            } else {
                state.failure.or_else(|| {
                    (outcome == AttemptOutcome::Cancelled).then(|| {
                        AttemptFailure::new(ErrorType::ClientCancelled, false, NextAction::Finish)
                    })
                })
            };
            AttemptSummary {
                outcome,
                http_status: state.http_status,
                response_ready_ms: state.response_ready_ms,
                upstream_first_byte_ms: state.upstream_first_byte_ms,
                upstream_ttft_ms: state.upstream_ttft_ms,
                duration_ms,
                generation_duration_ms,
                output_tokens_per_second,
                usage: state.usage,
                failure,
            }
        };

        // Record only the reviewed terminal, timing, and explicit usage fields on the attempt span.
        self.span.set_attribute("outcome", summary.outcome.as_str());
        record_optional_u64(
            &self.span,
            "http.response.status_code",
            summary.http_status.map(u64::from),
        );
        record_optional_u64(&self.span, "response_ready_ms", summary.response_ready_ms);
        record_optional_u64(
            &self.span,
            "upstream_first_byte_ms",
            summary.upstream_first_byte_ms,
        );
        record_optional_u64(&self.span, "upstream_ttft_ms", summary.upstream_ttft_ms);
        set_u64_attribute(&self.span, "duration_ms", summary.duration_ms);
        record_optional_u64(
            &self.span,
            "generation_duration_ms",
            summary.generation_duration_ms,
        );
        let usage = summary.usage.unwrap_or_default();
        record_optional_u64(&self.span, "input_tokens", usage.input_tokens);
        record_optional_u64(&self.span, "output_tokens", usage.output_tokens);
        record_optional_u64(
            &self.span,
            "reasoning_output_tokens",
            usage.reasoning_output_tokens,
        );
        record_optional_u64(&self.span, "total_tokens", usage.total_tokens);
        record_optional_u64(&self.span, "cached_input_tokens", usage.cached_input_tokens);
        record_optional_u64(
            &self.span,
            "cache_write_input_tokens",
            usage.cache_write_input_tokens,
        );
        if let Some(failure) = summary.failure {
            self.span
                .set_attribute("error.type", failure.error_type.as_str());
            self.span.set_attribute("retryable", failure.retryable);
            self.span
                .set_attribute("next_action", failure.next_action.as_str());
        }

        // Submit the terminal directly to OpenTelemetry and preserve the content-free local event.
        self.metrics
            .record_provider_attempt(&self.attributes, summary);
        self.span.in_scope(|| {
            tracing::info!(
                provider = %self.attributes.provider,
                route_id = %self.attributes.route_id,
                upstream_target = %self.attributes.upstream_target,
                upstream_operation = %self.attributes.upstream_operation,
                public_model = %self.attributes.public_model,
                operation = %self.attributes.operation,
                route_mode = %self.attributes.route_mode,
                streaming = self.attributes.streaming,
                outcome = summary.outcome.as_str(),
                response_ready_ms = summary.response_ready_ms,
                upstream_first_byte_ms = summary.upstream_first_byte_ms,
                upstream_ttft_ms = summary.upstream_ttft_ms,
                duration_ms = summary.duration_ms,
                input_tokens = summary.usage.and_then(|usage| usage.input_tokens),
                output_tokens = summary.usage.and_then(|usage| usage.output_tokens),
                reasoning_output_tokens = summary
                    .usage
                    .and_then(|usage| usage.reasoning_output_tokens),
                total_tokens = summary.usage.and_then(|usage| usage.total_tokens),
                cached_input_tokens = summary
                    .usage
                    .and_then(|usage| usage.cached_input_tokens),
                "provider_attempt_completed"
            );
        });
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
