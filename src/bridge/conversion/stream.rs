//! Chat Completions 与 Responses SSE renderer 的方向门面。
//!
//! 两个协议方向分别维护独立增量状态；共享模块只负责编码目标 SSE wire block。

mod chat_to_responses;
mod responses_to_chat;
mod shared;

pub(super) use chat_to_responses::ChatToResponsesStream;
pub(super) use responses_to_chat::ResponsesToChatStream;
