//! Chat Completions SSE chunk 的 bridge 生命周期状态机。
//!
//! 本模块按 tool index 累计 function call identity 与 arguments，并要求 finish reason 后出现
//! 显式 `[DONE]` terminal；EOF 不会被视为成功完成。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::transport::sse::SseEvent;

use super::{
    BridgeStreamError, BridgeToolCall, Lifecycle, StreamTerminal,
    shared::{required_str, required_u64, validate_arguments},
};

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

    /// 累计一个 tool-call delta，并固定首次出现的 identity。
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

    /// 在 finish reason 或 terminal 边界验证并完成所有 tool arguments。
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
