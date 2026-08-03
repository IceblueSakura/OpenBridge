//! Bounded parsing of explicit usage and first business output in downstream JSON and SSE.
//!
//! Parse failures indicate missing observation only; they do not change proxy bytes or response
//! status. Caches and SSE events remain subject to existing limits.

use bytes::Bytes;
use serde_json::Value;

use crate::transport::sse::{SseDecoder, SseEvent};

use super::request::RequestObservation;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TokenUsage {
    pub(super) input_tokens: Option<u64>,
    pub(super) output_tokens: Option<u64>,
    pub(super) total_tokens: Option<u64>,
    pub(super) cached_input_tokens: Option<u64>,
    pub(super) cache_write_input_tokens: Option<u64>,
}

impl TokenUsage {
    /// Merges two usage values, preserving parsed fields and filling missing fields.
    pub(super) fn merge(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.or(other.input_tokens);
        self.output_tokens = self.output_tokens.or(other.output_tokens);
        self.total_tokens = self.total_tokens.or(other.total_tokens);
        self.cached_input_tokens = self.cached_input_tokens.or(other.cached_input_tokens);
        self.cache_write_input_tokens = self
            .cache_write_input_tokens
            .or(other.cache_write_input_tokens);
    }
}

/// Usage parsing state for a response body; parse failure indicates missing observation only.
pub(crate) enum UsageCapture {
    /// The current response carries no parseable usage.
    None,
    /// Bounded JSON-body cache; abandon usage parsing after the limit while continuing passthrough.
    Json {
        bytes: Vec<u8>,
        limit: usize,
        truncated: bool,
    },
    /// Incrementally parses downstream events using the existing SSE event limit.
    Sse { decoder: SseDecoder, invalid: bool },
}

impl UsageCapture {
    /// Creates a bounded usage parser from a successful response media type.
    pub(crate) fn for_response(
        content_type: Option<&str>,
        max_json_body_bytes: usize,
        max_sse_event_bytes: usize,
    ) -> Self {
        // Select bounded JSON, SSE, or no-observation behavior from the response media type.
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

    /// Observes a passthrough chunk; parse problems never change downstream bytes or status.
    pub(crate) fn observe_chunk(&mut self, observation: &RequestObservation, chunk: &Bytes) {
        // Update observation state only; never modify or block current downstream bytes.
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

    /// Completes usage parsing at normal EOF and writes structured counters only.
    pub(crate) fn finish(&mut self, observation: &RequestObservation) {
        // Flush the final JSON/SSE event at real EOF and record parseable usage.
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

/// Observes the first business output and explicit usage in complete SSE events.
fn observe_usage_events(observation: &RequestObservation, events: Vec<SseEvent>) {
    // Parse only complete event data JSON; retain neither the event nor business output.
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

/// Returns whether an event carries the first text or function-argument increment.
pub(super) fn is_business_output(value: &Value) -> bool {
    // For Responses, only text/function-argument deltas are business output; lifecycle metadata is excluded.
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

    // For Chat, only non-empty content or tool-call deltas count as business output.
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

/// Returns whether a complete JSON response declares failure or lacks a complete terminal state.
pub(super) fn is_failed_terminal(value: &Value) -> bool {
    value
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "incomplete"))
}

/// Extracts explicit usage from Chat or Responses JSON shapes.
pub(super) fn extract_usage(value: &Value) -> Option<TokenUsage> {
    // Recognize Chat top-level usage and response.usage in Responses events.
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
    let cached_input_tokens = usage
        .get("cached_input_tokens")
        .or_else(|| usage.get("cache_read_input_tokens"))
        .or_else(|| usage.get("cached_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| nested_usage_token(usage, "prompt_tokens_details", "cached_tokens"))
        .or_else(|| nested_usage_token(usage, "input_tokens_details", "cached_tokens"));
    let cache_write_input_tokens = usage
        .get("cache_write_input_tokens")
        .or_else(|| usage.get("cache_creation_input_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| nested_usage_token(usage, "prompt_tokens_details", "cache_creation_tokens"))
        .or_else(|| nested_usage_token(usage, "input_tokens_details", "cache_creation_tokens"));
    if input_tokens.is_none()
        && output_tokens.is_none()
        && total_tokens.is_none()
        && cached_input_tokens.is_none()
        && cache_write_input_tokens.is_none()
    {
        None
    } else {
        Some(TokenUsage {
            input_tokens,
            output_tokens,
            total_tokens,
            cached_input_tokens,
            cache_write_input_tokens,
        })
    }
}

/// Reads one explicit token field from a nested Provider usage details object.
fn nested_usage_token(
    usage: &serde_json::Map<String, Value>,
    object: &str,
    field: &str,
) -> Option<u64> {
    usage
        .get(object)
        .and_then(Value::as_object)
        .and_then(|details| details.get(field))
        .and_then(Value::as_u64)
}
