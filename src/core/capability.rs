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
    ALL_TOOL_CHOICE_MODES, AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputSource,
    AudioUnderstandingProfile, ChatCompletionsCapabilities, ChatCompletionsProfile,
    ChatFileInputProfile, ChatMediaProfile, ExecutableAudioProfile, ExecutableResponsesState,
    FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType, FunctionToolCapabilities,
    GeneratedAudioCapabilities, HostedToolKind, ImageDetail, ImageDetailPolicy, ImageDetailProfile,
    ImageInputCapabilities, ImageInputSource, ImageMediaType, ImageSourceCapabilities,
    InlineAudioInputLimits, InlineAudioInputProfile, InlineFileInputLimits, InlineFileInputProfile,
    InlineImageInputLimits, InlineImageInputProfile, JsonAudioDelivery, JsonAudioFraming,
    JsonSchemaSupport, PresetVoiceCapabilities, ProviderAudioCeiling,
    ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
    ProviderResponsesStateCeiling, ReasoningOutput, RemoteAudioInputProfile,
    RemoteImageInputLimits, ResponseInclude, ResponsesAffinity, ResponsesCapabilities,
    ResponsesFileInputProfile, ResponsesMediaProfile, ResponsesProfile, SpeechRecognitionProfile,
    SpeechSynthesisProfile, SseAudioDelivery, SseAudioFraming, StorageSupport,
    StructuredOutputMode, StructuredOutputProfile, ToolChoiceMode, VoiceCloneProfile,
    VoiceDesignProfile,
};

/// One operation-tagged capability ceiling in a Provider contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderOperationCapabilities {
    /// Chat Completions ceiling.
    ChatCompletions(&'static ProviderChatCompletionsCapabilities),
    /// Responses ceiling.
    Responses(&'static ProviderResponsesCapabilities),
    /// Embeddings Create ceiling.
    Embeddings(&'static EmbeddingsCapabilities),
}

impl ProviderOperationCapabilities {
    /// Returns the operation owned by this profile.
    pub const fn operation(self) -> crate::core::OperationKind {
        match self {
            Self::ChatCompletions(_) => crate::core::OperationKind::ChatCompletions,
            Self::Responses(_) => crate::core::OperationKind::Responses,
            Self::Embeddings(_) => crate::core::OperationKind::EmbeddingsCreate,
        }
    }

    /// Extracts a Chat Completions ceiling.
    pub const fn chat_completions(self) -> Option<ProviderChatCompletionsCapabilities> {
        match self {
            Self::ChatCompletions(capabilities) => Some(*capabilities),
            Self::Responses(_) | Self::Embeddings(_) => None,
        }
    }

    /// Extracts a Responses ceiling.
    pub const fn responses(self) -> Option<ProviderResponsesCapabilities> {
        match self {
            Self::Responses(capabilities) => Some(*capabilities),
            Self::ChatCompletions(_) | Self::Embeddings(_) => None,
        }
    }

    /// Extracts an Embeddings Create ceiling.
    pub const fn embeddings(self) -> Option<EmbeddingsCapabilities> {
        match self {
            Self::Embeddings(capabilities) => Some(*capabilities),
            Self::ChatCompletions(_) | Self::Responses(_) => None,
        }
    }
}

/// Operation-indexed capability ceilings for a Provider contract.
///
/// A Provider contract omits an unsupported operation profile. A present Upstream API may narrow
/// capabilities supported by its Provider contract but cannot enable unimplemented capabilities.
/// The request path uses a separately precompiled Public Model contract. Chat Completions,
/// Responses, and Embeddings remain separate so observations from one operation are not
/// incorrectly applied to another.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApiCapabilities {
    operations: [Option<ProviderOperationCapabilities>; crate::core::OperationKind::COUNT],
}

impl ApiCapabilities {
    /// Builds a validated set from unique operation-tagged profiles.
    pub const fn from_operations<const N: usize>(
        operations: [ProviderOperationCapabilities; N],
    ) -> Self {
        let mut indexed = [None; crate::core::OperationKind::COUNT];
        let mut position = 0;
        while position < N {
            let capabilities = operations[position];
            let index = capabilities.operation().index();
            assert!(
                indexed[index].is_none(),
                "duplicate Provider operation capability"
            );
            indexed[index] = Some(capabilities);
            position += 1;
        }
        Self {
            operations: indexed,
        }
    }

    /// Builds an already indexed set for const endpoint-surface projection.
    pub(crate) const fn from_indexed_operations(
        operations: [Option<ProviderOperationCapabilities>; crate::core::OperationKind::COUNT],
    ) -> Self {
        let mut index = 0;
        while index < crate::core::OperationKind::COUNT {
            if let Some(capabilities) = operations[index] {
                assert!(
                    capabilities.operation().index() == index,
                    "misindexed Provider operation capability"
                );
            }
            index += 1;
        }
        Self { operations }
    }

    /// Returns the ceiling for one typed operation.
    pub const fn operation(
        self,
        operation: crate::core::OperationKind,
    ) -> Option<ProviderOperationCapabilities> {
        self.operations[operation.index()]
    }
}
