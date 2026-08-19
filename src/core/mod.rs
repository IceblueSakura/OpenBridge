//! OpenBridge request protocols and capability models.
//!
//! This module defines provider-independent protocol and capability value objects only. It does not
//! parse HTTP, select Routes, or rewrite request bodies, keeping protocol facts separate from Provider implementations.

mod capability;
mod generation_parameter;
mod request;

pub(crate) use capability::GenerationCapabilities;
pub use capability::{
    ALL_TOOL_CHOICE_MODES, ApiCapabilities, AsrLanguage, AudioFormat, AudioInputCapabilities,
    AudioInputLimits, AudioInputSource, AudioUnderstandingProfile, ChatCompletionsCapabilities,
    ChatCompletionsProfile, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
    EmbeddingsCapabilities, ExecutableAudioProfile, ExecutableResponsesState,
    FunctionToolCapabilities, GeneratedAudioCapabilities, HostedToolKind, ImageDetail,
    ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageInputSource,
    ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
    JsonAudioDelivery, JsonAudioFraming, JsonSchemaSupport, PresetVoiceCapabilities,
    ProviderAudioCeiling, ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
    ProviderResponsesStateCeiling, ReasoningOutput, RemoteImageInputLimits, ResponseInclude,
    ResponsesAffinity, ResponsesCapabilities, ResponsesProfile, SpeechRecognitionProfile,
    SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming, StorageSupport,
    StructuredOutputMode, StructuredOutputProfile, ToolChoiceMode, VoiceCloneProfile,
    VoiceDesignProfile,
};
pub(crate) use generation_parameter::{
    ChatStreamUsage, GenerationRequestField, parse_chat_stream_usage,
};
pub use request::{
    ApiProtocol, ApiRequest, EmbeddingRequest, GenerationBridgeDirection, OperationKind,
};
