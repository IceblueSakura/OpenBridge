//! Responses SSE event 的 bridge 生命周期状态机。
//!
//! 本模块固定 response、output item 与 function call identity，逐项累计 text 和 arguments，
//! 并要求 completed terminal 前所有 output item 已明确完成。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::transport::sse::SseEvent;

use super::{
    BridgeStreamError, BridgeToolCall, Lifecycle, StreamTerminal,
    shared::{required_str, required_u64, validate_arguments},
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ResponsesItem {
    Message {
        item_id: String,
        text: String,
        completed: bool,
    },
    Tool(BridgeToolCall),
}

/// 按 Responses SSE event 生命周期重建 bridge 语义的单请求状态。
#[derive(Debug)]
pub struct ResponsesStreamState {
    lifecycle: Lifecycle,
    response_id: Option<String>,
    items: BTreeMap<u64, ResponsesItem>,
}

impl ResponsesStreamState {
    /// 创建等待 `response.created` 的空状态机。
    pub fn new() -> Self {
        Self {
            lifecycle: Lifecycle::AwaitingStart,
            response_id: None,
            items: BTreeMap::new(),
        }
    }

    /// 消费一个已完成 framing 的 Responses SSE event。
    pub fn ingest(&mut self, event: &SseEvent) -> Result<(), BridgeStreamError> {
        // 解析 JSON 并统一校验 SSE event 名称与 payload type。
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeStreamError::InvalidJson)?;
        let payload_type = required_str(&value, "type")?;
        if event.event().is_some_and(|name| name != payload_type) {
            return Err(BridgeStreamError::EventTypeConflict);
        }

        // 按显式事件类型推进 lifecycle 与 output item 状态。
        match payload_type {
            "response.created" => self.on_created(&value),
            "response.output_item.added" => self.on_item_added(&value),
            "response.output_text.delta" => self.on_text_delta(&value),
            "response.function_call_arguments.delta" => self.on_arguments_delta(&value),
            "response.function_call_arguments.done" => self.on_arguments_done(&value),
            "response.output_item.done" => self.on_item_done(&value),
            "response.completed" => self.on_terminal(&value, StreamTerminal::Completed),
            "response.failed" => self.on_terminal(&value, StreamTerminal::Failed),
            "response.incomplete" => self.on_terminal(&value, StreamTerminal::Incomplete),
            "error" => self.on_error(),
            _ => Err(BridgeStreamError::UnexpectedEvent),
        }
    }

    /// 在上游 EOF 时验证唯一 terminal 已出现。
    pub fn finish(&self) -> Result<(), BridgeStreamError> {
        match self.lifecycle {
            Lifecycle::Terminal(_) => Ok(()),
            Lifecycle::AwaitingStart | Lifecycle::Streaming => {
                Err(BridgeStreamError::EofBeforeTerminal)
            }
        }
    }

    /// 返回已经确认的唯一终态。
    pub fn terminal(&self) -> Option<StreamTerminal> {
        match self.lifecycle {
            Lifecycle::Terminal(terminal) => Some(terminal),
            Lifecycle::AwaitingStart | Lifecycle::Streaming => None,
        }
    }

    /// 返回按 output index 拼接的 assistant 文本。
    pub fn text(&self) -> String {
        self.items
            .values()
            .filter_map(|item| match item {
                ResponsesItem::Message { text, .. } => Some(text.as_str()),
                ResponsesItem::Tool(_) => None,
            })
            .collect()
    }

    /// 返回按 output index 排序的 function tool calls。
    pub fn tool_calls(&self) -> Vec<&BridgeToolCall> {
        self.items
            .values()
            .filter_map(|item| match item {
                ResponsesItem::Tool(tool) => Some(tool),
                ResponsesItem::Message { .. } => None,
            })
            .collect()
    }

    /// 接受唯一 `response.created` 并固定 response identity。
    fn on_created(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 只允许创建一次 response，并固定 response identity。
        if self.lifecycle != Lifecycle::AwaitingStart {
            return Err(BridgeStreamError::UnexpectedEvent);
        }
        let response = value
            .get("response")
            .ok_or(BridgeStreamError::InvalidJson)?;
        self.response_id = Some(required_str(response, "id")?.to_owned());
        self.lifecycle = Lifecycle::Streaming;
        Ok(())
    }

    /// 注册一个具有唯一 index、item id 与 call id 的 output item。
    fn on_item_added(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 校验 stream 已开始并提取稳定 output identity。
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        if self.items.contains_key(&output_index) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }
        let item = value.get("item").ok_or(BridgeStreamError::InvalidJson)?;
        let item_id = required_str(item, "id")?.to_owned();
        if self.items.values().any(|known| match known {
            ResponsesItem::Message { item_id: known, .. } => known == &item_id,
            ResponsesItem::Tool(tool) => tool.item_id.as_deref() == Some(item_id.as_str()),
        }) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }

        // 按 item 类型初始化独立的文本或 tool-call accumulator。
        let parsed = match required_str(item, "type")? {
            "message" => ResponsesItem::Message {
                item_id,
                text: String::new(),
                completed: false,
            },
            "function_call" => {
                let call_id = required_str(item, "call_id")?.to_owned();
                if self.items.values().any(
                    |known| matches!(known, ResponsesItem::Tool(tool) if tool.call_id == call_id),
                ) {
                    return Err(BridgeStreamError::DuplicateIdentity);
                }
                ResponsesItem::Tool(BridgeToolCall {
                    output_index,
                    item_id: Some(item_id),
                    call_id,
                    name: required_str(item, "name")?.to_owned(),
                    arguments: item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    completed: false,
                })
            }
            _ => return Err(BridgeStreamError::UnexpectedEvent),
        };
        self.items.insert(output_index, parsed);
        Ok(())
    }

    /// 将 text delta 绑定到已注册且未完成的 message item。
    fn on_text_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 将 delta 绑定到已经注册的 message item，不以 index 猜测 identity。
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let delta = required_str(value, "delta")?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Message {
                item_id: known,
                text,
                completed: false,
            }) if known == item_id => {
                text.push_str(delta);
                Ok(())
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// 将 arguments delta 绑定到已注册且未完成的 function call item。
    fn on_arguments_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 将 arguments 分片绑定到稳定 item id，并拒绝完成后的追加。
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let delta = required_str(value, "delta")?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Tool(tool))
                if tool.item_id.as_deref() == Some(item_id) && !tool.completed =>
            {
                tool.arguments.push_str(delta);
                Ok(())
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// 对照累计值校验完整 function arguments 快照。
    fn on_arguments_done(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 验证上游 done 快照与累计 arguments 完全一致且已经形成 JSON object。
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let item_id = required_str(value, "item_id")?;
        let arguments = required_str(value, "arguments")?;
        match self.items.get(&output_index) {
            Some(ResponsesItem::Tool(tool))
                if tool.item_id.as_deref() == Some(item_id) && tool.arguments == arguments =>
            {
                validate_arguments(arguments)
            }
            Some(_) => Err(BridgeStreamError::IdentityConflict),
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// 对照完整快照验证并完成指定 output item。
    fn on_item_done(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 对照完整 item 快照验证 identity 与累计内容，再标记完成。
        self.ensure_streaming()?;
        let output_index = required_u64(value, "output_index")?;
        let snapshot = value.get("item").ok_or(BridgeStreamError::InvalidJson)?;
        match self.items.get_mut(&output_index) {
            Some(ResponsesItem::Message {
                item_id,
                text,
                completed,
            }) => {
                if *completed || required_str(snapshot, "id")? != item_id {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                let snapshot_text = snapshot
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| content.iter().find_map(|part| part.get("text")))
                    .and_then(Value::as_str)
                    .ok_or(BridgeStreamError::InvalidJson)?;
                if snapshot_text != text {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                *completed = true;
                Ok(())
            }
            Some(ResponsesItem::Tool(tool)) => {
                if tool.completed
                    || required_str(snapshot, "id")? != tool.item_id.as_deref().unwrap_or_default()
                    || required_str(snapshot, "call_id")? != tool.call_id
                    || required_str(snapshot, "name")? != tool.name
                    || required_str(snapshot, "arguments")? != tool.arguments
                {
                    return Err(BridgeStreamError::IdentityConflict);
                }
                validate_arguments(&tool.arguments)?;
                tool.completed = true;
                Ok(())
            }
            None => Err(BridgeStreamError::UnknownOutputItem),
        }
    }

    /// 接受显式 Responses terminal，并验证 response identity 与完成边界。
    fn on_terminal(
        &mut self,
        value: &Value,
        terminal: StreamTerminal,
    ) -> Result<(), BridgeStreamError> {
        // 拒绝重复 terminal，并验证 terminal response identity 未漂移。
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::DuplicateTerminal);
        }
        self.ensure_streaming()?;
        let response = value
            .get("response")
            .ok_or(BridgeStreamError::InvalidJson)?;
        if required_str(response, "id")? != self.response_id.as_deref().unwrap_or_default() {
            return Err(BridgeStreamError::IdentityConflict);
        }

        // 成功 terminal 必须等待所有 output item 完成；失败终态不伪造完成状态。
        if terminal == StreamTerminal::Completed
            && self.items.values().any(|item| match item {
                ResponsesItem::Message { completed, .. } => !completed,
                ResponsesItem::Tool(tool) => !tool.completed,
            })
        {
            return Err(BridgeStreamError::IncompleteOutputItem);
        }
        self.lifecycle = Lifecycle::Terminal(terminal);
        Ok(())
    }

    /// 接受独立 `error` event，并把它固定为当前 response 的失败终态。
    fn on_error(&mut self) -> Result<(), BridgeStreamError> {
        // 拒绝在既有 terminal 后追加独立 error event。
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::DuplicateTerminal);
        }

        // 独立 error 只有在 response 已创建后才能结束当前 stream。
        self.ensure_streaming()?;
        self.lifecycle = Lifecycle::Terminal(StreamTerminal::Error);
        Ok(())
    }

    /// 确认当前状态允许消费普通 Responses output event。
    fn ensure_streaming(&self) -> Result<(), BridgeStreamError> {
        match self.lifecycle {
            Lifecycle::Streaming => Ok(()),
            Lifecycle::AwaitingStart | Lifecycle::Terminal(_) => {
                Err(BridgeStreamError::UnexpectedEvent)
            }
        }
    }
}

impl Default for ResponsesStreamState {
    fn default() -> Self {
        Self::new()
    }
}
