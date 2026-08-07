//! OpenTelemetry instruments for gateway and Provider lifecycle measurements.
//!
//! Instruments use the SDK's native sum and histogram aggregation. Attribute sets contain only
//! bounded protocol, Provider, Route, target, model, outcome, and execution-mode values; they do
//! not contain downstream identity, credential, request ID, endpoint URL, or business content.

use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram, Meter, MeterProvider as _, noop::NoopMeterProvider},
};
use tracing::Span;

use crate::core::OperationKind;

use super::provider::{AttemptSummary, ProviderAttemptObservation, ProviderMetricAttributes};

const TOKEN_BOUNDARIES: &[f64] = &[
    1.0,
    4.0,
    16.0,
    64.0,
    256.0,
    1_024.0,
    4_096.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    16_777_216.0,
    67_108_864.0,
];
const DURATION_BOUNDARIES: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0, 30.0, 60.0,
    120.0,
];
const RATE_BOUNDARIES: &[f64] = &[
    0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1_000.0,
];

/// Cloneable OpenTelemetry instrument handles used by one gateway process.
#[derive(Clone)]
pub struct GatewayMetrics {
    request_started: Counter<u64>,
    request_completed: Counter<u64>,
    request_duration: Histogram<f64>,
    routing_events: Counter<u64>,
    provider_attempt_started: Counter<u64>,
    provider_attempt_completed: Counter<u64>,
    provider_operation_duration: Histogram<f64>,
    provider_response_ready: Histogram<f64>,
    provider_first_byte: Histogram<f64>,
    provider_ttft: Histogram<f64>,
    gateway_ttft: Histogram<f64>,
    provider_generation_duration: Histogram<f64>,
    provider_output_speed: Histogram<f64>,
    provider_token_usage: Histogram<u64>,
    provider_cache_read_tokens: Histogram<u64>,
    provider_cache_write_tokens: Histogram<u64>,
    provider_cache_requests: Counter<u64>,
}

impl Default for GatewayMetrics {
    fn default() -> Self {
        Self::from_meter(NoopMeterProvider::new().meter("openbridge"))
    }
}

