//! Chat Completions SSE chunks 到 typed Responses events 的增量转换。
//!
//! renderer 先调用严格 Chat 状态机固定 lifecycle 与 call identity，再生成 Responses item、
//! arguments 和 completed events；`[DONE]` 到达前不会生成目标 terminal。

use std::collections::{BTreeMap, btree_map::Entry};

use bytes::Bytes;
use serde_json::{Value, json};

use crate::{bridge::ChatStreamState, transport::sse::SseEvent};

use super::{
    super::{
        BridgeError,
        shared::{bridge_item_id, id_suffix, map_id, required_string, validate_arguments},
    },
    shared::response_event,
};

/// 将单个 Chat SSE 生命周期增量转换为 typed Responses events。
pub(in crate::bridge::conversion) struct ChatToResponsesStream {
    state: ChatStreamState,
    response_id: Option<String>,
    message_id: Option<String>,
    created_emitted: bool,
    message_started: bool,
    text: String,
    calls: BTreeMap<u64, StreamCall>,
    finish_reason: Option<String>,
    terminal_emitted: bool,
}

struct StreamCall {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    completed: bool,
}

impl ChatToResponsesStream {
    /// 创建等待首个 Chat chunk 的 renderer。
    pub(in crate::bridge::conversion) fn new() -> Self {
        Self {
            state: ChatStreamState::new(),
            response_id: None,
            message_id: None,
            created_emitted: false,
            message_started: false,
            text: String::new(),
            calls: BTreeMap::new(),
            finish_reason: None,
            terminal_emitted: false,
        }
    }

