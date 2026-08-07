//! Directional facade for Chat Completions and Responses request conversion.
//!
//! Each protocol direction maintains its own message/tool ledger; a dedicated validation module
//! enforces the top-level field allowlist. All submodules continue to fail closed on unmodeled semantics.

mod chat_to_responses;
mod responses_to_chat;
mod structured;
mod validation;

pub(super) use chat_to_responses::chat_request_to_responses;
pub(super) use responses_to_chat::responses_request_to_chat;
pub(super) use validation::reject_unsupported_request;