impl GatewayMetrics {
    /// Builds the complete fixed instrument set from a runtime-owned meter.
    pub fn from_meter(meter: Meter) -> Self {
        // Build gateway request and routing counters with native monotonic sum aggregation.
        let request_started = meter
            .u64_counter("openbridge.downstream.request.started")
            .with_description("Number of authenticated downstream requests admitted.")
            .with_unit("{request}")
            .build();
        let request_completed = meter
            .u64_counter("openbridge.downstream.request.completed")
            .with_description("Number of downstream requests reaching one terminal outcome.")
            .with_unit("{request}")
            .build();
        let routing_events = meter
            .u64_counter("openbridge.routing.events")
            .with_description("Number of bounded retry, rotation, fallback, and cooldown events.")
            .with_unit("{event}")
            .build();

        // Build request and Provider duration histograms in semantic-convention seconds.
        let request_duration = duration_histogram(
            &meter,
            "openbridge.downstream.request.duration",
            "Elapsed downstream request-body lifecycle duration.",
        );
        let provider_operation_duration = duration_histogram(
            &meter,
            "gen_ai.client.operation.duration",
            "Elapsed Provider operation duration.",
        );
        let provider_response_ready = duration_histogram(
            &meter,
            "openbridge.provider.response_ready.duration",
            "Elapsed time until upstream response headers are ready.",
        );
        let provider_first_byte = duration_histogram(
            &meter,
            "openbridge.provider.first_byte.duration",
            "Elapsed time until the first non-empty upstream body frame.",
        );
        let provider_ttft = duration_histogram(
            &meter,
            "openbridge.provider.time_to_first_token",
            "Elapsed time until the first upstream generation output.",
        );
        let gateway_ttft = duration_histogram(
            &meter,
            "openbridge.gateway.time_to_first_output",
            "Elapsed time until the first downstream generation output.",
        );
        let provider_generation_duration = duration_histogram(
            &meter,
            "openbridge.provider.generation.duration",
            "Elapsed upstream generation duration after first output.",
        );

        // Build Provider attempt, usage, cache, and output-rate instruments.
        let provider_attempt_started = meter
            .u64_counter("openbridge.provider.attempt.started")
            .with_description("Number of actual Provider attempts started.")
            .with_unit("{attempt}")
            .build();
        let provider_attempt_completed = meter
            .u64_counter("openbridge.provider.attempt.completed")
            .with_description("Number of Provider attempts reaching one terminal outcome.")
            .with_unit("{attempt}")
            .build();
        let provider_token_usage = meter
            .u64_histogram("gen_ai.client.token.usage")
            .with_description("Provider-reported input and output token usage.")
            .with_unit("{token}")
            .with_boundaries(TOKEN_BOUNDARIES.to_vec())
            .build();
        let provider_cache_read_tokens = token_histogram(
            &meter,
            "openbridge.provider.cache.read.token.usage",
            "Provider-reported cache-read input tokens.",
        );
        let provider_cache_write_tokens = token_histogram(
            &meter,
            "openbridge.provider.cache.write.token.usage",
            "Provider-reported cache-write input tokens.",
        );
        let provider_cache_requests = meter
            .u64_counter("openbridge.provider.cache.requests")
            .with_description("Requests with an explicit Provider cache-read observation.")
            .with_unit("{request}")
            .build();
        let provider_output_speed = meter
            .f64_histogram("openbridge.provider.output.speed")
            .with_description("Output tokens per second after first Provider generation output.")
            .with_unit("{token}/s")
            .with_boundaries(RATE_BOUNDARIES.to_vec())
            .build();

        Self {
            request_started,
            request_completed,
            request_duration,
            routing_events,
            provider_attempt_started,
            provider_attempt_completed,
            provider_operation_duration,
            provider_response_ready,
            provider_first_byte,
            provider_ttft,
            gateway_ttft,
            provider_generation_duration,
            provider_output_speed,
            provider_token_usage,
            provider_cache_read_tokens,
            provider_cache_write_tokens,
            provider_cache_requests,
        }
    }

    /// Records one authenticated request entering the observed downstream lifecycle.
    pub(super) fn record_request_started(&self) {
        self.request_started.add(1, &[]);
    }

    /// Records one downstream terminal and its elapsed lifecycle duration.
    pub(super) fn record_request_completed(
        &self,
        outcome: &'static str,
        duration_ms: u64,
        operation: Option<OperationKind>,
        public_model: Option<&str>,
        streaming: bool,
    ) {
        // Count every terminal with one stable outcome dimension.
        self.request_completed
            .add(1, &[KeyValue::new("openbridge.request.outcome", outcome)]);

        // Record a distribution by only trusted request-planning dimensions.
        let mut attributes = vec![
            KeyValue::new("openbridge.request.outcome", outcome),
            KeyValue::new("gen_ai.request.stream", streaming),
        ];
        if let Some(operation) = operation {
            attributes.push(KeyValue::new(
                "openbridge.downstream.operation",
                operation.as_str(),
            ));
        }
        if let Some(public_model) = public_model {
            attributes.push(KeyValue::new(
                "openbridge.public_model",
                public_model.to_owned(),
            ));
        }
        self.request_duration
            .record(milliseconds_to_seconds(duration_ms), &attributes);
    }

    /// Records one bounded retry, credential rotation, fallback, or cooldown-skip event.
    pub(super) fn record_routing_event(&self, event: &'static str) {
        self.routing_events
            .add(1, &[KeyValue::new("openbridge.routing.event", event)]);
    }

    /// Creates an attempt observation and records the actual Provider call start.
    pub(super) fn start_provider_attempt(
        &self,
        attributes: ProviderMetricAttributes,
        span: Span,
    ) -> ProviderAttemptObservation {
        self.provider_attempt_started
            .add(1, &attributes.openbridge_attributes());
        ProviderAttemptObservation::new(self.clone(), attributes, span)
    }

