//! Bounded protocol evidence extraction for administrative Generation probes.

use std::time::Instant;

use serde_json::Value;

use crate::{core::ApiProtocol, registry::UpstreamApi, transport::sse::SseEvent};

use super::super::{
    GenerationProbeEvidence, GenerationProbeResult, ProbeGenerationMode, ProbeProtocol,
    ProbeReasoningEffort, ProbeResult, ProbeTerminal, ProbeTokenUsage,
};

pub(super) fn generation_result(
    protocol: ApiProtocol,
    mode: ProbeGenerationMode,
    reasoning_effort: ProbeReasoningEffort,
    upstream_model: Option<&str>,
    elapsed_ms: u64,
    outcome: ProbeResult,
    evidence: Option<GenerationProbeEvidence>,
) -> GenerationProbeResult {
    GenerationProbeResult {
        protocol: ProbeProtocol::from_api(protocol),
        mode,
        reasoning_effort,
        upstream_model: upstream_model.map(str::to_owned),
        elapsed_ms,
        outcome,
        evidence,
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
            evidence.output_text_observed |= document
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        choice
                            .get("delta")
                            .and_then(|delta| delta.get("content"))
                            .is_some_and(|content| !content.is_null())
                    })
                });
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
