//! Chat Completions 与 Responses 流式语义的显式 bridge 状态机。
//!
//! 本模块只维护单个请求内的文本、tool identity、arguments 与 terminal 生命周期。它不执行
//! tool、不持久化 continuation ledger，也不把 bridge 自动接入 Route；调用方必须在受限
//! `BridgePlan` 中显式选择方向，并在渲染目标 wire 前处理这里返回的确定状态。

use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::transport::sse::SseEvent;

/// bridge stream 的唯一终态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamTerminal {
    /// 上游协议明确报告成功完成。
    Completed,
    /// Responses 明确报告失败。
    Failed,
    /// Responses 明确报告未完整完成。
    Incomplete,
    /// Responses 以独立 `error` event 报告失败。
    Error,
}

/// bridge stream 生命周期或 identity 校验失败。
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BridgeStreamError {
    /// SSE data 不是合法 JSON。
    #[error("bridge event data is not valid JSON")]
    InvalidJson,
    /// SSE `event` 与 JSON `type` 不一致。
    #[error("SSE event name conflicts with the JSON event type")]
    EventTypeConflict,
    /// 当前阶段不接受该事件。
    #[error("bridge event is not valid in the current lifecycle state")]
    UnexpectedEvent,
    /// output index、item id 或 call id 被重复注册。
    #[error("bridge event repeats an existing identity")]
    DuplicateIdentity,
    /// 后续分片试图替换已固定的 identity。
    #[error("bridge event conflicts with an established identity")]
    IdentityConflict,
    /// delta 引用了尚未注册的 output item。
    #[error("bridge event references an unknown output item")]
    UnknownOutputItem,
    /// function arguments 不是已闭合的 JSON object。
    #[error("function tool arguments are incomplete or not a JSON object")]
    InvalidToolArguments,
    /// terminal 到达时仍有未完成的 output item。
    #[error("bridge terminal arrived before all output items completed")]
    IncompleteOutputItem,
    /// stream 出现多个 terminal。
    #[error("bridge stream contains more than one terminal")]
    DuplicateTerminal,
    /// 输入 EOF 不能替代协议 terminal。
    #[error("bridge stream ended before an explicit terminal")]
    EofBeforeTerminal,
}

/// bridge 状态机重建的一个 function tool call。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeToolCall {
    output_index: u64,
    item_id: Option<String>,
    call_id: String,
    name: String,
    arguments: String,
    completed: bool,
}

impl BridgeToolCall {
    /// 返回协议内的 output index。
    pub fn output_index(&self) -> u64 {
        self.output_index
    }

    /// 返回 Responses item id；Chat 原生流没有该 identity。
    pub fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }

    /// 返回跨 tool result 往返使用的稳定 call id。
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// 返回 function tool 名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回按 wire 顺序拼接且已校验闭合的 arguments。
    pub fn arguments(&self) -> &str {
        &self.arguments
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    AwaitingStart,
    Streaming,
    Terminal(StreamTerminal),
}

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

/// 按 Chat Completions SSE chunk 生命周期重建 bridge 语义的单请求状态。
#[derive(Debug)]
pub struct ChatStreamState {
    lifecycle: Lifecycle,
    finish_reason_seen: bool,
    text: String,
    tools: BTreeMap<u64, BridgeToolCall>,
}

impl ChatStreamState {
    /// 创建等待第一个 Chat chunk 的空状态机。
    pub fn new() -> Self {
        Self {
            lifecycle: Lifecycle::AwaitingStart,
            finish_reason_seen: false,
            text: String::new(),
            tools: BTreeMap::new(),
        }
    }

