//! Fixed JSON requests and minimum response-shape checks for bounded upstream probes.
//!
//! This module generates only built-in text, structured-output, inline-image, first-turn
//! function-tool, and Embeddings inputs. The parent validates an optional model ID and bounded
//! admin-authored prompt/schema overrides; no external URL, path, tool definition, arbitrary
//! body, tool result, continuation state, or action is accepted.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

use super::{ProbeGenerationCapability, ProbeGenerationMode};

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const STRUCTURED_PROBE_PROMPT: &str =
    "Reply with exactly the plain text OK. Do not return a JSON object.";
const IMAGE_PROBE_PROMPT: &str =
    "Read the exact uppercase letters and digit shown in the image. Reply with only that text.";
const IMAGE_PROBE_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAVkAAABKAQAAAAA5vBr3AAABrklEQVRIx+3Vu3HDMAwGYPBUsOQG5ibmWq5C8VJkLeYyQFagN2DJQmcEfECiTCdOkypyo/PpO5kC8MOAv//McOADH/gvcJQ4T+hBeusimAT6BmBBI4IJMJFIAIyDoKtlrBLIgidcQA3YA11NxobwlEAUDGRGTI+dQTMWdKdiGx9gyFiNmA48YpGR8nQe7aKBpOkACjPWKthWiYZvUzBuWfGctLdJ4xS0N5+MHWMZCcuMFWHPONIVH2C9x6HglH+MsWh4kekbPG94GrEk7Bgvyj3AasVvhAWfucc3ucNUZ+VfSwe1xw2DHHEs+IOwLHW+w0uPSwcr1jQbjKNkrHZYZ/xenkwdfoJNxtc6G2rA9L3HNlfjWtpN9zdcq7HHAjMOpSnPML13xrHg03pmZBxN38GGRTThohnPP+JUcS1d6nAw/dTtsEOeuhXbfp53eEae5xHnpFS8EPaXNhs5KYw99hnccI1Vy+CIKcoZU7oJAwU2vHC6ZU13yeu6Nxq+NXzmvcHYYbeROkzDH8+8kUZMu65hFGWe4+lu1+HUb9EO66TT6W6Ldvj4Azrwgf89/gKfaJFN9aplBwAAAABJRU5ErkJggg==";
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

/// Fixed default name of the built-in response-format JSON schema.
pub(super) const DEFAULT_SCHEMA_NAME: &str = "openbridge_probe";

/// Validated admin-authored replacements for one closed Generation case.
///
/// The parent validates these overrides against the selected case before credential access; the
/// payload module trusts them to be bounded literal values.
#[derive(Clone, Copy, Default)]
pub(super) struct ProbeGenerationOverrides<'a> {
    /// Replaces the case's fixed user prompt text.
    pub(super) prompt: Option<&'a str>,
    /// Replaces a JSON Schema case's response-format schema object (parsed JSON text).
    pub(super) schema: Option<&'a str>,
    /// Replaces the fixed response-format schema name.
    pub(super) schema_name: Option<&'a str>,
}

/// Builds one fixed Generation request for a registered or explicitly selected upstream model.
///
/// The selection carries the closed case, delivery mode, and any validated admin-authored
/// overrides; wire policy stays fixed for every axis the admin cannot override.
pub(super) fn probe_generation_request(
    model: &str,
    max_output_tokens: u32,
    allow_unbounded_streaming_output: bool,
    selection: &crate::probe::GenerationCaseSelection,
) -> Value {
    let protocol = selection.protocol;
    let mode = selection.mode;
    let capability = selection.capability();
    let reasoning_effort = selection.reasoning_effort();
    let overrides = ProbeGenerationOverrides {
        prompt: selection.custom_prompt.as_deref(),
        schema: selection.custom_schema.as_deref(),
        schema_name: selection.custom_schema_name.as_deref(),
    };
    let default_prompt = match capability {
        ProbeGenerationCapability::Text => PROBE_PROMPT,
        ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => STRUCTURED_PROBE_PROMPT,
        ProbeGenerationCapability::ImageInputInlinePng => IMAGE_PROBE_PROMPT,
        ProbeGenerationCapability::ToolAuto => TOOL_PROBE_PROMPT,
        ProbeGenerationCapability::ToolNone => TOOL_NONE_PROMPT,
        ProbeGenerationCapability::ToolRequired => TOOL_PROBE_PROMPT,
        ProbeGenerationCapability::ToolNamed => TOOL_FORCED_PROMPT,
        ProbeGenerationCapability::ToolStrict => TOOL_STRICT_PROMPT,
        ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => TOOL_PARALLEL_PROMPT,
    };
    // Apply only the validated prompt override; tool cases bind their oracle to the fixed prompt.
    let prompt = overrides.prompt.unwrap_or(default_prompt);
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
    add_generation_capability(protocol, capability, overrides, &mut request);
    // Add only the Responses-only wire differential selected by the closed case; selection
    // validation guarantees these cases never reach the Chat wire shape.
    if protocol == ApiProtocol::Responses {
        let object = request
            .as_object_mut()
            .expect("built-in probe request is an object");
        match selection.case {
            crate::probe::ProbeGenerationCase::ReasoningSummary => {
                if let Some(reasoning) = object.get_mut("reasoning") {
                    reasoning
                        .as_object_mut()
                        .expect("Responses reasoning is an object")
                        .insert("summary".to_owned(), json!("auto"));
                }
            }
            crate::probe::ProbeGenerationCase::IncludeEncryptedContent => {
                object.insert("include".to_owned(), json!(["reasoning.encrypted_content"]));
            }
            crate::probe::ProbeGenerationCase::PromptCacheKey => {
                object.insert(
                    "prompt_cache_key".to_owned(),
                    Value::String("openbridge-probe-cache-key".to_owned()),
                );
            }
            _ => {}
        }
    }
    request
}

