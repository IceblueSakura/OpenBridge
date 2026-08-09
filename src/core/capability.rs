//! Provider-independent capability ceilings grouped by operation family.
//!
//! Generation and Embeddings capabilities live in private submodules because their wire fields,
//! validation, and subset rules are independent. This facade preserves one provider-independent
//! API and combines the domains only in [`ApiCapabilities`].

mod embeddings;
mod generation;

pub use embeddings::{
    EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
};
pub(crate) use generation::GenerationCapabilities;
pub use generation::{
    ALL_TOOL_CHOICE_MODES, AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputLimits,
    AudioInputSource, AudioUnderstandingProfile, ChatCompletionsCapabilities,
    ChatCompletionsProfile, ExecutableAudioProfile, ExecutableResponsesState,
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

/// Protocol-specific capability ceilings for a Provider contract.
///
/// A Provider contract omits an unsupported operation profile. A present Upstream API may narrow
/// capabilities supported by its Provider contract but cannot enable unimplemented capabilities.
/// The request path uses a separately precompiled Public Model contract. Chat Completions,
/// Responses, and Embeddings remain separate so observations from one operation are not
/// incorrectly applied to another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApiCapabilities {
    /// Capability ceiling for Chat Completions, or `None` when the operation is unsupported.
    pub chat_completions: Option<ProviderChatCompletionsCapabilities>,
    /// Capability ceiling for Responses, or `None` when the operation is unsupported.
    pub responses: Option<ProviderResponsesCapabilities>,
    /// Capability ceiling for Embeddings Create, or `None` when the operation is unsupported.
    pub embeddings: Option<EmbeddingsCapabilities>,
}
