//! Fixed JSON requests and minimum response-shape checks for bounded upstream probes.
//!
//! This module generates only built-in text and Embeddings inputs. The parent validates an optional
//! model ID; no external URL, path, prompt, tool definition, arbitrary body, or action is accepted.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

use super::{ProbeGenerationCapability, ProbeGenerationMode, ProbeReasoningEffort};

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const STRUCTURED_PROBE_PROMPT: &str =
    "Reply with exactly the plain text OK. Do not return a JSON object.";
const EMBEDDING_PROBE_INPUT: &str = "OpenBridge probe";

/// Builds one fixed Generation request for a registered or explicitly selected upstream model.
pub(super) fn probe_generation_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
    mode: ProbeGenerationMode,
    reasoning_effort: ProbeReasoningEffort,
    capability: ProbeGenerationCapability,
    allow_unbounded_streaming_output: bool,
) -> Value {
    let prompt = match capability {
        ProbeGenerationCapability::Text => PROBE_PROMPT,
        ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => STRUCTURED_PROBE_PROMPT,
    };
    let mut request = match (protocol, mode) {
        (ApiProtocol::ChatCompletions, ProbeGenerationMode::NonStreaming) => json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        (ApiProtocol::Responses, ProbeGenerationMode::NonStreaming) => json!({
            "model": model,
            "input": prompt,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
        (ApiProtocol::ChatCompletions, ProbeGenerationMode::Streaming) => json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_completion_tokens": max_output_tokens,
            "stream": true,
        }),
        (ApiProtocol::Responses, ProbeGenerationMode::Streaming) => json!({
            "model": model,
            "input": prompt,
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

    // Add only the standard reasoning field selected by this differential case.
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
    add_generation_capability(protocol, capability, &mut request);
    request
}

/// Adds one closed response-format differential without caller-provided schema or prompt.
fn add_generation_capability(
    protocol: ApiProtocol,
    capability: ProbeGenerationCapability,
    request: &mut Value,
) {
    let Some(format) = (match capability {
        ProbeGenerationCapability::Text => None,
        ProbeGenerationCapability::JsonObject => Some(json!({"type": "json_object"})),
        ProbeGenerationCapability::JsonSchema | ProbeGenerationCapability::JsonSchemaStrict => {
            let strict = capability == ProbeGenerationCapability::JsonSchemaStrict;
            let schema = json!({
                "type": "object",
                "properties": {"probe": {"type": "string", "const": "ok"}},
                "required": ["probe"],
                "additionalProperties": false
            });
            Some(match protocol {
                ApiProtocol::ChatCompletions => json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "openbridge_probe",
                        "strict": strict,
                        "schema": schema
                    }
                }),
                ApiProtocol::Responses => json!({
                    "type": "json_schema",
                    "name": "openbridge_probe",
                    "strict": strict,
                    "schema": schema
                }),
            })
        }
    }) else {
        return;
    };
    let object = request
        .as_object_mut()
        .expect("built-in probe request is an object");
    match protocol {
        ApiProtocol::ChatCompletions => {
            object.insert("response_format".to_owned(), format);
        }
        ApiProtocol::Responses => {
            object.insert("text".to_owned(), json!({"format": format}));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::ProbeGenerationCapability;

    #[test]
    fn structured_cases_use_fixed_protocol_specific_formats() {
        let chat = probe_generation_request(
            ApiProtocol::ChatCompletions,
            "candidate",
            64,
            ProbeGenerationMode::NonStreaming,
            ProbeReasoningEffort::Omitted,
            ProbeGenerationCapability::JsonSchemaStrict,
            false,
        );
        assert_eq!(
            chat.pointer("/response_format/type"),
            Some(&json!("json_schema"))
        );
        assert_eq!(
            chat.pointer("/response_format/json_schema/strict"),
            Some(&json!(true))
        );
        assert_eq!(
            chat.pointer("/response_format/json_schema/schema/properties/probe/const"),
            Some(&json!("ok"))
        );

        let responses = probe_generation_request(
            ApiProtocol::Responses,
            "candidate",
            64,
            ProbeGenerationMode::Streaming,
            ProbeReasoningEffort::Omitted,
            ProbeGenerationCapability::JsonObject,
            false,
        );
        assert_eq!(
            responses.pointer("/text/format/type"),
            Some(&json!("json_object"))
        );
        assert_eq!(responses["stream"], true);
        assert!(responses.get("response_format").is_none());
    }
}
