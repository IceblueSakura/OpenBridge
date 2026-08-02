//! 下游 JSON 与 SSE 中明确 usage 和首个业务输出的有界解析。
//!
//! 解析失败只表示观测缺失，不改变代理字节或响应状态；缓存与 SSE event 均受现有上限约束。

use bytes::Bytes;
use serde_json::Value;

use crate::transport::sse::{SseDecoder, SseEvent};

use super::request::RequestObservation;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TokenUsage {
    pub(super) input_tokens: Option<u64>,
    pub(super) output_tokens: Option<u64>,
    pub(super) total_tokens: Option<u64>,
}

/// response body 的 usage 解析状态；解析失败只表示观测缺失，不改变代理响应。
pub(crate) enum UsageCapture {
    /// 当前 response 不携带可解析 usage。
    None,
    /// 有界缓存一个 JSON body，超限后放弃 usage 解析但继续透传。
    Json {
        bytes: Vec<u8>,
        limit: usize,
        truncated: bool,
    },
    /// 按既有 SSE event 上限增量解析下游 event。
    Sse { decoder: SseDecoder, invalid: bool },
}

impl UsageCapture {
    /// 根据成功 response 的 media type 创建有界 usage 解析器。
    pub(crate) fn for_response(
        content_type: Option<&str>,
        max_json_body_bytes: usize,
        max_sse_event_bytes: usize,
    ) -> Self {
        match content_type {
            Some(value) if value.starts_with("application/json") => Self::Json {
                bytes: Vec::new(),
                limit: max_json_body_bytes,
                truncated: false,
            },
            Some(value) if value.starts_with("text/event-stream") => Self::Sse {
                decoder: SseDecoder::new(max_sse_event_bytes),
                invalid: false,
            },
            _ => Self::None,
        }
    }

    /// 观察一个透传 chunk；任何解析问题都不会改变下游字节或状态。
    pub(crate) fn observe_chunk(&mut self, observation: &RequestObservation, chunk: &Bytes) {
        match self {
            Self::None => {}
            Self::Json {
                bytes,
                limit,
                truncated,
            } => {
                if !*truncated && bytes.len().saturating_add(chunk.len()) <= *limit {
                    bytes.extend_from_slice(chunk);
                } else {
                    bytes.clear();
                    *truncated = true;
                }
            }
            Self::Sse { decoder, invalid } => match decoder.push(chunk) {
                Ok(events) => observe_usage_events(observation, events),
                Err(_) => *invalid = true,
            },
        }
    }

    /// 正常 EOF 时完成 usage 解析并只写入结构化计数。
    pub(crate) fn finish(&mut self, observation: &RequestObservation) {
        match self {
            Self::None => {}
            Self::Json {
                bytes, truncated, ..
            } if !*truncated => {
                if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
                    if is_failed_terminal(&value) {
                        observation.record_stream_failure("provider_terminal_failed");
                    }
                    if let Some(usage) = extract_usage(&value) {
                        observation.record_usage(usage);
                    }
                }
            }
            Self::Json { .. } => {}
            Self::Sse { decoder, invalid } if !*invalid => {
                if let Ok(events) = decoder.finish() {
                    observe_usage_events(observation, events);
                }
            }
            Self::Sse { .. } => {}
        }
    }
}

/// 观察完整 SSE events 中的首个业务输出和明确 usage。
fn observe_usage_events(observation: &RequestObservation, events: Vec<SseEvent>) {
    // 只解析完整 event 的 data JSON，不保留 event 或业务输出。
    for event in events {
        if event.data() == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(event.data()) {
            if is_business_output(&value) {
                observation.record_first_output();
            }
            if let Some(usage) = extract_usage(&value) {
                observation.record_usage(usage);
            }
        }
    }
}

/// 判断 event 是否携带首个 text 或 function arguments 增量。
fn is_business_output(value: &Value) -> bool {
    // Responses 只有 text/function arguments delta 属于首个业务输出，lifecycle metadata 不计入。
    if value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            matches!(
                kind,
                "response.output_text.delta" | "response.function_call_arguments.delta"
            )
        })
    {
        return true;
    }

    // Chat 只把非空 content 或 tool-call 增量视为业务输出。
    value
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| {
            choices.iter().any(|choice| {
                choice.get("delta").is_some_and(|delta| {
                    delta
                        .get("content")
                        .and_then(Value::as_str)
                        .is_some_and(|content| !content.is_empty())
                        || delta
                            .get("tool_calls")
                            .and_then(Value::as_array)
                            .is_some_and(|calls| !calls.is_empty())
                })
            })
        })
}

/// 判断完整 JSON response 是否声明失败或未完整终态。
fn is_failed_terminal(value: &Value) -> bool {
    value
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "incomplete"))
}

/// 从 Chat 或 Responses JSON 形状中提取明确 usage。
pub(super) fn extract_usage(value: &Value) -> Option<TokenUsage> {
    // 同时识别 Chat 顶层 usage 与 Responses event 中的 response.usage。
    let usage = value
        .get("usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("usage"))
        })?
        .as_object()?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            input_tokens
                .zip(output_tokens)
                .map(|(input, output)| input.saturating_add(output))
        });
    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        None
    } else {
        Some(TokenUsage {
            input_tokens,
            output_tokens,
            total_tokens,
        })
    }
}
