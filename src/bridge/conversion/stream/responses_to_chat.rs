//! Incremental conversion from Responses SSE events to Chat Completions chunks.
//!
//! The renderer first uses the strict Responses state machine to fix lifecycle and identities,
//! then maps text/function argument deltas to Chat chunks and emits `[DONE]` only after a completed terminal.

use std::collections::BTreeMap;

use bytes::Bytes;
use serde_json::{Map, Value, json};

use crate::{bridge::ResponsesStreamState, transport::sse::SseEvent};

use super::{
    super::{
        BridgeError,
        shared::{map_id, required_string},
    },
    shared::sse_data,
};

/// Converts one Responses SSE lifecycle increment into Chat chunks.
pub(in crate::bridge::conversion) struct ResponsesToChatStream {
    state: ResponsesStreamState,
    reasoning_supported: bool,
    chat_id: Option<String>,
    role_emitted: bool,
    tool_indices: BTreeMap<u64, u64>,
    has_text: bool,
    has_reasoning: bool,
    has_tools: bool,
    terminal_emitted: bool,
}

impl ResponsesToChatStream {
    /// Creates a renderer waiting for `response.created`.
    pub(in crate::bridge::conversion) fn new(reasoning_supported: bool) -> Self {
        Self {
            state: ResponsesStreamState::new(),
            reasoning_supported,
            chat_id: None,
            role_emitted: false,
            tool_indices: BTreeMap::new(),
            has_text: false,
            has_reasoning: false,
            has_tools: false,
            terminal_emitted: false,
        }
    }

    /// Validates and converts one complete Responses SSE event.
    pub(in crate::bridge::conversion) fn render(
        &mut self,
        event: SseEvent,
        public_model: &str,
    ) -> Result<Bytes, BridgeError> {
        // Let the strict state machine validate event/type, identity, and terminal before generating target wire data.
        self.state.ingest(&event)?;
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeError::InvalidStream)?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidStream)?;
        let mut output = Vec::new();
        match kind {
            "response.created" => {
                let response = value
                    .get("response")
                    .and_then(Value::as_object)
                    .ok_or(BridgeError::InvalidStream)?;
                let id = required_string(response, "id")?;
                self.chat_id = Some(map_id(&id, "resp_", "chatcmpl_"));
            }
            "response.output_item.added" => {
                let index = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .ok_or(BridgeError::InvalidStream)?;
                let item = value
                    .get("item")
                    .and_then(Value::as_object)
                    .ok_or(BridgeError::InvalidStream)?;
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    self.has_tools = true;
                    let chat_index = self.tool_indices.len() as u64;
                    self.tool_indices.insert(index, chat_index);
                    let delta = json!({
                        "role": if self.role_emitted { Value::Null } else { Value::String("assistant".to_owned()) },
                        "tool_calls": [{
                            "function": {"arguments": "", "name": required_string(item, "name")?},
                            "id": required_string(item, "call_id")?,
                            "index": chat_index,
                            "type": "function"
                        }]
                    });
                    self.role_emitted = true;
                    output.extend(chat_chunk(
                        self.chat_id()?,
                        public_model,
                        strip_null_role(delta),
                        Value::Null,
                    )?);
                } else if item.get("type").and_then(Value::as_str) == Some("reasoning")
                    && !self.reasoning_supported
                {
                    return Err(BridgeError::UnsupportedSemantics);
                }
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                if !self.reasoning_supported {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                self.has_reasoning = true;
                let reasoning = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or(BridgeError::InvalidStream)?;
                let mut delta = Map::new();
                if !self.role_emitted {
                    delta.insert("role".to_owned(), Value::String("assistant".to_owned()));
                    self.role_emitted = true;
                }
                delta.insert(
                    "reasoning_content".to_owned(),
                    Value::String(reasoning.to_owned()),
                );
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    Value::Object(delta),
                    Value::Null,
                )?);
            }
            "response.reasoning_summary_part.added"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.done" => {}
            "response.output_text.delta" => {
                self.has_text = true;
                let text = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or(BridgeError::InvalidStream)?;
                let mut delta = Map::new();
                if !self.role_emitted {
                    delta.insert("role".to_owned(), Value::String("assistant".to_owned()));
                    self.role_emitted = true;
                }
                delta.insert("content".to_owned(), Value::String(text.to_owned()));
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    Value::Object(delta),
                    Value::Null,
                )?);
            }
            "response.function_call_arguments.delta" => {
                let index = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .ok_or(BridgeError::InvalidStream)?;
                let chat_index = self
                    .tool_indices
                    .get(&index)
                    .copied()
                    .ok_or(BridgeError::InvalidToolIdentity)?;
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    json!({"tool_calls": [{
                        "function": {"arguments": value.get("delta").cloned().ok_or(BridgeError::InvalidStream)?},
                        "index": chat_index
                    }]}),
                    Value::Null,
                )?);
            }
            "response.completed" => {
                let finish = if self.has_tools { "tool_calls" } else { "stop" };
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    json!({}),
                    Value::String(finish.to_owned()),
                )?);
                output.extend_from_slice(b"data: [DONE]\n\n");
                self.terminal_emitted = true;
            }
            "response.failed" | "response.incomplete" | "error" => {
                return Err(BridgeError::InvalidStream);
            }
            _ => {}
        }
        Ok(Bytes::from(output))
    }

    /// Confirms that the state machine and target Chat terminal are complete at EOF.
    pub(in crate::bridge::conversion) fn finish(&mut self) -> Result<Bytes, BridgeError> {
        self.state.finish()?;
        if !self.terminal_emitted || (!self.has_text && !self.has_reasoning && !self.has_tools) {
            return Err(BridgeError::InvalidStream);
        }
        Ok(Bytes::new())
    }

    /// Returns the Chat completion ID derived from the Responses response ID.
    fn chat_id(&self) -> Result<&str, BridgeError> {
        self.chat_id.as_deref().ok_or(BridgeError::InvalidStream)
    }
}

/// Removes the null role used for internal assembly to keep the Chat delta wire shape minimal.
fn strip_null_role(mut value: Value) -> Value {
    // Remove the null role used only for internal assembly to avoid a meaningless Chat field.
    if value.get("role").is_some_and(Value::is_null) {
        value
            .as_object_mut()
            .expect("delta is object")
            .remove("role");
    }
    value
}

/// Wraps a Chat delta as a data-only SSE chunk.
fn chat_chunk(
    id: &str,
    model: &str,
    delta: Value,
    finish_reason: Value,
) -> Result<Vec<u8>, BridgeError> {
    // Wrap the single-choice Chat chunk and encode it as a data-only SSE block.
    sse_data(&json!({
        "choices": [{"delta": delta, "finish_reason": finish_reason, "index": 0}],
        "id": id,
        "model": model,
        "object": "chat.completion.chunk"
    }))
}
