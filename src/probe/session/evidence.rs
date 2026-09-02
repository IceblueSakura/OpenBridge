//! Bounded protocol evidence extraction for administrative Generation probes.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use serde_json::Value;

use crate::{core::ApiProtocol, registry::UpstreamApi, transport::sse::SseEvent};

use super::super::{
    GenerationCaseSelection, GenerationProbeEvidence, GenerationProbeResult,
    ProbeCapabilityEvidence, ProbeCapabilityVerdict, ProbeGenerationCapability,
    ProbeGenerationMode, ProbeProtocol, ProbeResult, ProbeTerminal, ProbeTokenUsage,
};

pub(super) fn generation_result(
    selection: &GenerationCaseSelection,
    upstream_model: Option<&str>,
    elapsed_ms: u64,
    outcome: ProbeResult,
    evidence: Option<GenerationProbeEvidence>,
    capability_evidence: Option<ProbeCapabilityEvidence>,
) -> GenerationProbeResult {
    GenerationProbeResult {
        protocol: ProbeProtocol::from_api(selection.protocol),
        mode: selection.mode,
        case: selection.case,
        upstream_model: upstream_model.map(str::to_owned),
        elapsed_ms,
        outcome,
        evidence,
        capability_evidence,
        custom_prompt_fingerprint: selection
            .custom_prompt
            .as_deref()
            .map(crate::probe::override_fingerprint),
        custom_schema_fingerprint: selection
            .custom_schema
            .as_deref()
            .map(crate::probe::override_fingerprint),
        custom_schema_name: selection.custom_schema_name.clone(),
    }
}

const PRIMARY_TOOL_NAME: &str = "openbridge_probe_primary";
const SECONDARY_TOOL_NAME: &str = "openbridge_probe_secondary";
const MAX_OBSERVED_TOOL_CALLS: usize = 16;

#[derive(Default)]
pub(super) struct GenerationOutputObservation {
    pub(super) text: String,
    tool_calls: Vec<ToolCallObservation>,
    stream_tool_calls: BTreeMap<String, StreamToolCallObservation>,
    tool_shape_valid: bool,
}

struct ToolCallObservation {
    name: String,
    arguments: String,
}

#[derive(Default)]
struct StreamToolCallObservation {
    call_id: Option<String>,
    output_index: Option<u64>,
    name: Option<String>,
    arguments: String,
}

impl GenerationOutputObservation {
    pub(super) fn new() -> Self {
        Self {
            tool_shape_valid: true,
            ..Self::default()
        }
    }

    #[cfg(test)]
    fn text(value: &str) -> Self {
        Self {
            text: value.to_owned(),
            ..Self::new()
        }
    }

    pub(super) fn finish_stream_tool_calls(&mut self) {
        let mut call_ids = BTreeSet::new();
        for (_, call) in std::mem::take(&mut self.stream_tool_calls) {
            let Some(call_id) = call.call_id else {
                self.tool_shape_valid = false;
                continue;
            };
            if call_id.is_empty() || !call_ids.insert(call_id) {
                self.tool_shape_valid = false;
                continue;
            }
            let Some(name) = call.name else {
                self.tool_shape_valid = false;
                continue;
            };
            self.tool_calls.push(ToolCallObservation {
                name,
                arguments: call.arguments,
            });
        }
    }
}

/// Extracts transient output text and function calls from one non-streaming protocol envelope.
pub(super) fn json_generation_output(
    protocol: ApiProtocol,
    body: &Value,
) -> GenerationOutputObservation {
    let mut output = GenerationOutputObservation::new();
    match protocol {
        ApiProtocol::ChatCompletions => {
            let Some(choices) = body.get("choices").and_then(Value::as_array) else {
                return output;
            };
            if choices.len() != 1 {
                output.tool_shape_valid = false;
                return output;
            }
            let mut call_ids = BTreeSet::new();
            for choice in choices {
                if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
                    output.tool_shape_valid = false;
                }
                let Some(message) = choice.get("message") else {
                    continue;
                };
                if let Some(fragment) = message.get("content").and_then(Value::as_str) {
                    output.text.push_str(fragment);
                }
                let Some(calls) = message.get("tool_calls").filter(|value| !value.is_null()) else {
                    continue;
                };
                let Some(calls) = calls.as_array() else {
                    output.tool_shape_valid = false;
                    continue;
                };
                if calls.len() > MAX_OBSERVED_TOOL_CALLS {
                    output.tool_shape_valid = false;
                    continue;
                }
                for call in calls {
                    let parsed = call
                        .as_object()
                        .and_then(|call| {
                            if call.get("type").and_then(Value::as_str) != Some("function") {
                                return None;
                            }
                            let call_id = call.get("id")?.as_str()?.to_owned();
                            if call_id.is_empty() || !call_ids.insert(call_id) {
                                return None;
                            }
                            call.get("function")
                        })
                        .and_then(Value::as_object)
                        .and_then(|function| {
                            Some(ToolCallObservation {
                                name: function.get("name")?.as_str()?.to_owned(),
                                arguments: function.get("arguments")?.as_str()?.to_owned(),
                            })
                        });
                    if let Some(call) = parsed {
                        output.tool_calls.push(call);
                    } else {
                        output.tool_shape_valid = false;
                    }
                }
            }
        }
        ApiProtocol::Responses => {
            let Some(items) = body.get("output").and_then(Value::as_array) else {
                return output;
            };
            let mut item_ids = BTreeSet::new();
            let mut call_ids = BTreeSet::new();
            let mut observed_tool_calls = 0usize;
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    if observed_tool_calls >= MAX_OBSERVED_TOOL_CALLS {
                        output.tool_shape_valid = false;
                        break;
                    }
                    observed_tool_calls += 1;
                    let parsed = item.as_object().and_then(|item| {
                        let item_id = item.get("id")?.as_str()?.to_owned();
                        let call_id = item.get("call_id")?.as_str()?.to_owned();
                        if item_id.is_empty()
                            || call_id.is_empty()
                            || !item_ids.insert(item_id)
                            || !call_ids.insert(call_id)
                        {
                            return None;
                        }
                        Some(ToolCallObservation {
                            name: item.get("name")?.as_str()?.to_owned(),
                            arguments: item.get("arguments")?.as_str()?.to_owned(),
                        })
                    });
                    if let Some(call) = parsed {
                        output.tool_calls.push(call);
                    } else {
                        output.tool_shape_valid = false;
                    }
                }
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
                {
                    if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                        output.text.push_str(fragment);
                    }
                }
            }
        }
    }
    output
}

