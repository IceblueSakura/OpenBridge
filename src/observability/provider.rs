//! Provider-attempt lifecycle observation and trusted OpenTelemetry dimensions.
//!
//! This module binds each actual upstream call to compile-time Route, target, upstream model,
//! operation, Provider, and execution-mode attributes. It records timing and explicit usage at raw
//! upstream body/SSE boundaries, then submits one terminal to OpenTelemetry instruments. It never
//! retains business bodies, credentials, endpoint URLs, or downstream identities.

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::body::Body;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use opentelemetry::KeyValue;
use serde_json::Value;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{core::OperationKind, provider::ProviderKind};

use super::{metrics::GatewayMetrics, request::RequestObservation, usage::TokenUsage};

/// Borrowed compile-time facts for one actual Provider attempt.
pub(crate) struct ProviderAttemptContext<'a> {
    /// One-based attempt index within the downstream request.
    pub(crate) attempt: u64,
    /// Compiled Route identifier.
    pub(crate) route_id: &'a str,
    /// Compiled Upstream Target identifier.
    pub(crate) upstream_target: &'a str,
    /// Operation selected on the Upstream Target.
    pub(crate) upstream_operation: OperationKind,
    /// Provider-visible model selected by the compiled API.
    pub(crate) upstream_model: &'a str,
    /// Provider family owning the selected Target.
    pub(crate) provider: ProviderKind,
    /// Whether this attempt crosses the protocol bridge.
    pub(crate) bridged: bool,
}

/// Bounded, non-sensitive dimensions used by Provider OpenTelemetry measurements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProviderMetricAttributes {
    pub(super) provider: String,
    gen_ai_provider: String,
    pub(super) route_id: String,
    pub(super) upstream_target: String,
    pub(super) upstream_operation: String,
    pub(super) upstream_model: String,
    pub(super) public_model: String,
    pub(super) operation: String,
    pub(super) gen_ai_operation: String,
    pub(super) route_mode: String,
    pub(super) streaming: bool,
}

