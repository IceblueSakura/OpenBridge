//! OpenBridge request protocols and capability models.
//!
//! This module defines provider-independent protocol and capability value objects only. It does not
//! parse HTTP, select Routes, or rewrite request bodies, keeping protocol facts separate from Provider implementations.

mod capability;
mod request;

pub(crate) use capability::GenerationCapabilities;
pub use capability::{
    ApiCapabilities, ChatCompletionsCapabilities, EmbeddingDimensionDomain, EmbeddingEncoding,
    EmbeddingInputForm, EmbeddingsCapabilities, HostedToolKind, ImageDetail,
    ImageInputCapabilities, ImageInputSource, ImageMediaType, ReasoningOutput, ResponseInclude,
    ResponsesCapabilities,
};
pub use request::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind};
