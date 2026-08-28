//! Fixed downstream Public Model contracts and their private execution snapshot.
//!
//! Client-visible DTOs remain in this facade. Private submodules own execution candidates and
//! startup compilation so serialized responses cannot acquire Provider, Target, Route,
//! upstream-model, or credential topology by accident.

use serde::{Serialize, Serializer};

use crate::core::{
    AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputSource,
    ChatCompletionsCapabilities, ChatFileInputProfile, DashScopeImagesCapabilities,
    EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, ExecutableAudioProfile,
    FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType, GeneratedAudioCapabilities,
    ImageDetail, ImageDetailPolicy, ImageInputCapabilities, ImageInputSource, ImageMediaType,
    ImageSourceCapabilities, ImagesResponseFormat, ImagesSizeDomain, InlineAudioInputProfile,
    InlineImageInputProfile, JsonAudioFraming, ReasoningOutput, RemoteAudioInputProfile,
    ResponseInclude, ResponsesCapabilities, ResponsesFileInputProfile, SseAudioFraming,
    StructuredOutputProfile,
};

pub use crate::core::{StructuredOutputMode, ToolChoiceMode};

use super::{
    CanonicalTaskKind, InputModality, ModelContextLength, ModelLifecycle, OutputModality,
    ReasoningLevel, ReasoningLevelPolicy, ReasoningSupport,
};

mod compiler;
mod embeddings;
mod execution;
mod generation;
mod images;
mod media;

pub(super) use compiler::{PublicRouteBinding, compile_public_model};
pub use execution::PublicModel;
pub(crate) use execution::{ModelExecutionInterface, OperationResponseBudget};

pub use embeddings::{
    EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities, EmbeddingInterfaceCapabilities,
    EmbeddingLimits,
};
pub use generation::ModelInterfaceCapabilities;
pub use images::ImagesInterfaceCapabilities;
pub use media::{
    AudioInputInterfaceCapabilities, AudioInputLimits, AudioInterfaceCapabilities,
    AudioOutputInterfaceCapabilities, ImageInputInterfaceCapabilities, MultimodalInputCapabilities,
    MultimodalOutputCapabilities,
};
use media::{AudioTaskProjection, InterfaceMediaCapabilities};
pub(crate) use media::{FileInputInterfaceCapabilities, FileInputSource};

/// Stable schema version for the extended model-information object.
pub const MODEL_INFO_SCHEMA_VERSION: &str = "1";

/// Capability evidence state; `unknown` cannot count as supported during request preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportState {
    /// Every executable Route explicitly supports the capability.
    Supported,
    /// At least one executable Route explicitly does not support the capability.
    Unsupported,
    /// Current static facts are insufficient for a safe decision.
    Unknown,
}

impl SupportState {
    /// Converts an explicit Boolean capability into a public state.
    const fn from_bool(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }

    /// Returns whether the request path can treat the capability as guaranteed.
    pub(crate) const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Computes the conservative intersection of complete Route contracts.
    fn intersection(values: impl Iterator<Item = Self>) -> Self {
        let mut saw_value = false;
        let mut saw_unknown = false;
        for value in values {
            saw_value = true;
            match value {
                Self::Unsupported => return Self::Unsupported,
                Self::Unknown => saw_unknown = true,
                Self::Supported => {}
            }
        }
        if !saw_value || saw_unknown {
            Self::Unknown
        } else {
            Self::Supported
        }
    }
}

impl From<ReasoningSupport> for SupportState {
    fn from(value: ReasoningSupport) -> Self {
        match value {
            ReasoningSupport::Supported => Self::Supported,
            ReasoningSupport::Unsupported => Self::Unsupported,
            ReasoningSupport::Unknown => Self::Unknown,
        }
    }
}

/// Task categories a Public Model can perform.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTask {
    /// Conversational generation.
    Chat,
    /// General text generation.
    TextGeneration,
    /// Embedding-vector generation.
    Embedding,
    /// Image generation.
    ImageGeneration,
    /// Speech audio transcription.
    SpeechRecognition,
    /// Ordinary text-to-speech synthesis.
    SpeechSynthesis,
    /// Speech synthesis from a natural-language voice description.
    VoiceDesign,
    /// Speech synthesis conditioned on a reference voice recording.
    VoiceClone,
}

impl ModelTask {
    /// Projects one validated canonical task into its stable downstream task labels.
    fn from_canonical(task: CanonicalTaskKind) -> Vec<Self> {
        match task {
            CanonicalTaskKind::Generation => vec![Self::Chat, Self::TextGeneration],
            CanonicalTaskKind::Embedding => vec![Self::Embedding],
            CanonicalTaskKind::ImageGeneration => vec![Self::ImageGeneration],
            CanonicalTaskKind::SpeechRecognition => vec![Self::SpeechRecognition],
            CanonicalTaskKind::SpeechSynthesis => vec![Self::SpeechSynthesis],
            CanonicalTaskKind::VoiceDesign => vec![Self::VoiceDesign],
            CanonicalTaskKind::VoiceClone => vec![Self::VoiceClone],
        }
    }
}

