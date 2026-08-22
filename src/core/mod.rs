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
    AudioInputSource, AudioUnderstandingProfile, ChatCompletionsCapabilities,
    ChatCompletionsProfile, ChatFileInputProfile, ChatMediaProfile, EmbeddingDimensionDomain,
    EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities, ExecutableAudioProfile,
    ExecutableResponsesState, FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType,
    FunctionToolCapabilities, GeneratedAudioCapabilities, HostedToolKind, ImageDetail,
    ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageInputSource,
    ImageMediaType, ImageSourceCapabilities, ImagesGenerationsCapabilities, ImagesResponseFormat,
    ImagesSizeDomain, InlineAudioInputLimits, InlineAudioInputProfile, InlineFileInputLimits,
    InlineFileInputProfile, InlineImageInputLimits, InlineImageInputProfile, JsonAudioDelivery,
    JsonAudioFraming, JsonSchemaSupport, PresetVoiceCapabilities, ProviderAudioCeiling,
    ProviderChatCompletionsCapabilities, ProviderOperationCapabilities,
    ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
    RemoteAudioInputProfile, RemoteImageInputLimits, ResponseInclude, ResponsesAffinity,
    ResponsesCapabilities, ResponsesFileInputProfile, ResponsesMediaProfile, ResponsesProfile,
    SpeechRecognitionProfile, SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming,
    StorageSupport, StructuredOutputMode, StructuredOutputProfile, ToolChoiceMode,
    VoiceCloneProfile, VoiceDesignProfile,
};
pub(crate) use generation_parameter::{
    ChatStreamUsage, GenerationRequestField, parse_chat_stream_usage,
};
pub use request::{
    ApiProtocol, ApiRequest, EmbeddingRequest, GenerationBridgeDirection, ImagesRequest,
    OperationKind,
};