/// Derives one semantic conclusion without retaining the transient generated text.
pub(super) fn generation_capability_evidence(
    capability: ProbeGenerationCapability,
    output: &GenerationOutputObservation,
    terminal: Option<ProbeTerminal>,
) -> ProbeCapabilityEvidence {
    if terminal == Some(ProbeTerminal::ResponsesIncomplete)
        || terminal.is_none()
        || !output.tool_shape_valid
    {
        return ProbeCapabilityEvidence {
            verdict: ProbeCapabilityVerdict::Inconclusive,
            valid_json_object: None,
            fixed_schema_match: None,
            fixed_image_match: None,
            tool_call_count: None,
            fixed_tool_match: None,
            fixed_arguments_match: None,
        };
    }
    match capability {
        ProbeGenerationCapability::Text => ProbeCapabilityEvidence {
            verdict: if !output.text.trim().is_empty() {
                ProbeCapabilityVerdict::Supported
            } else {
                ProbeCapabilityVerdict::NotHonored
            },
            valid_json_object: None,
            fixed_schema_match: None,
            fixed_image_match: None,
            tool_call_count: None,
            fixed_tool_match: None,
            fixed_arguments_match: None,
        },
        ProbeGenerationCapability::ImageInputInlinePng => {
            let fixed_image_match = output.text == "OPENBRIDGE 7";
            ProbeCapabilityEvidence {
                verdict: if fixed_image_match {
                    ProbeCapabilityVerdict::Supported
                } else {
                    ProbeCapabilityVerdict::Inconclusive
                },
                valid_json_object: None,
                fixed_schema_match: None,
                fixed_image_match: Some(fixed_image_match),
                tool_call_count: None,
                fixed_tool_match: None,
                fixed_arguments_match: None,
            }
        }
        ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => {
            let document = crate::bridge::strict_json::from_str(&output.text).ok();
            let valid_json_object = document.as_ref().is_some_and(Value::is_object);
            let fixed_schema_match = document.as_ref().is_some_and(|document| {
                document.as_object().is_some_and(|object| {
                    object.len() == 1 && object.get("probe").and_then(Value::as_str) == Some("ok")
                })
            });
            let honored = match capability {
                ProbeGenerationCapability::JsonObject => valid_json_object,
                ProbeGenerationCapability::JsonSchema
                | ProbeGenerationCapability::JsonSchemaStrict => fixed_schema_match,
                ProbeGenerationCapability::Text
                | ProbeGenerationCapability::ImageInputInlinePng
                | ProbeGenerationCapability::ToolAuto
                | ProbeGenerationCapability::ToolNone
                | ProbeGenerationCapability::ToolRequired
                | ProbeGenerationCapability::ToolNamed
                | ProbeGenerationCapability::ToolStrict
                | ProbeGenerationCapability::ToolParallelDisabled
                | ProbeGenerationCapability::ToolParallelEnabled => {
                    unreachable!()
                }
            };
            ProbeCapabilityEvidence {
                verdict: if honored {
                    ProbeCapabilityVerdict::Supported
                } else {
                    ProbeCapabilityVerdict::NotHonored
                },
                valid_json_object: Some(valid_json_object),
                fixed_schema_match: matches!(
                    capability,
                    ProbeGenerationCapability::JsonSchema
                        | ProbeGenerationCapability::JsonSchemaStrict
                )
                .then_some(fixed_schema_match),
                fixed_image_match: None,
                tool_call_count: None,
                fixed_tool_match: None,
                fixed_arguments_match: None,
            }
        }
        ProbeGenerationCapability::ToolAuto
        | ProbeGenerationCapability::ToolNone
        | ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolNamed
        | ProbeGenerationCapability::ToolStrict
        | ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => {
            tool_capability_evidence(capability, output)
        }
    }
}

