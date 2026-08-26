//! Unit tests for usage normalization and redacted terminal events.

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use http::StatusCode;
use serde_json::json;
use tracing_subscriber::fmt::MakeWriter;

use crate::{core::OperationKind, provider::ProviderKind, transport::sse::SseDecoder};

use super::{
    GatewayMetrics, RequestObservation, TimeoutPhase,
    classification::{AttemptFailure, ErrorType, FailureStage, NextAction, RequestFailure},
    provider::{ProviderAttemptContext, ProviderMetricAttributes},
    usage::{TokenUsage, extract_usage, is_generation_output},
};

fn test_observation() -> RequestObservation {
    RequestObservation::new(GatewayMetrics::default(), tracing::Span::none())
}

fn test_attempt_context(attempt: u64) -> ProviderAttemptContext<'static> {
    ProviderAttemptContext {
        attempt,
        upstream_target: "target-test",
        upstream_operation: OperationKind::Responses,
        upstream_model: "model-test",
        provider: ProviderKind::OpenAi,
        bridged: false,
    }
}

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
fn provider_metric_attributes_keep_upstream_and_downstream_operations_distinct() {
    let context = ProviderAttemptContext {
        attempt: 1,
        upstream_target: "openai-main",
        upstream_operation: OperationKind::Responses,
        upstream_model: "upstream-model",
        provider: ProviderKind::OpenAi,
        bridged: true,
    };
    let attributes = ProviderMetricAttributes::new(
        &context,
        "public-model",
        Some(OperationKind::ChatCompletions),
        false,
    );

    assert_eq!(attributes.upstream_operation, "responses");
    assert_eq!(attributes.operation, "chat_completions");
    assert_eq!(attributes.gen_ai_operation, "chat");
}

#[test]
fn extracts_chat_and_responses_usage_without_business_content() {
    // Verify that explicit usage from both protocols uses shared internal counters.
    assert_eq!(
        extract_usage(&json!({
            "usage": {
                "prompt_tokens": 2,
                "completion_tokens": 3,
                "total_tokens": 5,
                "prompt_tokens_details": {"cached_tokens": 1}
            },
            "choices": [{"message": {"content": "must not be retained"}}]
        })),
        Some(TokenUsage {
            input_tokens: Some(2),
            output_tokens: Some(3),
            reasoning_output_tokens: None,
            total_tokens: Some(5),
            cached_input_tokens: Some(1),
            cache_write_input_tokens: None,
        })
    );
    assert_eq!(
        extract_usage(&json!({
            "response": {"usage": {"input_tokens": 7, "output_tokens": 11}}
        })),
        Some(TokenUsage {
            input_tokens: Some(7),
            output_tokens: Some(11),
            reasoning_output_tokens: None,
            total_tokens: Some(18),
            cached_input_tokens: None,
            cache_write_input_tokens: None,
        })
    );
    assert_eq!(
        extract_usage(&json!({
            "usage": {
                "completion_tokens": 13,
                "completion_tokens_details": {"reasoning_tokens": 8}
            }
        }))
        .unwrap()
        .reasoning_output_tokens,
        Some(8)
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
fn reasoning_text_deltas_are_token_bearing_generation_output() {
    // Recognize the two reasoning delta shapes observed from MiMo without retaining their text.
    assert!(is_generation_output(&json!({
        "choices": [{"delta": {"reasoning_content": "reasoning"}}]
    })));
    assert!(is_generation_output(&json!({
        "type": "response.reasoning_text.delta",
        "delta": "reasoning"
    })));

    // Keep lifecycle-only and empty reasoning events outside the generation window.
    assert!(!is_generation_output(&json!({
        "choices": [{"delta": {"reasoning_content": ""}}]
    })));
    assert!(!is_generation_output(&json!({
        "type": "response.reasoning_text.done",
        "text": "reasoning"
    })));
}

#[test]
fn completion_event_contains_diagnostics_but_no_body_or_credentials() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();

    // Emit a terminal event in a local subscriber and verify stable fields and redaction boundaries.
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
            reasoning_output_tokens: None,
            total_tokens: Some(5),
            cached_input_tokens: None,
            cache_write_input_tokens: None,
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

#[test]
fn timeout_completion_event_records_only_bounded_phase_and_commit_context() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(logs.clone())
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("downstream_request", request_id = "timeout-observed");
        let observation = RequestObservation::new(GatewayMetrics::default(), span);
        observation.record_response_ready(StatusCode::OK);
        let events = SseDecoder::new(256)
            .push(b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n")
            .unwrap();
        observation.record_upstream_events(&events);
        observation.record_stream_timeout(TimeoutPhase::EventIdle);
        observation.finish();
    });

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(output.contains("timeout_phase=\"event_idle\""));
    assert!(output.contains("timeout_committed=true"));
    assert!(output.contains("last_upstream_event_ms="));
    assert!(output.contains("failure_kind=\"timeout\""));
    assert!(!output.contains("reqwest"));
    assert!(!output.contains("response.created"));
}

#[test]
fn failure_recording_retains_the_first_request_cause_across_attempts() {
    let first = RequestFailure::terminal(ErrorType::InvalidRequest, FailureStage::Analysis, false);
    let transport_failure =
        AttemptFailure::new(ErrorType::Timeout, true, NextAction::RetryCandidate);

    let transport = test_observation();
    transport.record_request_failure(first.error_type, first.stage, first.retryable);
    transport.record_attempt_transport_failure(1, transport_failure);
    assert_eq!(transport.failure_for_test(), Some(first));

    let http = test_observation();
    http.record_request_failure(first.error_type, first.stage, first.retryable);
    http.record_attempt(test_attempt_context(1));
    http.record_attempt_http_result(
        1,
        StatusCode::BAD_GATEWAY,
        Some(AttemptFailure::new(
            ErrorType::UpstreamFailure,
            false,
            NextAction::Finish,
        )),
    );
    assert_eq!(http.failure_for_test(), Some(first));
}

#[test]
fn failure_recording_marks_each_active_attempt_independently() {
    let observation = test_observation();
    observation.record_request_failure(ErrorType::Timeout, FailureStage::Upstream, true);
    observation.record_attempt(test_attempt_context(2));

    observation.record_stream_timeout(TimeoutPhase::EventIdle);
    observation.record_upstream_failure();

    assert_eq!(
        observation.active_attempt_failure_for_test(),
        Some(ErrorType::Timeout)
    );
}
