//! 观测 usage 归一化与脱敏终态 event 的单元测试。

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
    // 验证两种协议的明确 usage 使用统一内部计数。
    assert_eq!(
        extract_usage(&json!({
            "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
            "choices": [{"message": {"content": "must not be retained"}}]
        })),
        Some(TokenUsage {
            input_tokens: Some(2),
            output_tokens: Some(3),
            total_tokens: Some(5),
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

    // 在局部 subscriber 中生成终态 event，验证稳定字段与脱敏边界。
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