fn tool_capability_evidence(
    capability: ProbeGenerationCapability,
    output: &GenerationOutputObservation,
) -> ProbeCapabilityEvidence {
    let fixed_tool_match = match capability {
        ProbeGenerationCapability::ToolNone => output.tool_calls.is_empty(),
        ProbeGenerationCapability::ToolAuto
        | ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolNamed
        | ProbeGenerationCapability::ToolStrict => {
            output.tool_calls.len() == 1 && output.tool_calls[0].name == PRIMARY_TOOL_NAME
        }
        ProbeGenerationCapability::ToolParallelDisabled => {
            output.tool_calls.len() == 1
                && matches!(
                    output.tool_calls[0].name.as_str(),
                    PRIMARY_TOOL_NAME | SECONDARY_TOOL_NAME
                )
        }
        ProbeGenerationCapability::ToolParallelEnabled => {
            output.tool_calls.len() == 2
                && [PRIMARY_TOOL_NAME, SECONDARY_TOOL_NAME].iter().all(|name| {
                    output
                        .tool_calls
                        .iter()
                        .filter(|call| call.name == *name)
                        .count()
                        == 1
                })
        }
        ProbeGenerationCapability::Text
        | ProbeGenerationCapability::ImageInputInlinePng
        | ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => unreachable!(),
    };
    let fixed_arguments_match = match capability {
        ProbeGenerationCapability::ToolNone => None,
        ProbeGenerationCapability::ToolAuto
        | ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolNamed
        | ProbeGenerationCapability::ToolStrict
        | ProbeGenerationCapability::ToolParallelDisabled
        | ProbeGenerationCapability::ToolParallelEnabled => {
            (!output.tool_calls.is_empty()).then(|| {
                output.tool_calls.iter().all(|call| {
                    let expected = match call.name.as_str() {
                        PRIMARY_TOOL_NAME => "primary",
                        SECONDARY_TOOL_NAME => "secondary",
                        _ => return false,
                    };
                    tool_arguments_match(call, expected)
                })
            })
        }
        ProbeGenerationCapability::Text
        | ProbeGenerationCapability::ImageInputInlinePng
        | ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => unreachable!(),
    };
    let exact_match = fixed_tool_match && fixed_arguments_match.unwrap_or(true);
    let verdict = match capability {
        ProbeGenerationCapability::ToolNone => {
            if output.tool_calls.is_empty() {
                ProbeCapabilityVerdict::Supported
            } else {
                ProbeCapabilityVerdict::NotHonored
            }
        }
        ProbeGenerationCapability::ToolAuto => {
            if output.tool_calls.is_empty() {
                ProbeCapabilityVerdict::Inconclusive
            } else if exact_match {
                ProbeCapabilityVerdict::Supported
            } else {
                ProbeCapabilityVerdict::NotHonored
            }
        }
        ProbeGenerationCapability::ToolParallelDisabled => match output.tool_calls.len() {
            0 => ProbeCapabilityVerdict::Inconclusive,
            1 if exact_match => ProbeCapabilityVerdict::Supported,
            _ => ProbeCapabilityVerdict::NotHonored,
        },
        ProbeGenerationCapability::ToolParallelEnabled => match output.tool_calls.len() {
            0 | 1 => ProbeCapabilityVerdict::Inconclusive,
            2 if exact_match => ProbeCapabilityVerdict::Supported,
            _ => ProbeCapabilityVerdict::NotHonored,
        },
        ProbeGenerationCapability::ToolRequired
        | ProbeGenerationCapability::ToolNamed
        | ProbeGenerationCapability::ToolStrict => {
            if exact_match {
                ProbeCapabilityVerdict::Supported
            } else {
                ProbeCapabilityVerdict::NotHonored
            }
        }
        ProbeGenerationCapability::Text
        | ProbeGenerationCapability::ImageInputInlinePng
        | ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => unreachable!(),
    };
    ProbeCapabilityEvidence {
        verdict,
        valid_json_object: None,
        fixed_schema_match: None,
        fixed_image_match: None,
        tool_call_count: Some(output.tool_calls.len()),
        fixed_tool_match: Some(fixed_tool_match),
        fixed_arguments_match,
    }
}

fn tool_arguments_match(call: &ToolCallObservation, expected: &str) -> bool {
    crate::bridge::strict_json::from_str(&call.arguments)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .is_some_and(|object| {
            object.len() == 1 && object.get("value").and_then(Value::as_str) == Some(expected)
        })
}

pub(super) fn probe_mode_allowed(upstream_api: &UpstreamApi, mode: ProbeGenerationMode) -> bool {
    match mode {
        ProbeGenerationMode::Streaming => upstream_api
            .capabilities()
            .generation_capabilities()
            .is_some_and(|capabilities| capabilities.streaming),
        ProbeGenerationMode::NonStreaming => !upstream_api.streaming_policy().requires_streaming(),
    }
}

