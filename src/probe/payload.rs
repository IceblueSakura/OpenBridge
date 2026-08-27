//! Fixed JSON requests and minimum response-shape checks for basic upstream probes.
//!
//! This module generates only built-in text and Embeddings inputs. The parent validates an optional
//! model ID; no external URL, path, prompt, tool definition, arbitrary body, or action is accepted.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

use super::{ProbeGenerationMode, ProbeReasoningEffort};

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const EMBEDDING_PROBE_INPUT: &str = "OpenBridge probe";

/// Builds one fixed text request for a registered or explicitly selected upstream model.
pub(super) fn probe_text_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
    mode: ProbeGenerationMode,
    reasoning_effort: ProbeReasoningEffort,
    allow_unbounded_streaming_output: bool,
) -> Value {
    let mut request = match (protocol, mode) {
        (ApiProtocol::ChatCompletions, ProbeGenerationMode::NonStreaming) => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        (ApiProtocol::Responses, ProbeGenerationMode::NonStreaming) => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
        (ApiProtocol::ChatCompletions, ProbeGenerationMode::Streaming) => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": true,
        }),
        (ApiProtocol::Responses, ProbeGenerationMode::Streaming) => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": true,
        }),
    };

    // Require an explicit opt-in before omitting the only upstream generation-token budget.
    if mode == ProbeGenerationMode::Streaming && allow_unbounded_streaming_output {
        let object = request
            .as_object_mut()
            .expect("built-in probe request is an object");
        match protocol {
            ApiProtocol::ChatCompletions => {
                object.remove("max_completion_tokens");
            }
            ApiProtocol::Responses => {
                object.remove("max_output_tokens");
            }
        }
    }

    // Add only the standard protocol field selected by this differential case.
    if let Some(effort) = reasoning_effort.as_wire() {
        let object = request
            .as_object_mut()
            .expect("built-in probe request is an object");
        match protocol {
            ApiProtocol::ChatCompletions => {
                object.insert(
                    "reasoning_effort".to_owned(),
                    Value::String(effort.to_owned()),
                );
            }
            ApiProtocol::Responses => {
                object.insert("reasoning".to_owned(), json!({"effort": effort}));
            }
        }
    }
    request
}

/// Builds one fixed single-text Embeddings Create request.
pub(super) fn probe_embedding_request(model: &str) -> Value {
    json!({
        "model": model,
        "input": EMBEDDING_PROBE_INPUT,
    })
}

/// Returns whether successful JSON has the minimum response shape for the target protocol.
pub(super) fn is_protocol_response(protocol: ApiProtocol, response: &Value) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => response
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|choices| !choices.is_empty()),
        ApiProtocol::Responses => {
            response.get("object").and_then(Value::as_str) == Some("response")
        }
    }
}

/// Returns whether successful JSON has one recognizable Embeddings response item.
pub(super) fn is_embedding_response(response: &Value, upstream_model: &str) -> bool {
    let data = response.get("data").and_then(Value::as_array);
    let usage = response.get("usage").and_then(Value::as_object);
    response.get("object").and_then(Value::as_str) == Some("list")
        && response.get("model").and_then(Value::as_str) == Some(upstream_model)
        && data.is_some_and(|items| {
            items.len() == 1
                && items[0].get("object").and_then(Value::as_str) == Some("embedding")
                && items[0].get("index").and_then(Value::as_u64) == Some(0)
                && items[0]
                    .get("embedding")
                    .is_some_and(|value| value.is_array() || value.is_string())
        })
        && usage.is_some_and(|usage| {
            usage.get("prompt_tokens").and_then(Value::as_u64).is_some()
                && usage.get("total_tokens").and_then(Value::as_u64).is_some()
        })
}
