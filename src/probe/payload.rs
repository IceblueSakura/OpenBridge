//! Fixed JSON requests and minimum response-shape checks for basic upstream probes.
//!
//! This module generates only built-in text and Embeddings inputs. It accepts no external URL,
//! model selection, tool definition, arbitrary request body, or executable action.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const EMBEDDING_PROBE_INPUT: &str = "OpenBridge probe";

/// Wire mode required by one minimum generation probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GenerationProbeMode {
    /// Expects one bounded JSON response.
    Json,
    /// Expects a bounded SSE stream with a Provider-recognized terminal event.
    Sse,
}

/// Builds the minimum text request for one registered generation API.
pub(super) fn probe_text_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
    mode: GenerationProbeMode,
) -> Value {
    match (protocol, mode) {
        (ApiProtocol::ChatCompletions, GenerationProbeMode::Json) => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        (ApiProtocol::Responses, GenerationProbeMode::Json) => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
        (ApiProtocol::ChatCompletions, GenerationProbeMode::Sse) => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "stream": true,
        }),
        (ApiProtocol::Responses, GenerationProbeMode::Sse) => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "store": false,
            "stream": true,
        }),
    }
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
