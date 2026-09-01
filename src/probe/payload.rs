//! Fixed JSON requests and minimum response-shape checks for bounded upstream probes.
//!
//! This module generates only built-in text, structured-output, first-turn function-tool, and
//! Embeddings inputs. The parent validates an optional model ID; no external URL, path, prompt,
//! tool definition, arbitrary body, tool result, continuation state, or action is accepted.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

use super::{ProbeGenerationCapability, ProbeGenerationMode, ProbeReasoningEffort};

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const STRUCTURED_PROBE_PROMPT: &str =
    "Reply with exactly the plain text OK. Do not return a JSON object.";
const TOOL_PROBE_PROMPT: &str =
    "Call openbridge_probe_primary exactly once with value primary. Do not answer with text.";
const TOOL_NONE_PROMPT: &str =
    "Call openbridge_probe_primary exactly once with value primary. Do not answer with text.";
const TOOL_FORCED_PROMPT: &str = "Reply with exactly OK without calling any tool.";
const TOOL_STRICT_PROMPT: &str =
    "Call openbridge_probe_primary exactly once with value wrong. Do not answer with text.";
const TOOL_PARALLEL_PROMPT: &str = "Call openbridge_probe_primary with value primary and openbridge_probe_secondary with value secondary in the same response. Do not answer with text.";
const PRIMARY_TOOL_NAME: &str = "openbridge_probe_primary";
const SECONDARY_TOOL_NAME: &str = "openbridge_probe_secondary";
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
        ProbeGenerationCapability::ToolAuto => TOOL_PROBE_PROMPT,
        ProbeGenerationCapability::ToolNone => TOOL_NONE_PROMPT,
        ProbeGenerationCapability::ToolRequired => TOOL_PROBE_PROMPT,
        ProbeGenerationCapability::ToolNamed => TOOL_FORCED_PROMPT,
        ProbeGenerationCapability::ToolStrict => TOOL_STRICT_PROMPT,
        ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => TOOL_PARALLEL_PROMPT,
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
    if matches!(
        capability,
        ProbeGenerationCapability::ToolAuto
            | ProbeGenerationCapability::ToolNone
            | ProbeGenerationCapability::ToolRequired
            | ProbeGenerationCapability::ToolNamed
            | ProbeGenerationCapability::ToolStrict
            | ProbeGenerationCapability::ToolParallelDisabled
            | ProbeGenerationCapability::ToolParallelEnabled
    ) {
        add_function_tools(protocol, capability, request);
        return;
    }
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
        ProbeGenerationCapability::ToolAuto
        | ProbeGenerationCapability::ToolNone
        | ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolNamed
        | ProbeGenerationCapability::ToolStrict
        | ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => unreachable!(),
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

/// Adds closed fixed function tools without caller-provided prompt, name, or schema.
fn add_function_tools(
    protocol: ApiProtocol,
    capability: ProbeGenerationCapability,
    request: &mut Value,
) {
    let strict = capability == ProbeGenerationCapability::ToolStrict;
    let mut tools = vec![fixed_function_tool(
        protocol,
        PRIMARY_TOOL_NAME,
        "primary",
        strict,
    )];
    if capability != ProbeGenerationCapability::ToolStrict {
        tools.push(fixed_function_tool(
            protocol,
            SECONDARY_TOOL_NAME,
            "secondary",
            false,
        ));
    }
    let tool_choice = match capability {
        ProbeGenerationCapability::ToolAuto => json!("auto"),
        ProbeGenerationCapability::ToolNone => json!("none"),
        ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolStrict
        | ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => json!("required"),
        ProbeGenerationCapability::ToolNamed => match protocol {
            ApiProtocol::ChatCompletions => {
                json!({"type": "function", "function": {"name": PRIMARY_TOOL_NAME}})
            }
            ApiProtocol::Responses => {
                json!({"type": "function", "name": PRIMARY_TOOL_NAME})
            }
        },
        ProbeGenerationCapability::Text
        | ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => unreachable!(),
    };
    let object = request
        .as_object_mut()
        .expect("built-in probe request is an object");
    object.insert("tools".to_owned(), Value::Array(tools));
    object.insert("tool_choice".to_owned(), tool_choice);
    match capability {
        ProbeGenerationCapability::ToolParallelDisabled => {
            object.insert("parallel_tool_calls".to_owned(), Value::Bool(false));
        }
        ProbeGenerationCapability::ToolParallelEnabled => {
            object.insert("parallel_tool_calls".to_owned(), Value::Bool(true));
        }
        _ => {}
    }
}

