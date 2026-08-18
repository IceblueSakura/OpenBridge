//! Bounded parsing of explicit upstream usage and first observable downstream generation output.
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
    pub(super) reasoning_output_tokens: Option<u64>,
    pub(super) total_tokens: Option<u64>,
    pub(super) cached_input_tokens: Option<u64>,
    pub(super) cache_write_input_tokens: Option<u64>,
}

impl TokenUsage {
    /// Merges two usage values, preserving parsed fields and filling missing fields.
    pub(super) fn merge(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.or(other.input_tokens);
        self.output_tokens = self.output_tokens.or(other.output_tokens);
        self.reasoning_output_tokens = self
            .reasoning_output_tokens
            .or(other.reasoning_output_tokens);
        self.total_tokens = self.total_tokens.or(other.total_tokens);
        self.cached_input_tokens = self.cached_input_tokens.or(other.cached_input_tokens);
        self.cache_write_input_tokens = self
            .cache_write_input_tokens
            .or(other.cache_write_input_tokens);
    }
}

/// First-output parsing state for a downstream response body.
pub(crate) enum FirstOutputCapture {
    /// The current response does not expose a generation-output timing boundary.
    None,
    /// Records the first non-empty successful JSON chunk for non-streaming generation responses.
    Json,
    /// Incrementally parses downstream events only until the first generated delta.
    Sse { decoder: SseDecoder, invalid: bool },
}

impl FirstOutputCapture {
    /// Creates a bounded first-output parser from a successful response media type.
    pub(crate) fn for_response(
        content_type: Option<&str>,
        max_sse_event_bytes: usize,
        observe_non_streaming_json: bool,
    ) -> Self {
        // Select bounded SSE, non-streaming JSON, or no-observation behavior from the response contract.
        match content_type {
            Some(value) if value.starts_with("text/event-stream") => Self::Sse {
                decoder: SseDecoder::new(max_sse_event_bytes),
                invalid: false,
            },
            Some(value) if observe_non_streaming_json && value.starts_with("application/json") => {
                Self::Json
            }
            _ => Self::None,
        }
    }

    /// Observes a passthrough chunk until TTFT; parse problems never change downstream bytes or status.
    pub(crate) fn observe_chunk(&mut self, observation: &RequestObservation, chunk: &Bytes) {
        // Decode only while TTFT is unknown and never modify or block current downstream bytes.
        match self {
            Self::None => {}
            Self::Json if chunk.is_empty() || !observation.needs_first_output() => {}
            Self::Json => observation.record_non_streaming_first_output(),
            Self::Sse { .. } if !observation.needs_first_output() => {}
            Self::Sse { decoder, invalid } => match decoder.push(chunk) {
                Ok(events) => observe_first_output_events(observation, events),
                Err(_) => *invalid = true,
            },
        }
    }

    /// Flushes a final partial SSE event at normal EOF when TTFT is still unknown.
    pub(crate) fn finish(&mut self, observation: &RequestObservation) {
        // Flush only the final SSE event needed for a still-missing first-output observation.
        match self {
            Self::None | Self::Json => {}
            Self::Sse { .. } if !observation.needs_first_output() => {}
            Self::Sse { decoder, invalid } if !*invalid => {
                if let Ok(events) = decoder.finish() {
                    observe_first_output_events(observation, events);
                }
            }
            Self::Sse { .. } => {}
        }
    }
}

/// Observes the first generated output in complete downstream SSE events.
fn observe_first_output_events(observation: &RequestObservation, events: Vec<SseEvent>) {
    // Stop after the first token-bearing event and retain neither the event nor generated output.
    for event in events {
        if event.data() == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(event.data())
            && observation.needs_first_output()
            && is_generation_output(&value)
        {
            observation.record_first_output();
            break;
        }
    }
}

/// Returns whether an event carries a non-empty text, reasoning, or function increment.
pub(super) fn is_generation_output(value: &Value) -> bool {
    // Recognize only token-bearing Responses deltas and exclude lifecycle metadata or empty deltas.
    if value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            matches!(
                kind,
                "response.output_text.delta"
                    | "response.reasoning_text.delta"
                    | "response.function_call_arguments.delta"
            )
        })
    {
        return value
            .get("delta")
            .and_then(Value::as_str)
            .is_some_and(|delta| !delta.is_empty());
    }

    // Recognize non-empty Chat content, reasoning, or tool-call deltas.
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
                            .get("reasoning_content")
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
    let reasoning_output_tokens = usage
        .get("reasoning_output_tokens")
        .or_else(|| usage.get("reasoning_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| nested_usage_token(usage, "output_tokens_details", "reasoning_tokens"))
        .or_else(|| nested_usage_token(usage, "completion_tokens_details", "reasoning_tokens"));
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
        && reasoning_output_tokens.is_none()
        && total_tokens.is_none()
        && cached_input_tokens.is_none()
        && cache_write_input_tokens.is_none()
    {
        None
    } else {
        Some(TokenUsage {
            input_tokens,
            output_tokens,
            reasoning_output_tokens,
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