    /// 校验并转换一个完整 Chat SSE event。
    pub(in crate::bridge::conversion) fn render(
        &mut self,
        event: SseEvent,
        public_model: &str,
    ) -> Result<Bytes, BridgeError> {
        // 先用严格 Chat 状态机固定 index/call identity 和 DONE 终态。
        self.state.ingest(&event)?;
        if event.data() == "[DONE]" {
            return self.render_done(public_model);
        }
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeError::InvalidStream)?;
        let upstream_id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidStream)?;
        let response_id = self
            .response_id
            .get_or_insert_with(|| map_id(upstream_id, "chatcmpl_", "resp_"))
            .clone();
        self.message_id
            .get_or_insert_with(|| format!("msg_{}", id_suffix(upstream_id, "chatcmpl_")));
        let mut output = Vec::new();
        if !self.created_emitted {
            output.extend(response_event(
                "response.created",
                json!({
                    "response": {"id": response_id, "model": public_model, "object": "response", "output": [], "status": "in_progress"},
                    "type": "response.created"
                }),
            )?);
            self.created_emitted = true;
        }

        // 将单 choice Chat delta 展开为 typed Responses lifecycle events。
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            if !self.calls.is_empty() {
                return Err(BridgeError::UnsupportedSemantics);
            }
            if !self.message_started {
                output.extend(response_event(
                    "response.output_item.added",
                    json!({
                        "item": {"content": [], "id": self.message_id(), "role": "assistant", "status": "in_progress", "type": "message"},
                        "output_index": 0,
                        "type": "response.output_item.added"
                    }),
                )?);
                self.message_started = true;
            }
            self.text.push_str(content);
            output.extend(response_event(
                "response.output_text.delta",
                json!({
                    "content_index": 0,
                    "delta": content,
                    "item_id": self.message_id(),
                    "output_index": 0,
                    "type": "response.output_text.delta"
                }),
            )?);
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            if self.message_started {
                return Err(BridgeError::UnsupportedSemantics);
            }
            for tool_call in tool_calls {
                output.extend(self.render_tool_delta(tool_call)?);
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_owned());
            output.extend(self.render_item_completion()?);
        }
        Ok(Bytes::from(output))
    }

    /// 注册或追加一个 Chat tool-call delta，并生成对应 Responses events。
    fn render_tool_delta(&mut self, tool_call: &Value) -> Result<Vec<u8>, BridgeError> {
        // 固定新 tool call 的 index 与 identity，再累计当前 arguments 分片。
        let tool_call = tool_call.as_object().ok_or(BridgeError::InvalidStream)?;
        let index = tool_call
            .get("index")
            .and_then(Value::as_u64)
            .ok_or(BridgeError::InvalidStream)?;
        let function = tool_call
            .get("function")
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        let mut output = Vec::new();
        if let Entry::Vacant(entry) = self.calls.entry(index) {
            let call_id = required_string(tool_call, "id")?;
            let name = required_string(function, "name")?;
            let item_id = bridge_item_id(&call_id);
            entry.insert(StreamCall {
                item_id: item_id.clone(),
                call_id: call_id.clone(),
                name: name.clone(),
                arguments: String::new(),
                completed: false,
            });
            output.extend(response_event(
                "response.output_item.added",
                json!({
                    "item": {"arguments": "", "call_id": call_id, "id": item_id, "name": name, "status": "in_progress", "type": "function_call"},
                    "output_index": index,
                    "type": "response.output_item.added"
                }),
            )?);
        }
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !arguments.is_empty() {
            let call = self.calls.get_mut(&index).expect("call inserted above");
            call.arguments.push_str(arguments);
            output.extend(response_event(
                "response.function_call_arguments.delta",
                json!({
                    "delta": arguments,
                    "item_id": call.item_id,
                    "output_index": index,
                    "type": "response.function_call_arguments.delta"
                }),
            )?);
        }
        Ok(output)
    }

    /// 按 Chat finish reason 生成 Responses output item 完成事件。
    fn render_item_completion(&mut self) -> Result<Vec<u8>, BridgeError> {
        // 按 finish reason 完成全部 tool items 或唯一 message item。
        let mut output = Vec::new();
        if self.finish_reason.as_deref() == Some("tool_calls") {
            for (index, call) in &mut self.calls {
                validate_arguments(&call.arguments)?;
                output.extend(response_event(
                    "response.function_call_arguments.done",
                    json!({
                        "arguments": call.arguments,
                        "item_id": call.item_id,
                        "output_index": index,
                        "type": "response.function_call_arguments.done"
                    }),
                )?);
                output.extend(response_event(
                    "response.output_item.done",
                    json!({
                        "item": call_value(call),
                        "output_index": index,
                        "type": "response.output_item.done"
                    }),
                )?);
                call.completed = true;
            }
        } else if self.message_started {
            output.extend(response_event(
                "response.output_item.done",
                json!({
                    "item": self.message_value(),
                    "output_index": 0,
                    "type": "response.output_item.done"
                }),
            )?);
        }
        Ok(output)
    }

    /// 将 Chat `[DONE]` 转换为唯一 Responses completed terminal。
    fn render_done(&mut self, public_model: &str) -> Result<Bytes, BridgeError> {
        // 验证 Chat 已给出 finish reason，再构造唯一 Responses completed terminal。
        if !self.created_emitted || self.finish_reason.is_none() {
            return Err(BridgeError::InvalidStream);
        }
        let output_items = if self.calls.is_empty() {
            vec![self.message_value()]
        } else {
            self.calls.values().map(call_value).collect()
        };
        let bytes = response_event(
            "response.completed",
            json!({
                "response": {
                    "id": self.response_id.as_deref().ok_or(BridgeError::InvalidStream)?,
                    "model": public_model,
                    "object": "response",
                    "output": output_items,
                    "status": "completed"
                },
                "type": "response.completed"
            }),
        )?;
        self.terminal_emitted = true;
        Ok(Bytes::from(bytes))
    }

    /// 在 EOF 时确认状态机与目标 Responses terminal 均已完成。
    pub(in crate::bridge::conversion) fn finish(&mut self) -> Result<Bytes, BridgeError> {
        self.state.finish()?;
        if !self.terminal_emitted {
            return Err(BridgeError::InvalidStream);
        }
        Ok(Bytes::new())
    }

    fn message_id(&self) -> &str {
        self.message_id
            .as_deref()
            .expect("message id follows response id")
    }

    fn message_value(&self) -> Value {
        json!({
            "content": [{"annotations": [], "text": self.text, "type": "output_text"}],
            "id": self.message_id(),
            "role": "assistant",
            "status": "completed",
            "type": "message"
        })
    }
}

fn call_value(call: &StreamCall) -> Value {
    json!({
        "arguments": call.arguments,
        "call_id": call.call_id,
        "id": call.item_id,
        "name": call.name,
        "status": "completed",
        "type": "function_call"
    })
}
