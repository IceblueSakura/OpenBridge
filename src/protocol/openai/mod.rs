//! Pure codecs for the explicitly supported Generation migration slice.
mod accounting_shapes;
pub(crate) mod adapter_shapes;
pub mod chat;
pub mod chat_envelope;
pub(crate) mod chat_logprobs;
mod chat_reasoning;
pub mod chat_sse;
pub(crate) mod client;
mod common;
pub mod envelope;
pub mod events;
mod function_tools;
mod image;
mod inference_shapes;
pub(crate) mod json;
mod reasoning;
pub mod responses;
mod settings;
pub mod sse;
pub(crate) mod static_response;
mod terminal;
mod text;
#[cfg(test)]
mod tool_results_test;

pub use crate::protocol::{CodecError, DecodedRequest, DecodedResponse, ResponseMetadata};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::task::generation::{GenerationRequest, GenerationResponse},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Chat,
    Responses,
}
/// Only lowering can construct an encoding input; codecs cannot bypass representability.
pub struct RequestRepresentation<'a> {
    pub(crate) adaptation: crate::protocol::adaptation::Adaptation,
    pub(crate) semantic: &'a GenerationRequest,
    pub(crate) fidelity: &'a FidelityRecords,
    pub(crate) profile: Profile,
}
pub struct ResponseRepresentation<'a> {
    pub(crate) adaptation: crate::protocol::adaptation::Adaptation,
    pub(crate) semantic: &'a GenerationResponse,
    pub(crate) fidelity: &'a FidelityRecords,
    pub(crate) metadata: &'a ResponseMetadata,
    pub(crate) profile: Profile,
}