    /// Records one finalized Provider attempt into SDK counters and histograms.
    pub(super) fn record_provider_attempt(
        &self,
        attributes: &ProviderMetricAttributes,
        summary: AttemptSummary,
    ) {
        // Record the terminal counter and custom timings with the full trusted Route dimensions.
        let mut openbridge_attributes = attributes.openbridge_attributes();
        openbridge_attributes.push(KeyValue::new(
            "openbridge.attempt.outcome",
            summary.outcome.as_str(),
        ));
        self.provider_attempt_completed
            .add(1, &openbridge_attributes);
        record_optional_duration(
            &self.provider_response_ready,
            summary.response_ready_ms,
            &openbridge_attributes,
        );
        record_optional_duration(
            &self.provider_first_byte,
            summary.upstream_first_byte_ms,
            &openbridge_attributes,
        );
        record_optional_duration(
            &self.provider_ttft,
            summary.upstream_ttft_ms,
            &openbridge_attributes,
        );
        record_optional_duration(
            &self.gateway_ttft,
            summary.gateway_ttft_ms,
            &openbridge_attributes,
        );
        self.provider_operation_duration.record(
            milliseconds_to_seconds(summary.duration_ms),
            &attributes.gen_ai_attributes(Some(summary.outcome)),
        );
        record_optional_duration(
            &self.provider_generation_duration,
            summary.generation_duration_ms,
            &openbridge_attributes,
        );
        if let Some(output_speed) = summary.output_tokens_per_second {
            self.provider_output_speed
                .record(output_speed, &openbridge_attributes);
        }

        // Record only explicit Provider usage and let the SDK aggregate each token/cache distribution.
        if let Some(usage) = summary.usage {
            if let Some(input_tokens) = usage.input_tokens {
                let mut token_attributes = attributes.gen_ai_attributes(None);
                token_attributes.push(KeyValue::new("gen_ai.token.type", "input"));
                self.provider_token_usage
                    .record(input_tokens, &token_attributes);
            }
            if let Some(output_tokens) = usage.output_tokens {
                let mut token_attributes = attributes.gen_ai_attributes(None);
                token_attributes.push(KeyValue::new("gen_ai.token.type", "output"));
                self.provider_token_usage
                    .record(output_tokens, &token_attributes);
            }
            if let Some(cached_input_tokens) = usage.cached_input_tokens {
                self.provider_cache_read_tokens
                    .record(cached_input_tokens, &openbridge_attributes);
                let mut cache_attributes = openbridge_attributes.clone();
                cache_attributes.push(KeyValue::new(
                    "openbridge.cache.result",
                    if cached_input_tokens > 0 {
                        "hit"
                    } else {
                        "miss"
                    },
                ));
                self.provider_cache_requests.add(1, &cache_attributes);
            }
            if let Some(cache_write_input_tokens) = usage.cache_write_input_tokens {
                self.provider_cache_write_tokens
                    .record(cache_write_input_tokens, &openbridge_attributes);
            }
        }
    }
}

/// Builds one duration histogram with shared seconds unit and latency buckets.
fn duration_histogram(
    meter: &Meter,
    name: &'static str,
    description: &'static str,
) -> Histogram<f64> {
    meter
        .f64_histogram(name)
        .with_description(description)
        .with_unit("s")
        .with_boundaries(DURATION_BOUNDARIES.to_vec())
        .build()
}

/// Builds one token histogram with the GenAI semantic-convention token buckets.
fn token_histogram(meter: &Meter, name: &'static str, description: &'static str) -> Histogram<u64> {
    meter
        .u64_histogram(name)
        .with_description(description)
        .with_unit("{token}")
        .with_boundaries(TOKEN_BOUNDARIES.to_vec())
        .build()
}

/// Records an optional millisecond duration as semantic-convention seconds.
fn record_optional_duration(
    histogram: &Histogram<f64>,
    duration_ms: Option<u64>,
    attributes: &[KeyValue],
) {
    if let Some(duration_ms) = duration_ms {
        histogram.record(milliseconds_to_seconds(duration_ms), attributes);
    }
}

/// Converts the internal monotonic millisecond observation to OTLP seconds.
fn milliseconds_to_seconds(duration_ms: u64) -> f64 {
    duration_ms as f64 / 1_000.0
}