pub(super) fn elapsed_millis(started: &Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn json_generation_evidence(
    protocol: ApiProtocol,
    body: &Value,
    content_type: Option<String>,
) -> GenerationProbeEvidence {
    let terminal = match protocol {
        ApiProtocol::ChatCompletions => Some(ProbeTerminal::NonStreaming),
        ApiProtocol::Responses => match body.get("status").and_then(Value::as_str) {
            Some("completed") => Some(ProbeTerminal::ResponsesCompleted),
            Some("incomplete") => Some(ProbeTerminal::ResponsesIncomplete),
            Some("failed") => Some(ProbeTerminal::ResponsesFailed),
            _ => Some(ProbeTerminal::NonStreaming),
        },
    };
    GenerationProbeEvidence {
        content_type,
        terminal,
        usage_present: body.get("usage").is_some_and(Value::is_object),
        usage: body.get("usage").and_then(probe_token_usage),
        output_text_observed: match protocol {
            ApiProtocol::ChatCompletions => body
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice
                            .get("message")
                            .and_then(|message| message.get("content"))
                            .is_some_and(|content| !content.is_null())
                    })
                }),
            ApiProtocol::Responses => response_output_has_type(body, "output_text"),
        },
        reasoning_observed: match protocol {
            ApiProtocol::ChatCompletions => body
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice.get("message").is_some_and(|message| {
                            ["reasoning_content", "reasoning", "reasoning_details"]
                                .iter()
                                .any(|field| {
                                    message.get(*field).is_some_and(|value| !value.is_null())
                                })
                        })
                    })
                }),
            ApiProtocol::Responses => response_output_has_type(body, "reasoning"),
        },
        // A summary is observed when some reasoning output item carries a non-empty summary
        // array; summary text itself is never retained.
        reasoning_summary_observed: match protocol {
            ApiProtocol::ChatCompletions => false,
            ApiProtocol::Responses => {
                body.get("output")
                    .and_then(Value::as_array)
                    .is_some_and(|items| {
                        items.iter().any(|item| {
                            item.get("type").and_then(Value::as_str) == Some("reasoning")
                                && item
                                    .get("summary")
                                    .and_then(Value::as_array)
                                    .is_some_and(|summary| !summary.is_empty())
                        })
                    })
            }
        },
        event_types: Vec::new(),
    }
}

fn response_output_has_type(body: &Value, expected: &str) -> bool {
    body.get("output")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some(expected)
                    || item
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|content| {
                            content.iter().any(|part| {
                                part.get("type").and_then(Value::as_str) == Some(expected)
                            })
                        })
            })
        })
}

fn probe_token_usage(usage: &Value) -> Option<ProbeTokenUsage> {
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64);
    let reasoning_tokens = usage
        .pointer("/output_tokens_details/reasoning_tokens")
        .or_else(|| usage.pointer("/completion_tokens_details/reasoning_tokens"))
        .and_then(Value::as_u64);
    let total_tokens = usage.get("total_tokens").and_then(Value::as_u64);
    [input_tokens, output_tokens, reasoning_tokens, total_tokens]
        .iter()
        .any(Option::is_some)
        .then_some(ProbeTokenUsage {
            input_tokens,
            output_tokens,
            reasoning_tokens,
            total_tokens,
        })
}

pub(super) fn observe_sse_tool_event(
    protocol: ApiProtocol,
    event: &SseEvent,
    output: &mut GenerationOutputObservation,
) {
    if event.data() == "[DONE]" {
        return;
    }
    let Ok(document) = crate::bridge::strict_json::from_str(event.data()) else {
        output.tool_shape_valid = false;
        return;
    };
    match protocol {
        ApiProtocol::ChatCompletions => observe_chat_sse_tools(&document, output),
        ApiProtocol::Responses => observe_responses_sse_tools(event, &document, output),
    }
}

fn observe_chat_sse_tools(document: &Value, output: &mut GenerationOutputObservation) {
    let Some(choices) = document.get("choices").and_then(Value::as_array) else {
        return;
    };
    if choices
        .iter()
        .any(|choice| choice.get("finish_reason").and_then(Value::as_str) == Some("length"))
    {
        output.tool_shape_valid = false;
    }
    for call in choices
        .iter()
        .filter_map(|choice| {
            choice
                .pointer("/delta/tool_calls")
                .and_then(Value::as_array)
        })
        .flatten()
    {
        let Some(index) = call.get("index").and_then(Value::as_u64) else {
            output.tool_shape_valid = false;
            continue;
        };
        if call
            .get("type")
            .filter(|value| !value.is_null())
            .is_some_and(|value| value.as_str() != Some("function"))
        {
            output.tool_shape_valid = false;
            continue;
        }
        let Some(function) = call.get("function").and_then(Value::as_object) else {
            continue;
        };
        let valid = {
            let Some(accumulator) = stream_tool_call(output, format!("chat:{index}")) else {
                continue;
            };
            merge_stream_call_id(accumulator, call.get("id"))
                && merge_stream_tool_name(accumulator, function.get("name"))
                && append_stream_tool_arguments(accumulator, function.get("arguments"))
        };
        output.tool_shape_valid &= valid;
    }
}