    /// 消费一个已完成 framing 的 Chat SSE event。
    pub fn ingest(&mut self, event: &SseEvent) -> Result<(), BridgeStreamError> {
        // 单独处理 Chat `[DONE]` terminal，并拒绝缺少 finish reason 的提前结束。
        if event.data() == "[DONE]" {
            if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
                return Err(BridgeStreamError::DuplicateTerminal);
            }
            if !self.finish_reason_seen {
                return Err(BridgeStreamError::IncompleteOutputItem);
            }
            self.validate_all_arguments()?;
            self.lifecycle = Lifecycle::Terminal(StreamTerminal::Completed);
            return Ok(());
        }
        if matches!(self.lifecycle, Lifecycle::Terminal(_)) {
            return Err(BridgeStreamError::UnexpectedEvent);
        }

        // 解析单 choice chunk，并按 delta 顺序更新文本与 tool accumulators。
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeStreamError::InvalidJson)?;
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(BridgeStreamError::InvalidJson)?;
        if choices.len() != 1 || required_u64(&choices[0], "index")? != 0 {
            return Err(BridgeStreamError::UnexpectedEvent);
        }
        self.lifecycle = Lifecycle::Streaming;
        let choice = &choices[0];
        let delta = choice.get("delta").ok_or(BridgeStreamError::InvalidJson)?;
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            self.text.push_str(content);
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                self.ingest_tool_delta(tool_call)?;
            }
        }

        // finish reason 固定输出结束语义，但 `[DONE]` 才是 Chat stream terminal。
        if !choice.get("finish_reason").is_none_or(Value::is_null) {
            if self.finish_reason_seen {
                return Err(BridgeStreamError::DuplicateTerminal);
            }
            self.finish_reason_seen = true;
            self.validate_all_arguments()?;
        }
        Ok(())
    }

    /// 在上游 EOF 时验证 `[DONE]` 已出现。
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

    /// 返回按 chunk 顺序拼接的 assistant 文本。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 返回按 Chat tool index 排序的 function tool calls。
    pub fn tool_calls(&self) -> Vec<&BridgeToolCall> {
        self.tools.values().collect()
    }

    fn ingest_tool_delta(&mut self, value: &Value) -> Result<(), BridgeStreamError> {
        // 用 index 关联同一流内分片，但首次出现时固定 call id 与 name。
        let index = required_u64(value, "index")?;
        let function = value
            .get("function")
            .ok_or(BridgeStreamError::InvalidJson)?;
        if let Some(existing) = self.tools.get_mut(&index) {
            if value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id != existing.call_id)
                || function
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name != existing.name)
            {
                return Err(BridgeStreamError::IdentityConflict);
            }
            if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                existing.arguments.push_str(arguments);
            }
            return Ok(());
        }

        // 新 tool index 必须同时携带稳定 call id 与 function name。
        let call_id = required_str(value, "id")?.to_owned();
        if self.tools.values().any(|tool| tool.call_id == call_id) {
            return Err(BridgeStreamError::DuplicateIdentity);
        }
        self.tools.insert(
            index,
            BridgeToolCall {
                output_index: index,
                item_id: None,
                call_id,
                name: required_str(function, "name")?.to_owned(),
                arguments: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                completed: false,
            },
        );
        Ok(())
    }

    fn validate_all_arguments(&mut self) -> Result<(), BridgeStreamError> {
        // 在 finish reason 或 terminal 边界验证所有 arguments 并标记完成。
        for tool in self.tools.values_mut() {
            validate_arguments(&tool.arguments)?;
            tool.completed = true;
        }
        Ok(())
    }
}

impl Default for ChatStreamState {
    fn default() -> Self {
        Self::new()
    }
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(BridgeStreamError::InvalidJson)
}

fn required_u64(value: &Value, field: &str) -> Result<u64, BridgeStreamError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(BridgeStreamError::InvalidJson)
}

fn validate_arguments(arguments: &str) -> Result<(), BridgeStreamError> {
    // function arguments 必须是完整 JSON object，不能仅凭字符串结束位置推断完成。
    let parsed: Value =
        serde_json::from_str(arguments).map_err(|_| BridgeStreamError::InvalidToolArguments)?;
    if parsed.is_object() {
        Ok(())
    } else {
        Err(BridgeStreamError::InvalidToolArguments)
    }
}