fn fixed_function_tool(protocol: ApiProtocol, name: &str, value: &str, strict: bool) -> Value {
    let schema = json!({
        "type": "object",
        "properties": {"value": {"type": "string", "const": value}},
        "required": ["value"],
        "additionalProperties": false
    });
    let mut function = json!({
        "name": name,
        "description": "Returns one fixed probe value.",
        "parameters": schema
    });
    if strict {
        function["strict"] = Value::Bool(true);
    }
    match protocol {
        ApiProtocol::ChatCompletions => json!({"type": "function", "function": function}),
        ApiProtocol::Responses => {
            let mut tool = function;
            tool["type"] = json!("function");
            tool
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

    #[test]
    fn tool_auto_case_uses_fixed_protocol_specific_function_wire() {
        let chat = probe_generation_request(
            ApiProtocol::ChatCompletions,
            "candidate",
            4096,
            ProbeGenerationMode::NonStreaming,
            ProbeReasoningEffort::Omitted,
            ProbeGenerationCapability::ToolAuto,
            false,
        );
        assert_eq!(chat["tool_choice"], "auto");
        assert_eq!(chat["tools"][0]["type"], "function");
        assert_eq!(
            chat["tools"][0]["function"]["name"],
            "openbridge_probe_primary"
        );
        assert_eq!(
            chat["tools"][0]["function"]["parameters"]["properties"]["value"]["const"],
            "primary"
        );
        assert!(chat["tools"][0]["function"].get("strict").is_none());

        let responses = probe_generation_request(
            ApiProtocol::Responses,
            "candidate",
            4096,
            ProbeGenerationMode::Streaming,
            ProbeReasoningEffort::Omitted,
            ProbeGenerationCapability::ToolAuto,
            false,
        );
        assert_eq!(responses["tool_choice"], "auto");
        assert_eq!(responses["tools"][0]["type"], "function");
        assert_eq!(responses["tools"][0]["name"], "openbridge_probe_primary");
        assert_eq!(
            responses["tools"][0]["parameters"]["properties"]["value"]["const"],
            "primary"
        );
        assert!(responses["tools"][0].get("strict").is_none());
    }

    #[test]
    fn tool_choice_strict_and_parallel_cases_use_closed_fixed_wire() {
        let cases = [
            (ProbeGenerationCapability::ToolNone, "none"),
            (ProbeGenerationCapability::ToolRequired, "required"),
        ];
        for (capability, choice) in cases {
            for protocol in [ApiProtocol::ChatCompletions, ApiProtocol::Responses] {
                let request = probe_generation_request(
                    protocol,
                    "candidate",
                    4096,
                    ProbeGenerationMode::NonStreaming,
                    ProbeReasoningEffort::Omitted,
                    capability,
                    false,
                );
                assert_eq!(request["tool_choice"], choice);
                assert_eq!(request["tools"].as_array().unwrap().len(), 2);
            }
        }

        for protocol in [ApiProtocol::ChatCompletions, ApiProtocol::Responses] {
            let named = probe_generation_request(
                protocol,
                "candidate",
                4096,
                ProbeGenerationMode::NonStreaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ToolNamed,
                false,
            );
            match protocol {
                ApiProtocol::ChatCompletions => assert_eq!(
                    named["tool_choice"],
                    json!({"type": "function", "function": {"name": PRIMARY_TOOL_NAME}})
                ),
                ApiProtocol::Responses => assert_eq!(
                    named["tool_choice"],
                    json!({"type": "function", "name": PRIMARY_TOOL_NAME})
                ),
            }

            let strict = probe_generation_request(
                protocol,
                "candidate",
                4096,
                ProbeGenerationMode::NonStreaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ToolStrict,
                false,
            );
            let tool = match protocol {
                ApiProtocol::ChatCompletions => &strict["tools"][0]["function"],
                ApiProtocol::Responses => &strict["tools"][0],
            };
            assert_eq!(tool["strict"], true);
            assert_eq!(strict["tool_choice"], "required");

            for (capability, enabled) in [
                (ProbeGenerationCapability::ToolParallelDisabled, false),
                (ProbeGenerationCapability::ToolParallelEnabled, true),
            ] {
                let parallel = probe_generation_request(
                    protocol,
                    "candidate",
                    4096,
                    ProbeGenerationMode::NonStreaming,
                    ProbeReasoningEffort::Omitted,
                    capability,
                    false,
                );
                assert_eq!(parallel["tool_choice"], "required");
                assert_eq!(parallel["parallel_tool_calls"], enabled);
                assert_eq!(parallel["tools"].as_array().unwrap().len(), 2);
            }
        }
    }
}