/// Public Model context-window limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContextWindow {
    max_context_tokens: Option<u32>,
    max_input_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
}

impl ContextWindow {
    /// Builds the public object from the three registry-internal limits.
    const fn from_model(value: ModelContextLength) -> Self {
        Self {
            max_context_tokens: value.context_tokens(),
            max_input_tokens: value.input_tokens(),
            max_output_tokens: value.output_tokens(),
        }
    }

    /// Returns the maximum output-token count guaranteed by the public contract.
    pub(crate) const fn max_output_tokens(self) -> Option<u32> {
        self.max_output_tokens
    }
}

/// Confirmed Public Model input and output modalities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelModalities {
    input: Vec<InputModality>,
    output: Vec<OutputModality>,
}

/// Reasoning capabilities of the model itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelReasoningCapabilities {
    support: SupportState,
    levels: Vec<ReasoningLevel>,
}

/// Reasoning output form observable through the interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningOutputMode {
    /// The upstream explicitly returns no reasoning output.
    Unsupported,
    /// Returns readable complete reasoning text.
    PlainText,
    /// Returns only a readable reasoning summary.
    Summary,
    /// Returns an unreadable opaque continuation.
    Opaque,
    /// Current evidence is insufficient to determine the output form.
    Unknown,
}

impl From<ReasoningOutput> for ReasoningOutputMode {
    fn from(value: ReasoningOutput) -> Self {
        match value {
            ReasoningOutput::Unsupported => Self::Unsupported,
            ReasoningOutput::PlainText => Self::PlainText,
            ReasoningOutput::Summary => Self::Summary,
            ReasoningOutput::Opaque => Self::Opaque,
            ReasoningOutput::Unknown => Self::Unknown,
        }
    }
}

/// Public capability summary of the model itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelCapabilities {
    tasks: Vec<ModelTask>,
    context_window: ContextWindow,
    modalities: ModelModalities,
    tokenizer: Option<String>,
    knowledge_cutoff: Option<String>,
    reasoning: ModelReasoningCapabilities,
}

/// Public Model function-tool capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolCapabilities {
    support: SupportState,
    types: Vec<ToolType>,
    tool_choice_modes: Vec<ToolChoiceMode>,
    parallel_calls: SupportState,
    strict_schema: SupportState,
}

/// Tool kinds that downstream clients may declare.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// JSON-schema function tool.
    Function,
}

/// Reasoning capabilities of one downstream interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InterfaceReasoningCapabilities {
    support: SupportState,
    levels: Vec<ReasoningLevel>,
    accepted_levels: Vec<ReasoningLevel>,
    input_policy: ReasoningLevelPolicy,
    output: ReasoningOutputMode,
}

/// Persistent-state capabilities of one downstream interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StateCapabilities {
    store: SupportState,
    previous_response_id: SupportState,
    background: SupportState,
}

/// Read-only wire projection of explicit and omitted `detail` behavior.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImageDetailCapabilities {
    default: Option<ImageDetail>,
    allowed: Vec<ImageDetail>,
}

/// Read-only wire projection of image-input limits for one fixed interface.
///
/// A zero source-specific value means that source is absent from the projected source union; it is
/// never accepted as registry configuration or used by request preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImageInputLimits {
    max_parts: u32,
    max_url_length: u32,
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

/// Typed OpenAI-compatible operation contracts of a Public Model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaces {
    chat_completions: Option<ModelInterfaceCapabilities>,
    responses: Option<ModelInterfaceCapabilities>,
    embeddings: Option<EmbeddingInterfaceCapabilities>,
    images: Option<ImagesInterfaceCapabilities>,
}

/// Strict four-field projection of the standard OpenAI Models resource.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StandardModel {
    id: String,
    object: &'static str,
    created: u64,
    owned_by: &'static str,
}

impl StandardModel {
    /// Returns the stable downstream Public Model ID.
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Complete Public Model information returned by the OpenBridge extension interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublicModelInfo {
    schema_version: &'static str,
    #[serde(flatten)]
    standard: StandardModel,
    name: String,
    description: Option<String>,
    lifecycle: ModelLifecycle,
    capabilities: ModelCapabilities,
    interfaces: ModelInterfaces,
}

impl PublicModelInfo {
    /// Returns the standard OpenAI four-field projection.
    pub fn standard(&self) -> &StandardModel {
        &self.standard
    }
}