fn observe_responses_sse_tools(
    event: &SseEvent,
    document: &Value,
    output: &mut GenerationOutputObservation,
) {
    let event_type = event
        .event()
        .or_else(|| document.get("type").and_then(Value::as_str));
    match event_type {
        Some("response.output_item.added" | "response.output_item.done") => {
            let Some(item) = document.get("item").and_then(Value::as_object) else {
                output.tool_shape_valid = false;
                return;
            };
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return;
            }
            let Some(key) = item
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(|id| format!("responses:{id}"))
            else {
                output.tool_shape_valid = false;
                return;
            };
            let Some(output_index) = document.get("output_index").and_then(Value::as_u64) else {
                output.tool_shape_valid = false;
                return;
            };
            let valid = {
                let Some(accumulator) = stream_tool_call(output, key) else {
                    return;
                };
                merge_stream_output_index(accumulator, output_index)
                    && merge_stream_call_id(accumulator, item.get("call_id"))
                    && merge_stream_tool_name(accumulator, item.get("name"))
                    && set_stream_tool_arguments(accumulator, item.get("arguments"))
            };
            output.tool_shape_valid &= valid;
        }
        Some("response.function_call_arguments.delta") => {
            let Some(key) = responses_item_key(document) else {
                output.tool_shape_valid = false;
                return;
            };
            let Some(output_index) = document.get("output_index").and_then(Value::as_u64) else {
                output.tool_shape_valid = false;
                return;
            };
            let valid = {
                let Some(accumulator) = stream_tool_call(output, key) else {
                    return;
                };
                merge_stream_output_index(accumulator, output_index)
                    && append_stream_tool_arguments(accumulator, document.get("delta"))
            };
            output.tool_shape_valid &= valid;
        }
        Some("response.function_call_arguments.done") => {
            let Some(key) = responses_item_key(document) else {
                output.tool_shape_valid = false;
                return;
            };
            let Some(output_index) = document.get("output_index").and_then(Value::as_u64) else {
                output.tool_shape_valid = false;
                return;
            };
            let valid = {
                let Some(accumulator) = stream_tool_call(output, key) else {
                    return;
                };
                merge_stream_output_index(accumulator, output_index)
                    && set_stream_tool_arguments(accumulator, document.get("arguments"))
            };
            output.tool_shape_valid &= valid;
        }
        _ => {}
    }
}

fn stream_tool_call(
    output: &mut GenerationOutputObservation,
    key: String,
) -> Option<&mut StreamToolCallObservation> {
    if !output.stream_tool_calls.contains_key(&key)
        && output.stream_tool_calls.len() >= MAX_OBSERVED_TOOL_CALLS
    {
        output.tool_shape_valid = false;
        return None;
    }
    Some(output.stream_tool_calls.entry(key).or_default())
}

fn merge_stream_output_index(
    accumulator: &mut StreamToolCallObservation,
    output_index: u64,
) -> bool {
    if accumulator
        .output_index
        .is_some_and(|existing| existing != output_index)
    {
        false
    } else {
        accumulator.output_index = Some(output_index);
        true
    }
}

fn merge_stream_call_id(
    accumulator: &mut StreamToolCallObservation,
    value: Option<&Value>,
) -> bool {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return true;
    };
    let Some(call_id) = value.as_str() else {
        return false;
    };
    if accumulator
        .call_id
        .as_deref()
        .is_some_and(|existing| existing != call_id)
    {
        false
    } else {
        accumulator.call_id = Some(call_id.to_owned());
        true
    }
}

fn merge_stream_tool_name(
    accumulator: &mut StreamToolCallObservation,
    value: Option<&Value>,
) -> bool {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return true;
    };
    let Some(name) = value.as_str() else {
        return false;
    };
    if accumulator
        .name
        .as_deref()
        .is_some_and(|existing| existing != name)
    {
        false
    } else {
        accumulator.name = Some(name.to_owned());
        true
    }
}

fn append_stream_tool_arguments(
    accumulator: &mut StreamToolCallObservation,
    value: Option<&Value>,
) -> bool {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return true;
    };
    let Some(fragment) = value.as_str() else {
        return false;
    };
    accumulator.arguments.push_str(fragment);
    true
}

fn set_stream_tool_arguments(
    accumulator: &mut StreamToolCallObservation,
    value: Option<&Value>,
) -> bool {
    let Some(arguments) = value.and_then(Value::as_str) else {
        return false;
    };
    if accumulator.arguments.is_empty()
        || accumulator.arguments == arguments
        || arguments.starts_with(&accumulator.arguments)
    {
        accumulator.arguments = arguments.to_owned();
        true
    } else {
        false
    }
}

