//! Fixed JSON requests and protocol-response shape checks for capability probes.
//!
//! This module generates only built-in prompts, function schemas, and tool-result replays. It
//! accepts no external URL, model selection, or arbitrary request body.

use serde_json::{Value, json};

use crate::core::ApiProtocol;

const PROBE_PROMPT: &str = "Reply with exactly OK.";
const TOOL_NAME: &str = "openbridge_probe";

/// Builds the minimum non-streaming text probe request.
pub(super) fn probe_text_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
) -> Value {
    match protocol {
        ApiProtocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        ApiProtocol::Responses => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
    }
}

/// Builds the fixed function-tool definition for the target protocol.
fn tool_definition(protocol: ApiProtocol) -> Value {
    match protocol {
        ApiProtocol::ChatCompletions => json!({
            "type": "function",
            "function": {
                "name": TOOL_NAME,
                "description": "Return a deterministic local probe value.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                },
            },
        }),
        ApiProtocol::Responses => json!({
            "type": "function",
            "name": TOOL_NAME,
            "description": "Return a deterministic local probe value.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        }),
    }
}

/// Builds the first probe request that requires the fixed function call.
pub(super) fn probe_tool_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
) -> Value {
    let tools = vec![tool_definition(protocol)];
    match protocol {
        ApiProtocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": "Call the openbridge_probe function."}],
            "tools": tools,
            "tool_choice": {"type": "function", "function": {"name": TOOL_NAME}},
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        ApiProtocol::Responses => json!({
            "model": model,
            "input": "Call the openbridge_probe function.",
            "tools": tools,
            "tool_choice": {"type": "function", "name": TOOL_NAME},
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
    }
}

/// Extracts stable tool identity from the first response and builds the result-replay request.
pub(super) fn tool_result_replay_request(
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
    response: &Value,
) -> Option<Value> {
    match protocol {
        ApiProtocol::ChatCompletions => {
            let message = response.pointer("/choices/0/message")?.clone();
            let tool_calls = message.get("tool_calls")?.as_array()?;
            let call = tool_calls.iter().find(|call| {
                call.pointer("/function/name").and_then(Value::as_str) == Some(TOOL_NAME)
            })?;
            let call_id = call.get("id")?.as_str()?;
            let arguments = call.pointer("/function/arguments")?.as_str()?;
            serde_json::from_str::<Value>(arguments).ok()?;
            Some(json!({
                "model": model,
                "messages": [
                    {"role": "user", "content": "Call the openbridge_probe function."},
                    message,
                    {"role": "tool", "tool_call_id": call_id, "content": "{\"ok\":true}"},
                ],
                "tools": [tool_definition(protocol)],
                "max_completion_tokens": max_output_tokens,
                "stream": false,
            }))
        }
        ApiProtocol::Responses => {
            let output = response.get("output")?.as_array()?;
            let call = output.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("name").and_then(Value::as_str) == Some(TOOL_NAME)
            })?;
            let call_id = call.get("call_id")?.as_str()?;
            let arguments = call.get("arguments")?.as_str()?;
            serde_json::from_str::<Value>(arguments).ok()?;
            Some(json!({
                "model": model,
                "input": [{
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": "{\"ok\":true}",
                }],
                "tools": [tool_definition(protocol)],
                "max_output_tokens": max_output_tokens,
                "store": false,
                "stream": false,
            }))
        }
    }
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