impl ProviderMetricAttributes {
    /// Builds Provider attributes from trusted compile-time identifiers.
    pub(super) fn new(
        context: &ProviderAttemptContext<'_>,
        public_model: &str,
        operation: Option<OperationKind>,
        streaming: bool,
    ) -> Self {
        Self {
            provider: provider_name(context.provider).to_owned(),
            gen_ai_provider: gen_ai_provider_name(context.provider).to_owned(),
            route_id: context.route_id.to_owned(),
            upstream_target: context.upstream_target.to_owned(),
            upstream_operation: context.upstream_operation.as_str().to_owned(),
            upstream_model: context.upstream_model.to_owned(),
            public_model: public_model.to_owned(),
            operation: operation
                .map(OperationKind::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            gen_ai_operation: operation
                .map(gen_ai_operation_name)
                .unwrap_or("unknown")
                .to_owned(),
            route_mode: if context.bridged { "bridged" } else { "native" }.to_owned(),
            streaming,
        }
    }

    /// Returns the full trusted attributes for OpenBridge-specific instruments.
    pub(super) fn openbridge_attributes(&self) -> Vec<KeyValue> {
        vec![
            KeyValue::new("gen_ai.provider.name", self.gen_ai_provider.clone()),
            KeyValue::new("gen_ai.operation.name", self.gen_ai_operation.clone()),
            KeyValue::new("gen_ai.request.model", self.upstream_model.clone()),
            KeyValue::new("gen_ai.request.stream", self.streaming),
            KeyValue::new("openbridge.provider.name", self.provider.clone()),
            KeyValue::new("openbridge.route.id", self.route_id.clone()),
            KeyValue::new("openbridge.upstream.target", self.upstream_target.clone()),
            KeyValue::new(
                "openbridge.upstream.operation",
                self.upstream_operation.clone(),
            ),
            KeyValue::new("openbridge.downstream.operation", self.operation.clone()),
            KeyValue::new("openbridge.public_model", self.public_model.clone()),
            KeyValue::new("openbridge.route.mode", self.route_mode.clone()),
        ]
    }

    /// Returns the standard GenAI attributes and a bounded error category when applicable.
    pub(super) fn gen_ai_attributes(&self, outcome: Option<AttemptOutcome>) -> Vec<KeyValue> {
        let mut attributes = self.openbridge_attributes();
        if let Some(outcome) = outcome.filter(|outcome| *outcome != AttemptOutcome::Completed) {
            attributes.push(KeyValue::new("error.type", outcome.as_str()));
        }
        attributes
    }
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
    /// Returns the stable trace and metric outcome name.
    pub(super) fn as_str(self) -> &'static str {
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
pub(super) struct ProviderAttemptObservation {
    metrics: GatewayMetrics,
    attributes: ProviderMetricAttributes,
    span: Span,
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
pub(super) struct AttemptSummary {
    pub(super) outcome: AttemptOutcome,
    pub(super) response_ready_ms: Option<u64>,
    pub(super) upstream_first_byte_ms: Option<u64>,
    pub(super) upstream_ttft_ms: Option<u64>,
    pub(super) gateway_ttft_ms: Option<u64>,
    pub(super) duration_ms: u64,
    pub(super) generation_duration_ms: Option<u64>,
    pub(super) output_tokens_per_second: Option<f64>,
    pub(super) usage: Option<TokenUsage>,
}

impl ProviderAttemptObservation {
    /// Creates one lifecycle handle after the attempt-start counter was recorded.
    pub(super) fn new(
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

    /// Records the first token-bearing text/tool/reasoning output in raw upstream SSE.
    pub(super) fn record_upstream_ttft(&self) {
        self.with_state(|state| {
            state
                .upstream_ttft_ms
                .get_or_insert(self.started.elapsed().as_millis() as u64);
        });
    }

    /// Records when downstream observes the first streaming delta or non-streaming JSON body.
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

    /// Finalizes the attempt and submits its OpenTelemetry measurements at most once.
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
            let output_tokens_per_second = state
                .usage
                .and_then(|usage| usage.output_tokens)
                .zip(generation_duration_ms)
                .and_then(|(output_tokens, generation_ms)| {
                    (generation_ms > 0)
                        .then(|| output_tokens as f64 * 1_000.0 / generation_ms as f64)
                });
            AttemptSummary {
                outcome,
                response_ready_ms: state.response_ready_ms,
                upstream_first_byte_ms: state.upstream_first_byte_ms,
                upstream_ttft_ms: state.upstream_ttft_ms,
                gateway_ttft_ms: state.gateway_ttft_ms,
                duration_ms,
                generation_duration_ms,
                output_tokens_per_second,
                usage: state.usage,
            }
        };

        // Record only the reviewed terminal, timing, and explicit usage fields on the attempt span.
        self.span.set_attribute("outcome", summary.outcome.as_str());
        record_optional_u64(&self.span, "response_ready_ms", summary.response_ready_ms);
        record_optional_u64(
            &self.span,
            "upstream_first_byte_ms",
            summary.upstream_first_byte_ms,
        );
        record_optional_u64(&self.span, "upstream_ttft_ms", summary.upstream_ttft_ms);
        record_optional_u64(&self.span, "gateway_ttft_ms", summary.gateway_ttft_ms);
        set_u64_attribute(&self.span, "duration_ms", summary.duration_ms);
        record_optional_u64(
            &self.span,
            "generation_duration_ms",
            summary.generation_duration_ms,
        );
        let usage = summary.usage.unwrap_or_default();
        record_optional_u64(&self.span, "input_tokens", usage.input_tokens);
        record_optional_u64(&self.span, "output_tokens", usage.output_tokens);
        record_optional_u64(&self.span, "total_tokens", usage.total_tokens);
        record_optional_u64(&self.span, "cached_input_tokens", usage.cached_input_tokens);
        record_optional_u64(
            &self.span,
            "cache_write_input_tokens",
            usage.cache_write_input_tokens,
        );

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

/// Maps concrete Provider adapters to the closest stable GenAI provider namespace.
fn gen_ai_provider_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::ChatGpt | ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => "longcat",
        ProviderKind::DeepSeek => "deepseek",
        ProviderKind::MiMo => "mimo",
        ProviderKind::OpenRouter => "openrouter",
    }
}

/// Maps concrete downstream protocols to the stable GenAI operation vocabulary.
fn gen_ai_operation_name(operation: OperationKind) -> &'static str {
    match operation {
        OperationKind::ChatCompletions | OperationKind::Responses => "chat",
        OperationKind::EmbeddingsCreate => "embeddings",
    }
}
