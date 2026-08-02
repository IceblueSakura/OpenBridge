//! Chat Completions 与 Responses 流式语义的显式 bridge 状态机门面。
//!
//! Chat 与 Responses 状态机分别位于私有子模块，共享 terminal、tool identity 和错误契约。
//! `conversion` 子模块提供生产 `BridgePlan` 与双向 renderer。本模块不执行 tool、不持久化
//! continuation ledger，也不转换未进入显式 allowlist 的 Provider 私有语义。

use thiserror::Error;

mod chat;
mod conversion;
mod responses;
mod shared;

pub use chat::ChatStreamState;
pub use conversion::{BridgeError, BridgePlan, BridgeStreamRenderer};
pub use responses::ResponsesStreamState;

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
