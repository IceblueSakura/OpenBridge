//! Tracing, terminal-state, and OpenTelemetry submission for one authenticated request.
//!
//! Request content, user, credential, and endpoint values never enter telemetry. Validated Route,
//! target, upstream operation, Provider, and Public Model dimensions are used only by Provider
//! attempt instruments. Shared state stores pending lifecycle diagnostics and usage, and guarantees
//! that finish/cancel is submitted at most once.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use http::{HeaderMap, Method, StatusCode};
use opentelemetry::KeyValue;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{config::HttpLoggingConfig, core::OperationKind};

use super::{
    classification::{AttemptFailure, ErrorType, FailureStage, RequestFailure, RequestKind},
    http_jsonl::{HttpJsonlWriter, JsonlRecord},
    metrics::{GatewayMetrics, RequestMetricTerminal},
    provider::{
        AttemptOutcome, ProviderAttemptContext, ProviderAttemptObservation,
        ProviderMetricAttributes, observe_json_body,
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
    request_kind: RequestKind,
    request_id: String,
    http_logging: HttpLoggingConfig,
    jsonl_writer: Option<HttpJsonlWriter>,
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
    failure: Option<RequestFailure>,
    failure_kind: Option<&'static str>,
    finished: bool,
}

impl RequestObservation {
    /// Creates request observation and immediately increments started requests.
    #[cfg(test)]
    pub(crate) fn new(metrics: GatewayMetrics, span: Span) -> Self {
        Self::new_with_http_logging(
            metrics,
            span,
            RequestKind::Generation,
            "test-request".to_owned(),
            HttpLoggingConfig::default(),
            None,
        )
    }

    /// Creates request observation with one startup-frozen local HTTP logging policy.
    pub(crate) fn new_with_http_logging(
        metrics: GatewayMetrics,
        span: Span,
        request_kind: RequestKind,
        request_id: String,
        http_logging: HttpLoggingConfig,
        jsonl_writer: Option<HttpJsonlWriter>,
    ) -> Self {
        metrics.record_request_started(request_kind);
        Self {
            inner: Arc::new(RequestObservationInner {
                metrics,
                span,
                request_kind,
                request_id,
                http_logging,
                jsonl_writer,
                started: Instant::now(),
                first_body_byte_recorded: AtomicBool::new(false),
                first_output_recorded: AtomicBool::new(false),
                upstream_first_byte_pending: AtomicBool::new(false),
                upstream_first_output_pending: AtomicBool::new(false),
                state: Mutex::new(RequestState::default()),
            }),
        }
    }

    /// Emits authenticated downstream request headers when their independent switch is enabled.
    pub(crate) fn log_request_headers(&self, method: &Method, path: &str, headers: &HeaderMap) {
        if self.inner.http_logging.request_headers()
            && let Some(writer) = &self.inner.jsonl_writer
        {
            let record = JsonlRecord::request_headers(
                &self.inner.request_id,
                method.as_str(),
                path,
                headers,
            );
            writer.try_enqueue(record);
        }
    }

    /// Returns whether the request body needs a bounded local capture.
    pub(crate) fn logs_request_body(&self) -> bool {
        self.inner.http_logging.request_body()
    }

    /// Emits one authenticated downstream request-body snapshot.
    pub(crate) fn log_request_body(
        &self,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) {
        if self.inner.http_logging.request_body()
            && let Some(writer) = &self.inner.jsonl_writer
        {
            let record = JsonlRecord::request_body(
                &self.inner.request_id,
                bytes,
                total_bytes,
                complete,
                truncated,
            );
            writer.try_enqueue(record);
        }
    }

    /// Emits downstream response headers when their independent switch is enabled.
    pub(crate) fn log_response_headers(&self, status: StatusCode, headers: &HeaderMap) {
        if self.inner.http_logging.response_headers()
            && let Some(writer) = &self.inner.jsonl_writer
        {
            let record =
                JsonlRecord::response_headers(&self.inner.request_id, status.as_u16(), headers);
            writer.try_enqueue(record);
        }
    }

    /// Returns whether the response body needs a bounded local capture.
    pub(crate) fn logs_response_body(&self) -> bool {
        self.inner.http_logging.response_body()
    }

    /// Emits one downstream response-body snapshot.
    pub(crate) fn log_response_body(
        &self,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) {
        if self.inner.http_logging.response_body()
            && let Some(writer) = &self.inner.jsonl_writer
        {
            let record = JsonlRecord::response_body(
                &self.inner.request_id,
                bytes,
                total_bytes,
                complete,
                truncated,
            );
            writer.try_enqueue(record);
        }
    }

    /// Records the downstream operation and registry-validated Public Model after planning.
    pub(crate) fn record_planned_request(
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

    /// Retains the latest bounded request-level cause without changing the public response.
    pub(crate) fn record_request_failure(
        &self,
        error_type: ErrorType,
        stage: FailureStage,
        retryable: bool,
    ) {
        self.with_state(|state| {
            state.failure = Some(RequestFailure::terminal(error_type, stage, retryable));
        });
    }

    /// Records one actual upstream attempt and its compiled Route facts.
    pub(crate) fn record_attempt(&self, context: ProviderAttemptContext<'_>) {
        // Create a Provider-dimension attempt handle and keep Route details within the current trace.
        let (operation, public_model, streaming) = {
            let state = self.lock_state();
            (state.operation, state.public_model.clone(), state.streaming)
        };
        let attributes = ProviderMetricAttributes::new(
            &context,
            public_model.as_deref().unwrap_or("unknown"),
            operation,
            streaming,
        );
        let previous = self.with_state_return(|state| state.active_attempt.take());
        if let Some(previous) = previous {
            previous.finish(AttemptOutcome::Cancelled);
        }
        let attempt_index = context.attempt.min(i64::MAX as u64) as i64;
        let attempt_span = tracing::info_span!(
            parent: &self.inner.span,
            "provider_attempt",
            attempt = attempt_index,
            provider = %attributes.provider,
            route_id = %attributes.route_id,
            upstream_target = %attributes.upstream_target,
            upstream_operation = %attributes.upstream_operation,
            public_model = %attributes.public_model,
            operation = %attributes.operation,
            route_mode = %attributes.route_mode,
            streaming = attributes.streaming,
        );
        let provider_attempt = self
            .inner
            .metrics
            .start_provider_attempt(attributes, attempt_span);
        self.with_state(|state| state.active_attempt = Some(provider_attempt));
        self.inner
            .upstream_first_byte_pending
            .store(true, Ordering::Relaxed);
        self.inner
            .upstream_first_output_pending
            .store(true, Ordering::Relaxed);
        self.with_state(|state| state.attempts += 1);
        self.inner.span.in_scope(|| {
            tracing::info!(
                attempt = context.attempt,
                route_id = context.route_id,
                upstream_target = context.upstream_target,
                upstream_operation = context.upstream_operation.as_str(),
                provider = ?context.provider,
                route_mode = if context.bridged { "bridged" } else { "native" },
                "upstream_attempt"
            );
        });
    }

    /// Records a redacted HTTP result for an attempt and counts non-success statuses.
    pub(crate) fn record_attempt_http_result(
        &self,
        attempt: u64,
        status: StatusCode,
        failure: Option<AttemptFailure>,
    ) {
        // A successful status records response-ready; a non-success status finalizes the attempt at the headers boundary.
        if status.is_success() {
            if let Some(provider_attempt) = self.active_attempt() {
                provider_attempt.record_http_status(status.as_u16());
                provider_attempt.record_response_ready();
            }
        } else if let Some(provider_attempt) = self.take_active_attempt() {
            let failure = failure.expect("failed HTTP attempt requires diagnostic context");
            provider_attempt.record_http_status(status.as_u16());
            provider_attempt.record_response_ready();
            provider_attempt.record_failure(failure);
            provider_attempt.finish(AttemptOutcome::HttpFailed);
            self.with_state(|state| {
                state.failure = Some(failure.request_failure(FailureStage::Upstream));
            });
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
    pub(crate) fn record_attempt_transport_failure(&self, attempt: u64, failure: AttemptFailure) {
        // Finalize the Provider attempt at the boundary where no HTTP headers exist.
        if let Some(provider_attempt) = self.take_active_attempt() {
            provider_attempt.record_failure(failure);
            provider_attempt.finish(AttemptOutcome::TransportFailed);
        }
        self.with_state(|state| {
            state.failure = Some(failure.request_failure(FailureStage::Upstream));
        });
        self.inner.span.in_scope(|| {
            tracing::info!(
                attempt,
                failure_kind = failure.error_type.as_str(),
                "upstream_attempt_transport_failure"
            );
        });
    }

    /// Records one scheduled retry within the same candidate.
    pub(crate) fn record_retry(&self, reason: ErrorType, backoff: Duration) {
        self.inner.metrics.record_routing_event("retry", reason);
        self.with_state(|state| state.retries += 1);
        let scheduled_backoff_ms = backoff.as_millis().min(i64::MAX as u128) as i64;
        self.inner.span.add_event(
            "openbridge.retry",
            vec![
                KeyValue::new("reason", reason.as_str()),
                KeyValue::new("scheduled_backoff_ms", scheduled_backoff_ms),
            ],
        );
        self.inner.span.in_scope(|| {
            tracing::info!(
                reason = reason.as_str(),
                scheduled_backoff_ms,
                "upstream_retry"
            );
        });
    }

    /// Records rotation to another member in the same Provider pool after 429.
    pub(crate) fn record_credential_rotation(&self) {
        self.inner
            .metrics
            .record_routing_event("credential_rotation", ErrorType::UpstreamRateLimited);
        self.with_state(|state| state.credential_rotations += 1);
        self.inner.span.add_event(
            "openbridge.credential_rotation",
            vec![KeyValue::new(
                "reason",
                ErrorType::UpstreamRateLimited.as_str(),
            )],
        );
        self.inner
            .span
            .in_scope(|| tracing::info!("credential_rotated"));
    }

    /// Records one scheduled fallback to the next Route candidate.
    pub(crate) fn record_fallback(&self, reason: ErrorType, backoff: Duration) {
        self.inner
            .metrics
            .record_routing_event("route_fallback", reason);
        self.with_state(|state| state.fallbacks += 1);
        let scheduled_backoff_ms = backoff.as_millis().min(i64::MAX as u128) as i64;
        self.inner.span.add_event(
            "openbridge.fallback",
            vec![
                KeyValue::new("reason", reason.as_str()),
                KeyValue::new("scheduled_backoff_ms", scheduled_backoff_ms),
            ],
        );
        self.inner.span.in_scope(|| {
            tracing::info!(
                reason = reason.as_str(),
                scheduled_backoff_ms,
                "route_fallback"
            );
        });
    }

    /// Records a candidate skipped because of cooldown.
    pub(crate) fn record_cooldown_skip(&self, upstream_target: &str) {
        self.inner
            .metrics
            .record_routing_event("cooldown_skip", ErrorType::UpstreamUnavailable);
        self.with_state(|state| state.cooldown_skips += 1);
        self.inner.span.add_event(
            "openbridge.cooldown_skip",
            vec![
                KeyValue::new("reason", ErrorType::UpstreamUnavailable.as_str()),
                KeyValue::new("upstream_target", upstream_target.to_owned()),
            ],
        );
        self.inner
            .span
            .in_scope(|| tracing::info!(upstream_target, "cooldown_skip"));
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
    pub(crate) fn record_stream_failure(&self, error_type: ErrorType) {
        self.record_body_failure(error_type, FailureStage::Stream, true);
    }

    /// Records a gateway-owned Bridge failure without changing a completed Provider terminal.
    pub(crate) fn record_bridge_failure(&self) {
        self.record_body_failure(
            ErrorType::InvalidUpstreamResponse,
            FailureStage::Bridge,
            false,
        );
    }

    /// Records an error observed while delivering the downstream response body.
    pub(crate) fn record_downstream_failure(&self) {
        self.record_body_failure(
            ErrorType::DownstreamBodyError,
            FailureStage::DownstreamDelivery,
            true,
        );
    }

    /// Retains the first request cause and only then marks the Provider side when applicable.
    fn record_body_failure(&self, error_type: ErrorType, stage: FailureStage, mark_provider: bool) {
        let inserted = self.with_state_return(|state| {
            state.failure_kind.get_or_insert(error_type.as_str());
            if state.failure.is_some() {
                false
            } else {
                state.failure = Some(RequestFailure::terminal(error_type, stage, false));
                true
            }
        });
        if inserted
            && mark_provider
            && let Some(provider_attempt) = self.active_attempt()
        {
            provider_attempt.record_stream_failure(error_type);
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
            reasoning_output_tokens: None,
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
            self.record_stream_failure(ErrorType::ProviderTerminalFailed);
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
        self.record_stream_failure(ErrorType::InvalidUpstreamResponse);
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
                operation: state.operation,
                public_model: state.public_model.clone(),
                streaming: state.streaming,
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
                failure: state.failure,
                failure_kind: state.failure_kind,
                cancelled,
                active_attempt: state.active_attempt.take(),
            }
        };

        let outcome = request_outcome(&summary);
        let failure = terminal_failure(&summary, outcome, self.inner.request_kind);
        let recovery = request_recovery(&summary);

        // Submit the terminal directly to OpenTelemetry before closing its active Provider attempt.
        self.inner
            .metrics
            .record_request_terminal(RequestMetricTerminal {
                outcome,
                duration_ms: summary.duration_ms,
                response_ready_ms: summary.response_ready_ms,
                first_output_ms: summary.first_output_ms,
                request_kind: self.inner.request_kind,
                status: summary.status,
                failure,
                recovery,
                operation: summary.operation,
                public_model: summary.public_model.as_deref(),
                streaming: summary.streaming,
            });
        if let Some(provider_attempt) = summary.active_attempt.as_ref() {
            let outcome = provider_outcome_for_request(
                summary.cancelled,
                summary.failure.map(|failure| failure.stage),
            );
            provider_attempt.finish(outcome);
        }
        self.emit_completion(&summary, outcome, failure);
    }

    /// Emits a terminal event in the request span without business bodies or credentials.
    fn emit_completion(
        &self,
        summary: &CompletionSummary,
        outcome: &'static str,
        failure: Option<RequestFailure>,
    ) {
        // Emit only timing, attempt counts, terminal category, and structured usage counters.
        let usage = summary.usage.unwrap_or_default();
        self.inner.span.set_attribute("outcome", outcome);
        self.inner
            .span
            .set_attribute("recovery", request_recovery(summary));
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
        record_optional_u64(
            &self.inner.span,
            "reasoning_output_tokens",
            usage.reasoning_output_tokens,
        );
        record_optional_u64(&self.inner.span, "total_tokens", usage.total_tokens);
        if let Some(status) = summary.status {
            set_u64_attribute(
                &self.inner.span,
                "http.response.status_code",
                u64::from(status),
            );
        }
        if let Some(failure) = failure {
            self.inner
                .span
                .set_attribute("error.type", failure.error_type.as_str());
            self.inner
                .span
                .set_attribute("openbridge.failure.stage", failure.stage.as_str());
            self.inner
                .span
                .set_attribute("retryable", failure.retryable);
            self.inner
                .span
                .set_attribute("next_action", failure.next_action.as_str());
        }
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
                reasoning_output_tokens = usage.reasoning_output_tokens,
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
    operation: Option<OperationKind>,
    public_model: Option<String>,
    streaming: bool,
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
    failure: Option<RequestFailure>,
    failure_kind: Option<&'static str>,
    cancelled: bool,
    active_attempt: Option<ProviderAttemptObservation>,
}

/// Classifies one request terminal without exposing an underlying failure message.
fn request_outcome(summary: &CompletionSummary) -> &'static str {
    if summary.cancelled {
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
    }
}

/// Cancels an incomplete Bridge attempt while allowing its owner to preserve an observed upstream EOF.
fn provider_outcome_for_request(
    cancelled: bool,
    failure_stage: Option<FailureStage>,
) -> AttemptOutcome {
    if cancelled || failure_stage == Some(FailureStage::Bridge) {
        AttemptOutcome::Cancelled
    } else if matches!(
        failure_stage,
        Some(FailureStage::Stream | FailureStage::DownstreamDelivery)
    ) {
        AttemptOutcome::StreamFailed
    } else {
        AttemptOutcome::Completed
    }
}

/// Selects one terminal diagnostic only for a failed or cancelled observed request.
fn terminal_failure(
    summary: &CompletionSummary,
    outcome: &'static str,
    request_kind: RequestKind,
) -> Option<RequestFailure> {
    if outcome == "completed" {
        None
    } else {
        summary
            .failure
            .or_else(|| {
                summary.cancelled.then(|| {
                    RequestFailure::terminal(
                        ErrorType::ClientCancelled,
                        FailureStage::DownstreamDelivery,
                        false,
                    )
                })
            })
            .or_else(|| {
                summary.status.map(|status| {
                    if request_kind == RequestKind::Models && status == 404 {
                        RequestFailure::terminal(
                            ErrorType::UnknownModel,
                            FailureStage::Planning,
                            false,
                        )
                    } else if (400..500).contains(&status) {
                        RequestFailure::terminal(
                            ErrorType::InvalidRequest,
                            FailureStage::Analysis,
                            false,
                        )
                    } else {
                        // Any unclassified observed 5xx is gateway-owned; do not invent an upstream cause.
                        RequestFailure::terminal(
                            ErrorType::ConfigurationError,
                            FailureStage::Planning,
                            false,
                        )
                    }
                })
            })
    }
}

/// Collapses bounded routing counters into one low-cardinality recovery category.
fn request_recovery(summary: &CompletionSummary) -> &'static str {
    let used = [
        summary.retries > 0,
        summary.credential_rotations > 0,
        summary.fallbacks > 0,
    ];
    match used.into_iter().filter(|used| *used).count() {
        0 => "none",
        1 if used[0] => "retry",
        1 if used[1] => "credential_rotation",
        1 => "fallback",
        _ => "multiple",
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

#[cfg(test)]
mod tests {
    use super::{AttemptOutcome, FailureStage, provider_outcome_for_request};

    #[test]
    fn bridge_failure_cancels_only_an_incomplete_provider_attempt() {
        assert_eq!(
            provider_outcome_for_request(false, Some(FailureStage::Bridge)),
            AttemptOutcome::Cancelled
        );
        assert_eq!(
            provider_outcome_for_request(false, Some(FailureStage::Stream)),
            AttemptOutcome::StreamFailed
        );
        assert_eq!(
            provider_outcome_for_request(false, Some(FailureStage::DownstreamDelivery)),
            AttemptOutcome::StreamFailed
        );
        assert_eq!(
            provider_outcome_for_request(true, Some(FailureStage::Bridge)),
            AttemptOutcome::Cancelled
        );
    }
}
