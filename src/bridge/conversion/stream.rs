//! Directional facade for Chat Completions and Responses SSE renderers.
//!
//! Each protocol direction maintains independent incremental state; the shared module only encodes
//! target SSE wire blocks.

mod chat_to_responses;
mod responses_to_chat;
mod shared;

pub(super) use chat_to_responses::ChatToResponsesStream;
pub(super) use responses_to_chat::ResponsesToChatStream;
