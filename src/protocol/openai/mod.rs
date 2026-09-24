//! Pure codecs for the explicitly supported Generation migration slice.
pub mod chat;
mod common;
pub mod envelope;
pub mod events;
mod function_tools;
mod json;
mod reasoning;
pub mod responses;
mod settings;
pub mod sse;
mod static_response;
mod terminal;
mod text;

use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::task::generation::{GenerationError, GenerationRequest, GenerationResponse},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Chat,
    Responses,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedRequest {
    pub semantic: GenerationRequest,
    pub fidelity: FidelityRecords,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedResponse {
    pub semantic: GenerationResponse,
    pub fidelity: FidelityRecords,
    pub metadata: ResponseMetadata,
}
/// Envelope identity is not a task instruction or a routing input.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseMetadata {
    pub id: String,
    pub model: String,
    pub created: serde_json::Number,
    pub context: envelope::ResponseContext,
}
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CodecError {
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("unsupported field or representation: {0}")]
    Unsupported(String),
    #[error("codec input exceeds slice limits")]
    Limit,
    #[error("target representation belongs to another codec")]
    ProfileMismatch,
    #[error(transparent)]
    Semantic(#[from] GenerationError),
    #[error(transparent)]
    Event(#[from] crate::semantic::task::generation::EventError),
}

/// Only lowering can construct an encoding input; codecs cannot bypass representability.
pub struct RequestRepresentation<'a> {
    pub(crate) semantic: &'a GenerationRequest,
    pub(crate) fidelity: &'a FidelityRecords,
    pub(crate) profile: Profile,
}
pub struct ResponseRepresentation<'a> {
    pub(crate) semantic: &'a GenerationResponse,
    pub(crate) fidelity: &'a FidelityRecords,
    pub(crate) metadata: &'a ResponseMetadata,
    pub(crate) profile: Profile,
}