/// Adds one closed response-format differential; the validated schema override replaces only the
/// fixed JSON Schema case's response-format object, never the fixed tool or image payloads.
fn add_generation_capability(
    protocol: ApiProtocol,
    capability: ProbeGenerationCapability,
    overrides: ProbeGenerationOverrides<'_>,
    request: &mut Value,
) {
    if capability == ProbeGenerationCapability::ImageInputInlinePng {
        add_inline_png(protocol, request);
        return;
    }
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
        ProbeGenerationCapability::Text | ProbeGenerationCapability::ImageInputInlinePng => None,
        ProbeGenerationCapability::JsonObject => Some(json!({"type": "json_object"})),
        ProbeGenerationCapability::JsonSchema | ProbeGenerationCapability::JsonSchemaStrict => {
            let strict = capability == ProbeGenerationCapability::JsonSchemaStrict;
            // Apply only the validated schema override; keep the fixed conflict schema otherwise.
            let schema = overrides
                .schema
                .and_then(|custom| serde_json::from_str::<Value>(custom).ok())
                .unwrap_or_else(|| {
                    json!({
                        "type": "object",
                        "properties": {"probe": {"type": "string", "const": "ok"}},
                        "required": ["probe"],
                        "additionalProperties": false
                    })
                });
            let name = overrides.schema_name.unwrap_or(DEFAULT_SCHEMA_NAME);
            Some(match protocol {
                ApiProtocol::ChatCompletions => json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": name,
                        "strict": strict,
                        "schema": schema
                    }
                }),
                ApiProtocol::Responses => json!({
                    "type": "json_schema",
                    "name": name,
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

/// Replaces the text-only user input with one fixed text-and-inline-PNG message.
fn add_inline_png(protocol: ApiProtocol, request: &mut Value) {
    match protocol {
        ApiProtocol::ChatCompletions => {
            request["messages"][0]["content"] = json!([
                {"type": "text", "text": IMAGE_PROBE_PROMPT},
                {"type": "image_url", "image_url": {"url": IMAGE_PROBE_DATA_URL}}
            ]);
        }
        ApiProtocol::Responses => {
            request["input"] = json!([{
                "role": "user",
                "content": [
                    {"type": "input_text", "text": IMAGE_PROBE_PROMPT},
                    {"type": "input_image", "image_url": IMAGE_PROBE_DATA_URL}
                ]
            }]);
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
        | ProbeGenerationCapability::JsonSchemaStrict
        | ProbeGenerationCapability::ImageInputInlinePng => unreachable!(),
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

    fn selection(
        protocol: ApiProtocol,
        mode: ProbeGenerationMode,
        reasoning_effort: ProbeReasoningEffort,
        capability: ProbeGenerationCapability,
    ) -> crate::probe::GenerationCaseSelection {
        let case = match (capability, reasoning_effort) {
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::Omitted) => {
                ProbeGenerationCase::Text
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::None) => {
                ProbeGenerationCase::ReasoningNone
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::Minimal) => {
                ProbeGenerationCase::ReasoningMinimal
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::Low) => {
                ProbeGenerationCase::ReasoningLow
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::Medium) => {
                ProbeGenerationCase::ReasoningMedium
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::High) => {
                ProbeGenerationCase::ReasoningHigh
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::XHigh) => {
                ProbeGenerationCase::ReasoningXHigh
            }
            (ProbeGenerationCapability::Text, ProbeReasoningEffort::Max) => {
                ProbeGenerationCase::ReasoningMax
            }
            (ProbeGenerationCapability::JsonObject, _) => ProbeGenerationCase::JsonObject,
            (ProbeGenerationCapability::JsonSchema, _) => ProbeGenerationCase::JsonSchema,
            (ProbeGenerationCapability::JsonSchemaStrict, _) => {
                ProbeGenerationCase::JsonSchemaStrict
            }
            (ProbeGenerationCapability::ImageInputInlinePng, _) => {
                ProbeGenerationCase::ImageInputInlinePng
            }
            (ProbeGenerationCapability::ToolAuto, _) => ProbeGenerationCase::ToolAuto,
            (ProbeGenerationCapability::ToolNone, _) => ProbeGenerationCase::ToolNone,
            (ProbeGenerationCapability::ToolRequired, _) => ProbeGenerationCase::ToolRequired,
            (ProbeGenerationCapability::ToolNamed, _) => ProbeGenerationCase::ToolNamed,
            (ProbeGenerationCapability::ToolStrict, _) => ProbeGenerationCase::ToolStrict,
            (ProbeGenerationCapability::ToolParallelDisabled, _) => {
                ProbeGenerationCase::ToolParallelDisabled
            }
            (ProbeGenerationCapability::ToolParallelEnabled, _) => {
                ProbeGenerationCase::ToolParallelEnabled
            }
        };
        crate::probe::GenerationCaseSelection {
            protocol,
            mode,
            case,
            custom_prompt: None,
            custom_schema: None,
            custom_schema_name: None,
        }
    }
    use super::*;
    use crate::probe::{ProbeGenerationCapability, ProbeGenerationCase, ProbeReasoningEffort};

    #[test]
    fn structured_cases_use_fixed_protocol_specific_formats() {
        let chat = probe_generation_request(
            "candidate",
            64,
            false,
            &selection(
                ApiProtocol::ChatCompletions,
                ProbeGenerationMode::NonStreaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::JsonSchemaStrict,
            ),
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
            "candidate",
            64,
            false,
            &selection(
                ApiProtocol::Responses,
                ProbeGenerationMode::Streaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::JsonObject,
            ),
        );
        assert_eq!(
            responses.pointer("/text/format/type"),
            Some(&json!("json_object"))
        );
        assert_eq!(responses["stream"], true);
        assert!(responses.get("response_format").is_none());
    }

    #[test]
    fn inline_png_image_case_uses_fixed_protocol_specific_content_parts() {
        let chat = probe_generation_request(
            "candidate",
            4096,
            false,
            &selection(
                ApiProtocol::ChatCompletions,
                ProbeGenerationMode::NonStreaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ImageInputInlinePng,
            ),
        );
        assert_eq!(
            chat.pointer("/messages/0/content/0/type"),
            Some(&json!("text"))
        );
        assert_eq!(
            chat.pointer("/messages/0/content/1/type"),
            Some(&json!("image_url"))
        );
        assert!(
            chat.pointer("/messages/0/content/1/image_url/url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.starts_with("data:image/png;base64,iVBORw0KGgo"))
        );

        let responses = probe_generation_request(
            "candidate",
            4096,
            false,
            &selection(
                ApiProtocol::Responses,
                ProbeGenerationMode::Streaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ImageInputInlinePng,
            ),
        );
        assert_eq!(
            responses.pointer("/input/0/content/0/type"),
            Some(&json!("input_text"))
        );
        assert_eq!(
            responses.pointer("/input/0/content/1/type"),
            Some(&json!("input_image"))
        );
        assert!(
            responses
                .pointer("/input/0/content/1/image_url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.starts_with("data:image/png;base64,iVBORw0KGgo"))
        );
        assert_eq!(responses["stream"], true);
    }

    #[test]
    fn tool_auto_case_uses_fixed_protocol_specific_function_wire() {
        let chat = probe_generation_request(
            "candidate",
            4096,
            false,
            &selection(
                ApiProtocol::ChatCompletions,
                ProbeGenerationMode::NonStreaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ToolAuto,
            ),
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
            "candidate",
            4096,
            false,
            &selection(
                ApiProtocol::Responses,
                ProbeGenerationMode::Streaming,
                ProbeReasoningEffort::Omitted,
                ProbeGenerationCapability::ToolAuto,
            ),
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
                    "candidate",
                    4096,
                    false,
                    &selection(
                        protocol,
                        ProbeGenerationMode::NonStreaming,
                        ProbeReasoningEffort::Omitted,
                        capability,
                    ),
                );
                assert_eq!(request["tool_choice"], choice);
                assert_eq!(request["tools"].as_array().unwrap().len(), 2);
            }
        }

        for protocol in [ApiProtocol::ChatCompletions, ApiProtocol::Responses] {
            let named = probe_generation_request(
                "candidate",
                4096,
                false,
                &selection(
                    protocol,
                    ProbeGenerationMode::NonStreaming,
                    ProbeReasoningEffort::Omitted,
                    ProbeGenerationCapability::ToolNamed,
                ),
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
                "candidate",
                4096,
                false,
                &selection(
                    protocol,
                    ProbeGenerationMode::NonStreaming,
                    ProbeReasoningEffort::Omitted,
                    ProbeGenerationCapability::ToolStrict,
                ),
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
                    "candidate",
                    4096,
                    false,
                    &selection(
                        protocol,
                        ProbeGenerationMode::NonStreaming,
                        ProbeReasoningEffort::Omitted,
                        capability,
                    ),
                );
                assert_eq!(parallel["tool_choice"], "required");
                assert_eq!(parallel["parallel_tool_calls"], enabled);
                assert_eq!(parallel["tools"].as_array().unwrap().len(), 2);
            }
        }
    }
}
