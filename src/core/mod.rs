//! OpenBridge request protocols and capability models.
//!
//! This module defines provider-independent protocol and capability value objects only. It does not
//! parse HTTP, select Routes, or rewrite request bodies, keeping protocol facts separate from Provider implementations.

mod capability;
mod generation_parameter;
mod request;

pub(crate) use capability::GenerationCapabilities;
pub use capability::{
    ALL_STRUCTURED_OUTPUT_MODES, ALL_TOOL_CHOICE_MODES, ApiCapabilities, AudioCapabilities,
    AudioFormat, AudioInputCapabilities, AudioInputSource, AudioOutputCapabilities, AudioTask,
    ChatCompletionsCapabilities, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
    EmbeddingsCapabilities, FunctionToolCapabilities, HostedToolKind, ImageDetail,
    ImageInputCapabilities, ImageInputSource, ImageMediaType, ReasoningOutput, ResponseInclude,
    ResponsesCapabilities, StructuredOutputMode, StructuredOutputProfile, ToolChoiceMode,
};
pub(crate) use generation_parameter::GenerationRequestField;
pub use request::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind};
