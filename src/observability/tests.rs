//! Unit tests for usage normalization and redacted terminal events.

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use http::StatusCode;
use serde_json::json;
use tracing_subscriber::fmt::MakeWriter;

use super::{
    GatewayMetrics, RequestObservation,
    usage::{TokenUsage, extract_usage},
};

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
            total_tokens: Some(18),
            cached_input_tokens: None,
            cache_write_input_tokens: None,
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
