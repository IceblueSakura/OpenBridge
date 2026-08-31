//! Bounded protocol evidence extraction for administrative Generation probes.

use std::time::Instant;

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
        reasoning_effort: selection.reasoning_effort,
        capability: selection.capability,
        upstream_model: upstream_model.map(str::to_owned),
        elapsed_ms,
        outcome,
        evidence,
        capability_evidence,
    }
}

/// Extracts transient output text from one recognized non-streaming protocol envelope.
pub(super) fn json_generation_output_text(protocol: ApiProtocol, body: &Value) -> Option<String> {
    match protocol {
        ApiProtocol::ChatCompletions => body
            .get("choices")?
            .as_array()?
            .iter()
            .find_map(|choice| choice.pointer("/message/content").and_then(Value::as_str))
            .map(str::to_owned),
        ApiProtocol::Responses => {
            let mut output = String::new();
            for part in body
                .get("output")?
                .as_array()?
                .iter()
                .filter_map(|item| item.get("content").and_then(Value::as_array))
                .flatten()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
            {
                if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                    output.push_str(fragment);
                }
            }
            (!output.is_empty()).then_some(output)
        }
    }
}

/// Derives one semantic conclusion without retaining the transient generated text.
pub(super) fn generation_capability_evidence(
    capability: ProbeGenerationCapability,
    output_text: Option<&str>,
    terminal: Option<ProbeTerminal>,
) -> ProbeCapabilityEvidence {
    if terminal == Some(ProbeTerminal::ResponsesIncomplete) || terminal.is_none() {
        return ProbeCapabilityEvidence {
            verdict: ProbeCapabilityVerdict::Inconclusive,
            valid_json_object: None,
            fixed_schema_match: None,
        };
    }
    match capability {
        ProbeGenerationCapability::Text => ProbeCapabilityEvidence {
            verdict: if output_text.is_some_and(|output| !output.trim().is_empty()) {
                ProbeCapabilityVerdict::Supported
            } else {
                ProbeCapabilityVerdict::NotHonored
            },
            valid_json_object: None,
            fixed_schema_match: None,
        },
        ProbeGenerationCapability::JsonObject
        | ProbeGenerationCapability::JsonSchema
        | ProbeGenerationCapability::JsonSchemaStrict => {
            let document =
                output_text.and_then(|output| serde_json::from_str::<Value>(output).ok());
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
                ProbeGenerationCapability::Text => unreachable!(),
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
            }
        }
    }
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

pub(super) fn observe_sse_event(
    protocol: ApiProtocol,
    event: &SseEvent,
    evidence: &mut GenerationProbeEvidence,
    output_text: &mut String,
) -> Option<ProbeTerminal> {
    if protocol == ApiProtocol::ChatCompletions && event.data().trim() == "[DONE]" {
        return Some(ProbeTerminal::ChatDone);
    }

    let document = serde_json::from_str::<Value>(event.data()).ok();
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
    use super::*;
    use crate::probe::{ProbeCapabilityVerdict, ProbeGenerationCapability, ProbeTerminal};

    #[test]
    fn structured_oracle_distinguishes_honored_ignored_and_incomplete_results() {
        let supported = generation_capability_evidence(
            ProbeGenerationCapability::JsonSchema,
            Some(r#"{"probe":"ok"}"#),
            Some(ProbeTerminal::ResponsesCompleted),
        );
        assert_eq!(supported.verdict, ProbeCapabilityVerdict::Supported);
        assert_eq!(supported.valid_json_object, Some(true));
        assert_eq!(supported.fixed_schema_match, Some(true));

        let ignored = generation_capability_evidence(
            ProbeGenerationCapability::JsonObject,
            Some("OK"),
            Some(ProbeTerminal::NonStreaming),
        );
        assert_eq!(ignored.verdict, ProbeCapabilityVerdict::NotHonored);
        assert_eq!(ignored.valid_json_object, Some(false));

        let incomplete = generation_capability_evidence(
            ProbeGenerationCapability::JsonSchemaStrict,
            Some(r#"{"probe":"ok"}"#),
            Some(ProbeTerminal::ResponsesIncomplete),
        );
        assert_eq!(incomplete.verdict, ProbeCapabilityVerdict::Inconclusive);
    }
}