fn responses_item_key(document: &Value) -> Option<String> {
    document
        .get("item_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(|id| format!("responses:{id}"))
}

pub(super) fn observe_sse_event(
    protocol: ApiProtocol,
    event: &SseEvent,
    evidence: &mut GenerationProbeEvidence,
    output_text: &mut String,
) -> Option<ProbeTerminal> {
    if protocol == ApiProtocol::ChatCompletions && event.data().trim() == "[DONE]" {
        return Some(ProbeTerminal::ChatDone);
    }

    let document = crate::bridge::strict_json::from_str(event.data()).ok();
    let event_type = event.event().map(str::to_owned).or_else(|| {
        document
            .as_ref()
            .and_then(|document| document.get("type"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    if let Some(event_type) = event_type.as_deref().filter(|value| safe_event_type(value)) {
        if evidence.event_types.len() < 64
            && !evidence.event_types.iter().any(|value| value == event_type)
        {
            evidence.event_types.push(event_type.to_owned());
        }
        evidence.reasoning_observed |= event_type.contains("reasoning");
        evidence.output_text_observed |= event_type.contains("output_text");
        // Summary-specific SSE events (e.g. response.reasoning_summary_part.done) mark an
        // observed summary without retaining any summary text.
        evidence.reasoning_summary_observed |=
            event_type.contains("reasoning") && event_type.contains("summary");
    }

    if let Some(document) = document.as_ref() {
        let usage = document.get("usage").or_else(|| {
            document
                .get("response")
                .and_then(|response| response.get("usage"))
        });
        evidence.usage_present |= usage.is_some_and(Value::is_object);
        if let Some(usage) = usage.and_then(probe_token_usage) {
            evidence.usage = Some(usage);
        }
        if protocol == ApiProtocol::ChatCompletions {
            let fragment = document
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| {
                    choices
                        .iter()
                        .find_map(|choice| choice.pointer("/delta/content").and_then(Value::as_str))
                });
            if let Some(fragment) = fragment {
                evidence.output_text_observed = true;
                output_text.push_str(fragment);
            }
        } else if event_type.as_deref() == Some("response.output_text.delta")
            && let Some(fragment) = document.get("delta").and_then(Value::as_str)
        {
            evidence.output_text_observed = true;
            output_text.push_str(fragment);
        }
    }

    match event_type.as_deref() {
        Some("response.completed") => Some(ProbeTerminal::ResponsesCompleted),
        Some("response.incomplete") => Some(ProbeTerminal::ResponsesIncomplete),
        Some("response.failed") => Some(ProbeTerminal::ResponsesFailed),
        _ => None,
    }
}

fn safe_event_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::probe::{ProbeCapabilityVerdict, ProbeGenerationCapability, ProbeTerminal};
    use crate::transport::sse::SseDecoder;

    #[test]
    fn structured_oracle_distinguishes_honored_ignored_and_incomplete_results() {
        let supported = generation_capability_evidence(
            ProbeGenerationCapability::JsonSchema,
            &GenerationOutputObservation::text(r#"{"probe":"ok"}"#),
            Some(ProbeTerminal::ResponsesCompleted),
        );
        assert_eq!(supported.verdict, ProbeCapabilityVerdict::Supported);
        assert_eq!(supported.valid_json_object, Some(true));
        assert_eq!(supported.fixed_schema_match, Some(true));

        let ignored = generation_capability_evidence(
            ProbeGenerationCapability::JsonObject,
            &GenerationOutputObservation::text("OK"),
            Some(ProbeTerminal::NonStreaming),
        );
        assert_eq!(ignored.verdict, ProbeCapabilityVerdict::NotHonored);
        assert_eq!(ignored.valid_json_object, Some(false));

        let incomplete = generation_capability_evidence(
            ProbeGenerationCapability::JsonSchemaStrict,
            &GenerationOutputObservation::text(r#"{"probe":"ok"}"#),
            Some(ProbeTerminal::ResponsesIncomplete),
        );
        assert_eq!(incomplete.verdict, ProbeCapabilityVerdict::Inconclusive);

        let mut truncated = GenerationOutputObservation::text(r#"{"probe":"ok"}"#);
        truncated.tool_shape_valid = false;
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::JsonSchemaStrict,
                &truncated,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );
    }

    #[test]
    fn image_oracle_is_supported_only_for_the_fixed_visible_token() {
        let supported = generation_capability_evidence(
            ProbeGenerationCapability::ImageInputInlinePng,
            &GenerationOutputObservation::text("OPENBRIDGE 7"),
            Some(ProbeTerminal::NonStreaming),
        );
        assert_eq!(supported.verdict, ProbeCapabilityVerdict::Supported);
        assert_eq!(supported.fixed_image_match, Some(true));

        let uncertain = generation_capability_evidence(
            ProbeGenerationCapability::ImageInputInlinePng,
            &GenerationOutputObservation::text("I cannot read the image"),
            Some(ProbeTerminal::NonStreaming),
        );
        assert_eq!(uncertain.verdict, ProbeCapabilityVerdict::Inconclusive);
        assert_eq!(uncertain.fixed_image_match, Some(false));

        let padded = generation_capability_evidence(
            ProbeGenerationCapability::ImageInputInlinePng,
            &GenerationOutputObservation::text("OPENBRIDGE 7\n"),
            Some(ProbeTerminal::NonStreaming),
        );
        assert_eq!(padded.verdict, ProbeCapabilityVerdict::Inconclusive);
        assert_eq!(padded.fixed_image_match, Some(false));
    }

    #[test]
    fn tool_auto_oracle_accepts_fixed_chat_and_responses_calls_without_retaining_arguments() {
        let fixtures = [
            (
                ApiProtocol::ChatCompletions,
                json!({
                    "choices": [{"message": {"tool_calls": [{
                        "type": "function",
                        "id": "call_private",
                        "function": {
                            "name": "openbridge_probe_primary",
                            "arguments": "{\"value\":\"primary\"}"
                        }
                    }]}}]
                }),
            ),
            (
                ApiProtocol::Responses,
                json!({
                    "object": "response",
                    "status": "completed",
                    "output": [{
                        "type": "function_call",
                        "id": "item_private",
                        "call_id": "call_private",
                        "name": "openbridge_probe_primary",
                        "arguments": "{\"value\":\"primary\"}"
                    }]
                }),
            ),
        ];

        for (protocol, body) in fixtures {
            let output = json_generation_output(protocol, &body);
            let evidence = generation_capability_evidence(
                ProbeGenerationCapability::ToolAuto,
                &output,
                Some(ProbeTerminal::NonStreaming),
            );
            assert_eq!(evidence.verdict, ProbeCapabilityVerdict::Supported);
            assert_eq!(evidence.tool_call_count, Some(1));
            assert_eq!(evidence.fixed_tool_match, Some(true));
            assert_eq!(evidence.fixed_arguments_match, Some(true));
        }

        let truncated = json_generation_output(
            ApiProtocol::ChatCompletions,
            &json!({
                "choices": [{
                    "finish_reason": "length",
                    "message": {"tool_calls": [{
                        "type": "function",
                        "function": {
                            "name": PRIMARY_TOOL_NAME,
                            "arguments": "{\"value\":\"primary\"}"
                        }
                    }]}
                }]
            }),
        );
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolAuto,
                &truncated,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );

        let missing_identity = json_generation_output(
            ApiProtocol::ChatCompletions,
            &json!({
                "choices": [{"message": {"tool_calls": [{
                    "type": "function",
                    "function": {
                        "name": PRIMARY_TOOL_NAME,
                        "arguments": "{\"value\":\"primary\"}"
                    }
                }]}}]
            }),
        );
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolAuto,
                &missing_identity,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );

        let calls = (0..17)
            .map(|index| {
                json!({
                    "type": "function",
                    "id": format!("call_{index}"),
                    "function": {
                        "name": PRIMARY_TOOL_NAME,
                        "arguments": "{\"value\":\"primary\"}"
                    }
                })
            })
            .collect::<Vec<_>>();
        let too_many = json_generation_output(
            ApiProtocol::ChatCompletions,
            &json!({"choices": [{"message": {"tool_calls": calls}}]}),
        );
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolAuto,
                &too_many,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );

        let multiple_choices = json_generation_output(
            ApiProtocol::ChatCompletions,
            &json!({
                "choices": [
                    {"message": {"tool_calls": []}},
                    {"message": {"tool_calls": []}}
                ]
            }),
        );
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolNone,
                &multiple_choices,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );
    }

    #[test]
    fn tool_choice_strict_and_parallel_oracles_are_case_specific() {
        let call = |name: &str, value: &str| ToolCallObservation {
            name: name.to_owned(),
            arguments: json!({"value": value}).to_string(),
        };
        let cases = [
            (
                ProbeGenerationCapability::ToolNone,
                GenerationOutputObservation::new(),
            ),
            (
                ProbeGenerationCapability::ToolRequired,
                GenerationOutputObservation {
                    text: String::new(),
                    tool_calls: vec![call(PRIMARY_TOOL_NAME, "primary")],
                    stream_tool_calls: BTreeMap::new(),
                    tool_shape_valid: true,
                },
            ),
            (
                ProbeGenerationCapability::ToolNamed,
                GenerationOutputObservation {
                    text: String::new(),
                    tool_calls: vec![call(PRIMARY_TOOL_NAME, "primary")],
                    stream_tool_calls: BTreeMap::new(),
                    tool_shape_valid: true,
                },
            ),
            (
                ProbeGenerationCapability::ToolStrict,
                GenerationOutputObservation {
                    text: String::new(),
                    tool_calls: vec![call(PRIMARY_TOOL_NAME, "primary")],
                    stream_tool_calls: BTreeMap::new(),
                    tool_shape_valid: true,
                },
            ),
            (
                ProbeGenerationCapability::ToolParallelDisabled,
                GenerationOutputObservation {
                    text: String::new(),
                    tool_calls: vec![call(PRIMARY_TOOL_NAME, "primary")],
                    stream_tool_calls: BTreeMap::new(),
                    tool_shape_valid: true,
                },
            ),
            (
                ProbeGenerationCapability::ToolParallelEnabled,
                GenerationOutputObservation {
                    text: String::new(),
                    tool_calls: vec![
                        call(PRIMARY_TOOL_NAME, "primary"),
                        call("openbridge_probe_secondary", "secondary"),
                    ],
                    stream_tool_calls: BTreeMap::new(),
                    tool_shape_valid: true,
                },
            ),
        ];

        for (capability, output) in cases {
            let evidence = generation_capability_evidence(
                capability,
                &output,
                Some(ProbeTerminal::NonStreaming),
            );
            assert_eq!(
                evidence.verdict,
                ProbeCapabilityVerdict::Supported,
                "{capability:?}"
            );
        }

        let only_one = GenerationOutputObservation {
            text: String::new(),
            tool_calls: vec![call(PRIMARY_TOOL_NAME, "primary")],
            stream_tool_calls: BTreeMap::new(),
            tool_shape_valid: true,
        };
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolParallelEnabled,
                &only_one,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );

        let duplicate_arguments = GenerationOutputObservation {
            text: String::new(),
            tool_calls: vec![ToolCallObservation {
                name: PRIMARY_TOOL_NAME.to_owned(),
                arguments: r#"{"value":"wrong","value":"primary"}"#.to_owned(),
            }],
            stream_tool_calls: BTreeMap::new(),
            tool_shape_valid: true,
        };
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolStrict,
                &duplicate_arguments,
                Some(ProbeTerminal::NonStreaming),
            )
            .verdict,
            ProbeCapabilityVerdict::NotHonored
        );
    }

    #[test]
    fn streaming_tool_oracle_assembles_chat_and_responses_argument_fragments() {
        let fixtures = [
            (
                ApiProtocol::ChatCompletions,
                concat!(
                    "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_private\",\"type\":\"function\",\"function\":{\"name\":\"openbridge_probe_primary\",\"arguments\":\"{\\\"value\\\":\"}}]},\"finish_reason\":null}]}\n\n",
                    "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"primary\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n"
                ),
            ),
            (
                ApiProtocol::Responses,
                concat!(
                    "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_private\",\"type\":\"function_call\",\"call_id\":\"call_private\",\"name\":\"openbridge_probe_primary\",\"arguments\":\"\"}}\n\n",
                    "event: response.function_call_arguments.delta\ndata: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_private\",\"output_index\":0,\"delta\":\"{\\\"value\\\":\"}\n\n",
                    "event: response.function_call_arguments.done\ndata: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc_private\",\"output_index\":0,\"arguments\":\"{\\\"value\\\":\\\"primary\\\"}\"}\n\n"
                ),
            ),
        ];

        for (protocol, stream) in fixtures {
            let mut decoder = SseDecoder::new(64 * 1024);
            let events = decoder.push(stream.as_bytes()).unwrap();
            let mut output = GenerationOutputObservation::new();
            for event in events {
                observe_sse_tool_event(protocol, &event, &mut output);
            }
            output.finish_stream_tool_calls();
            assert_eq!(
                generation_capability_evidence(
                    ProbeGenerationCapability::ToolAuto,
                    &output,
                    Some(ProbeTerminal::ChatDone),
                )
                .verdict,
                ProbeCapabilityVerdict::Supported,
                "{protocol:?}"
            );
        }

        let mut decoder = SseDecoder::new(64 * 1024);
        let event = decoder
            .push(b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"call_id\":\"call_private\",\"name\":\"openbridge_probe_primary\",\"arguments\":\"{\\\"value\\\":\\\"primary\\\"}\"}}\n\n")
            .unwrap()
            .remove(0);
        let mut output = GenerationOutputObservation::new();
        observe_sse_tool_event(ApiProtocol::Responses, &event, &mut output);
        output.finish_stream_tool_calls();
        assert_eq!(
            generation_capability_evidence(
                ProbeGenerationCapability::ToolAuto,
                &output,
                Some(ProbeTerminal::ResponsesCompleted),
            )
            .verdict,
            ProbeCapabilityVerdict::Inconclusive
        );
    }

    #[test]
    fn sse_summary_event_marks_summary_observed_without_retaining_text() {
        let mut decoder = SseDecoder::new(64 * 1024);
        let events = decoder
            .push(
                b"event: response.reasoning_summary_text.done\ndata: {\"type\":\"response.reasoning_summary_text.done\",\"item_id\":\"rs_private\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
            )
            .unwrap();
        let mut evidence = GenerationProbeEvidence::default();
        let mut output_text = String::new();
        for event in &events {
            observe_sse_event(
                ApiProtocol::Responses,
                event,
                &mut evidence,
                &mut output_text,
            );
        }
        assert!(evidence.reasoning_observed);
        assert!(evidence.reasoning_summary_observed);
        assert!(
            evidence
                .event_types
                .iter()
                .all(|value| !value.contains("must-not"))
        );
    }

    #[test]
    fn json_generation_evidence_detects_summary_parts_and_ignores_plain_reasoning() {
        let with_summary = json_generation_evidence(
            ApiProtocol::Responses,
            &json!({
                "object": "response",
                "status": "completed",
                "output": [
                    {"type": "reasoning", "summary": [{"type": "summary_text", "text": "private"}]},
                    {"type": "message", "content": [{"type": "output_text", "text": "OK"}]}
                ]
            }),
            None,
        );
        assert!(with_summary.reasoning_summary_observed);

        let without_summary = json_generation_evidence(
            ApiProtocol::Responses,
            &json!({
                "object": "response",
                "status": "completed",
                "output": [
                    {"type": "reasoning", "summary": []},
                    {"type": "message", "content": [{"type": "output_text", "text": "OK"}]}
                ]
            }),
            None,
        );
        assert!(!without_summary.reasoning_summary_observed);

        let chat_never = json_generation_evidence(
            ApiProtocol::ChatCompletions,
            &json!({
                "choices": [{"message": {"role": "assistant", "content": "OK"}}]
            }),
            None,
        );
        assert!(!chat_never.reasoning_summary_observed);
    }
}
