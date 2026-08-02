//! Chat Completions 与 Responses 请求转换的方向门面。
//!
//! 两个协议方向分别维护自身的 message/tool ledger；顶层字段 allowlist 由独立 validation
//! 模块统一执行。所有子模块继续对未建模语义 fail closed。

mod chat_to_responses;
mod responses_to_chat;
mod validation;

pub(super) use chat_to_responses::chat_request_to_responses;
pub(super) use responses_to_chat::responses_request_to_chat;
pub(super) use validation::reject_unsupported_request;
