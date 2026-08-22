//! Static definitions compiled into the registry.

use std::time::Duration;

use serde::Serialize;

use crate::{
    core::{
        ApiCapabilities, ApiProtocol, ChatCompletionsCapabilities, EmbeddingsCapabilities,
        GenerationBridgeDirection, GenerationCapabilities, ImagesGenerationsCapabilities,
        OperationKind, ResponsesCapabilities,
    },
    provider::{CredentialKind, ProviderKind},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Evidence state for model reasoning capability.
pub enum ReasoningSupport {
    #[default]
    /// The configuration lacks enough evidence to determine reasoning support.
    Unknown,
    /// The model explicitly supports reasoning.
    Supported,
    /// The model explicitly does not support reasoning.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
/// Reasoning levels supported by a model.
pub enum ReasoningLevel {
    /// Explicitly disables reasoning.
    None,
    /// Minimum reasoning level.
    Minimal,
    /// Low reasoning level.
    Low,
    /// Medium reasoning level.
    Medium,
    /// High reasoning level.
    High,
    /// Extra-high reasoning level.
    #[serde(rename = "xhigh")]
    XHigh,
    /// Maximum reasoning level.
    Max,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Public Model policy for accepting and resolving downstream reasoning levels.
pub enum ReasoningLevelPolicy {
    /// Accepts only reasoning levels in the fixed executable interface contract.
    #[default]
    Strict,
    /// Floors positive levels within the executable set and clamps values below its minimum.
    ClampPositiveFloor,
}

const POSITIVE_REASONING_LEVELS: [ReasoningLevel; 6] = [
    ReasoningLevel::Minimal,
    ReasoningLevel::Low,
    ReasoningLevel::Medium,
    ReasoningLevel::High,
    ReasoningLevel::XHigh,
    ReasoningLevel::Max,
];

impl ReasoningLevelPolicy {
    /// Resolves one standard downstream level without converting the independent `none` value.
    pub fn resolve(
        self,
        requested: ReasoningLevel,
        executable: &[ReasoningLevel],
    ) -> Option<ReasoningLevel> {
        // Keep the explicit reasoning-disable value outside every positive normalization rule.
        if requested == ReasoningLevel::None {
            return executable
                .contains(&ReasoningLevel::None)
                .then_some(ReasoningLevel::None);
        }

        // Resolve strict membership or floor within the executable positive levels.
        match self {
            Self::Strict => executable.contains(&requested).then_some(requested),
            Self::ClampPositiveFloor => executable
                .iter()
                .copied()
                .filter(|level| *level != ReasoningLevel::None && *level <= requested)
                .max()
                .or_else(|| {
                    executable
                        .iter()
                        .copied()
                        .filter(|level| *level != ReasoningLevel::None)
                        .min()
                }),
        }
    }

    /// Returns the standard downstream levels accepted by this policy for one executable set.
    pub fn accepted_levels(self, executable: &[ReasoningLevel]) -> Vec<ReasoningLevel> {
        // Preserve exact executable levels for strict input contracts.
        if self == Self::Strict {
            return executable.to_vec();
        }

        // Publish every positive input only when at least one positive result can be resolved.
        let has_positive = executable
            .iter()
            .any(|level| *level != ReasoningLevel::None);
        let mut accepted = executable
            .contains(&ReasoningLevel::None)
            .then_some(ReasoningLevel::None)
            .into_iter()
            .collect::<Vec<_>>();
        if has_positive {
            accepted.extend(POSITIVE_REASONING_LEVELS);
        }
        accepted
    }
}

/// Ordered, duplicate-free reasoning levels confirmed for one canonical Model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReasoningLevels {
    values: Vec<ReasoningLevel>,
}

impl ReasoningLevels {
    /// Builds a stable set while preserving the first occurrence of each level.
    pub fn new(levels: impl IntoIterator<Item = ReasoningLevel>) -> Self {
        let mut values = Vec::new();
        for level in levels {
            if !values.contains(&level) {
                values.push(level);
            }
        }
        Self { values }
    }

    /// Returns the confirmed levels in their canonical catalog order.
    pub fn as_slice(&self) -> &[ReasoningLevel] {
        &self.values
    }

    /// Returns whether no configurable reasoning level is publicly confirmed.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns whether every level in this set is present in the supplied ceiling.
    fn is_subset_of(&self, upper: &Self) -> bool {
        self.values.iter().all(|level| upper.values.contains(level))
    }
}

/// Canonical reasoning evidence and the levels that may be selected downstream.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ReasoningProfile {
    /// The configuration lacks enough evidence to determine reasoning support.
    #[default]
    Unknown,
    /// The model explicitly does not support reasoning.
    Unsupported,
    /// The model supports reasoning with the listed confirmed control levels.
    Supported {
        /// Ordered, duplicate-free levels; an empty set means no selectable level is confirmed.
        levels: ReasoningLevels,
    },
}

impl ReasoningProfile {
    /// Builds a supported profile from an ordered set of confirmed levels.
    pub fn supported(levels: impl IntoIterator<Item = ReasoningLevel>) -> Self {
        Self::Supported {
            levels: ReasoningLevels::new(levels),
        }
    }

    /// Returns the public three-state support projection without duplicating stored state.
    pub const fn support(&self) -> ReasoningSupport {
        match self {
            Self::Unknown => ReasoningSupport::Unknown,
            Self::Unsupported => ReasoningSupport::Unsupported,
            Self::Supported { .. } => ReasoningSupport::Supported,
        }
    }

    /// Returns the confirmed selectable levels, or an empty slice when none are available.
    pub fn levels(&self) -> &[ReasoningLevel] {
        match self {
            Self::Supported { levels } => levels.as_slice(),
            Self::Unknown | Self::Unsupported => &[],
        }
    }

    /// Returns whether this profile is no broader than the supplied canonical ceiling.
    pub(crate) fn is_subset_of(&self, upper: &Self) -> bool {
        match (self, upper) {
            (Self::Unsupported, _) | (Self::Unknown, Self::Unknown | Self::Supported { .. }) => {
                true
            }
            (Self::Supported { levels }, Self::Supported { levels: upper }) => {
                levels.is_subset_of(upper)
            }
            (Self::Unknown | Self::Supported { .. }, Self::Unsupported)
            | (Self::Supported { .. }, Self::Unknown) => false,
        }
    }
}

impl ReasoningLevel {
    /// Parses a protocol wire string into a catalog enum.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// Returns the wire string used by the standard downstream protocol.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Standard non-admission generation parameters that OpenBridge may omit from upstream egress.
///
/// This closed set intentionally excludes streaming, reasoning effort or switches, tools,
/// structured output, state, media, and output-budget fields whose omission would bypass a
/// capability or resource boundary.
pub enum IgnorableGenerationParameter {
    /// Frequency penalty applied during token sampling.
    FrequencyPenalty,
    /// Presence penalty applied during token sampling.
    PresencePenalty,
    /// Deterministic sampling seed hint.
    Seed,
    /// Sampling temperature.
    Temperature,
    /// Nucleus-sampling probability threshold.
    TopP,
}

impl IgnorableGenerationParameter {
    /// Returns the standard top-level JSON field name for this parameter.
    pub const fn as_wire_name(self) -> &'static str {
        match self {
            Self::FrequencyPenalty => "frequency_penalty",
            Self::PresencePenalty => "presence_penalty",
            Self::Seed => "seed",
            Self::Temperature => "temperature",
            Self::TopP => "top_p",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Explicit mapping from a standard downstream reasoning level to an Upstream API wire level.
pub struct ReasoningLevelMapping {
    /// Standard downstream level declared by the Public Model.
    pub downstream: ReasoningLevel,
    /// Safe wire value accepted by the selected Upstream API.
    pub upstream: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
/// Independent model limits for total context, input, and output tokens.
pub struct ModelContextLength {
    /// Known combined input/output token limit; `None` means unknown.
    max_context_tokens: Option<u32>,
    /// Known maximum input token count; `None` means unknown. For OpenRouter-backed models this is
    /// populated from the model-level context length because the catalog does not publish a separate
    /// input-only limit.
    max_input_tokens: Option<u32>,
    /// Known maximum output token count; `None` means unknown.
    max_output_tokens: Option<u32>,
}

impl ModelContextLength {
    /// Creates independently optional total-context, input, and output limits.
    pub const fn new(
        max_context_tokens: Option<u32>,
        max_input_tokens: Option<u32>,
        max_output_tokens: Option<u32>,
    ) -> Self {
        Self {
            max_context_tokens,
            max_input_tokens,
            max_output_tokens,
        }
    }

    /// Returns the maximum combined input and output token count.
    pub const fn context_tokens(self) -> Option<u32> {
        self.max_context_tokens
    }

    /// Returns the maximum input token count.
    pub const fn input_tokens(self) -> Option<u32> {
        self.max_input_tokens
    }

    /// Returns the maximum output token count.
    pub const fn output_tokens(self) -> Option<u32> {
        self.max_output_tokens
    }
}

/// Input modalities accepted by a canonical Model.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    /// Text input。
    Text,
    /// Image input。
    Image,
    /// Audio input。
    Audio,
    /// File input。
    File,
    /// Video input.
    Video,
}

/// Output modalities a canonical Model can generate.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputModality {
    /// Text output。
    Text,
    /// Image output。
    Image,
    /// Audio output。
    Audio,
    /// Numeric embedding-vector output.
    Embedding,
}

/// Payload-free identity of one canonical Model task.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalTaskKind {
    /// General text or multimodal generation.
    Generation,
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

/// Typed identity of one model-bound Upstream API.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UpstreamApiKey {
    operation: OperationKind,
    task: CanonicalTaskKind,
}

impl UpstreamApiKey {
    /// Binds one closed operation to the canonical task selected by its Target.
    pub const fn new(operation: OperationKind, task: CanonicalTaskKind) -> Self {
        Self { operation, task }
    }

    /// Returns the callable operation.
    pub const fn operation(self) -> OperationKind {
        self.operation
    }

    /// Returns the selected canonical task.
    pub const fn task(self) -> CanonicalTaskKind {
        self.task
    }
}

/// Canonical facts owned only by a general generation task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationModelProfile {
    /// Combined input/output token limits confirmed for the model.
    pub context_length: ModelContextLength,
    /// Confirmed input modalities; `None` means unknown.
    pub input_modalities: Option<Vec<InputModality>>,
    /// Confirmed output modalities; `None` means unknown.
    pub output_modalities: Option<Vec<OutputModality>>,
    /// Ordinary model parameters, excluding protocol-specific reasoning aliases.
    pub supported_parameters: Vec<String>,
    /// Canonical model reasoning evidence and selectable levels.
    pub reasoning: ReasoningProfile,
}

/// Canonical facts owned only by an Embeddings task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingModelProfile {
    /// Maximum tokens accepted as embedding input; `None` means unknown.
    pub max_input_tokens: Option<u32>,
    /// Confirmed embedding input modalities; `None` means unknown.
    pub input_modalities: Option<Vec<InputModality>>,
    /// Embeddings request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Canonical facts owned only by an image-generation task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationModelProfile {
    /// Token limits published for the Images-native generation envelope.
    pub context_length: ModelContextLength,
    /// Images-specific request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Canonical facts owned only by a speech-recognition task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechRecognitionModelProfile {
    /// Token limits published for the Chat-native transcript envelope.
    pub context_length: ModelContextLength,
    /// ASR-specific request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Canonical facts owned only by an ordinary speech-synthesis task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechSynthesisModelProfile {
    /// Token limits published for the Chat-native synthesis envelope.
    pub context_length: ModelContextLength,
    /// TTS-specific request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Canonical facts owned only by a voice-design task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceDesignModelProfile {
    /// Token limits published for the Chat-native voice-design envelope.
    pub context_length: ModelContextLength,
    /// Voice-design-specific request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Canonical facts owned only by a reference-voice cloning task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceCloneModelProfile {
    /// Token limits published for the Chat-native voice-clone envelope.
    pub context_length: ModelContextLength,
    /// Voice-clone-specific request parameters declared by the model.
    pub supported_parameters: Vec<String>,
}

/// Closed canonical task union whose variant owns every task-specific model fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalModelTask {
    /// General text or multimodal generation facts.
    Generation(GenerationModelProfile),
    /// Embedding-vector generation facts.
    Embedding(EmbeddingModelProfile),
    /// Image-generation facts.
    ImageGeneration(ImageGenerationModelProfile),
    /// Speech-recognition facts.
    SpeechRecognition(SpeechRecognitionModelProfile),
    /// Ordinary speech-synthesis facts.
    SpeechSynthesis(SpeechSynthesisModelProfile),
    /// Voice-design synthesis facts.
    VoiceDesign(VoiceDesignModelProfile),
    /// Reference-voice cloning facts.
    VoiceClone(VoiceCloneModelProfile),
}

impl CanonicalModelTask {
    /// Returns the payload-free task identity used by registry compatibility gates.
    pub const fn kind(&self) -> CanonicalTaskKind {
        match self {
            Self::Generation(_) => CanonicalTaskKind::Generation,
            Self::Embedding(_) => CanonicalTaskKind::Embedding,
            Self::ImageGeneration(_) => CanonicalTaskKind::ImageGeneration,
            Self::SpeechRecognition(_) => CanonicalTaskKind::SpeechRecognition,
            Self::SpeechSynthesis(_) => CanonicalTaskKind::SpeechSynthesis,
            Self::VoiceDesign(_) => CanonicalTaskKind::VoiceDesign,
            Self::VoiceClone(_) => CanonicalTaskKind::VoiceClone,
        }
    }

    /// Returns the context or input-token limits owned by this task variant.
    pub const fn context_length(&self) -> ModelContextLength {
        match self {
            Self::Generation(profile) => profile.context_length,
            Self::Embedding(profile) => {
                ModelContextLength::new(profile.max_input_tokens, profile.max_input_tokens, None)
            }
            Self::ImageGeneration(profile) => profile.context_length,
            Self::SpeechRecognition(profile) => profile.context_length,
            Self::SpeechSynthesis(profile) => profile.context_length,
            Self::VoiceDesign(profile) => profile.context_length,
            Self::VoiceClone(profile) => profile.context_length,
        }
    }

    /// Returns confirmed input modalities, deriving fixed task semantics where applicable.
    pub fn input_modalities(&self) -> Option<&[InputModality]> {
        match self {
            Self::Generation(profile) => profile.input_modalities.as_deref(),
            Self::Embedding(profile) => profile.input_modalities.as_deref(),
            Self::ImageGeneration(_) => Some(&[InputModality::Text]),
            Self::SpeechRecognition(_) => Some(&[InputModality::Audio]),
            Self::SpeechSynthesis(_) | Self::VoiceDesign(_) => Some(&[InputModality::Text]),
            Self::VoiceClone(_) => Some(&[InputModality::Audio, InputModality::Text]),
        }
    }

    /// Returns confirmed output modalities, deriving fixed task semantics where applicable.
    pub fn output_modalities(&self) -> Option<&[OutputModality]> {
        match self {
            Self::Generation(profile) => profile.output_modalities.as_deref(),
            Self::Embedding(_) => Some(&[OutputModality::Embedding]),
            Self::ImageGeneration(_) => Some(&[OutputModality::Image]),
            Self::SpeechRecognition(_) => Some(&[OutputModality::Text]),
            Self::SpeechSynthesis(_) | Self::VoiceDesign(_) | Self::VoiceClone(_) => {
                Some(&[OutputModality::Audio])
            }
        }
    }

    /// Returns the ordinary or task-specific parameters owned by this variant.
    pub fn supported_parameters(&self) -> &[String] {
        match self {
            Self::Generation(profile) => &profile.supported_parameters,
            Self::Embedding(profile) => &profile.supported_parameters,
            Self::ImageGeneration(profile) => &profile.supported_parameters,
            Self::SpeechRecognition(profile) => &profile.supported_parameters,
            Self::SpeechSynthesis(profile) => &profile.supported_parameters,
            Self::VoiceDesign(profile) => &profile.supported_parameters,
            Self::VoiceClone(profile) => &profile.supported_parameters,
        }
    }

    /// Returns canonical reasoning support, deriving unsupported for non-generation tasks.
    pub const fn reasoning_support(&self) -> ReasoningSupport {
        match self {
            Self::Generation(profile) => profile.reasoning.support(),
            Self::Embedding(_)
            | Self::ImageGeneration(_)
            | Self::SpeechRecognition(_)
            | Self::SpeechSynthesis(_)
            | Self::VoiceDesign(_)
            | Self::VoiceClone(_) => ReasoningSupport::Unsupported,
        }
    }

    /// Returns confirmed reasoning levels for a generation task.
    pub fn reasoning_levels(&self) -> &[ReasoningLevel] {
        match self {
            Self::Generation(profile) => profile.reasoning.levels(),
            Self::Embedding(_)
            | Self::ImageGeneration(_)
            | Self::SpeechRecognition(_)
            | Self::SpeechSynthesis(_)
            | Self::VoiceDesign(_)
            | Self::VoiceClone(_) => &[],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Provider-independent canonical model facts.
pub struct ModelConfig {
    /// Stable model ID within the catalog.
    pub id: String,
    /// Model name shown to clients.
    pub name: String,
    /// Optional model description.
    pub description: Option<String>,
    /// Tokenizer identifier published by the model catalog; `None` means unknown.
    pub tokenizer: Option<String>,
    /// Knowledge-cutoff date published by the model catalog; `None` means unknown.
    pub knowledge_cutoff: Option<String>,
    /// Required task identity and every fact whose meaning depends on that task.
    pub task: CanonicalModelTask,
}

impl ModelConfig {
    /// Returns the payload-free canonical task identity.
    pub const fn task_kind(&self) -> CanonicalTaskKind {
        self.task.kind()
    }

    /// Returns the context or input-token limits owned by the task payload.
    pub const fn context_length(&self) -> ModelContextLength {
        self.task.context_length()
    }

    /// Returns confirmed input modalities derived from the task payload.
    pub fn input_modalities(&self) -> Option<&[InputModality]> {
        self.task.input_modalities()
    }

    /// Returns confirmed output modalities derived from the task payload.
    pub fn output_modalities(&self) -> Option<&[OutputModality]> {
        self.task.output_modalities()
    }

    /// Returns ordinary or task-specific parameters without reasoning protocol aliases.
    pub fn supported_parameters(&self) -> &[String] {
        self.task.supported_parameters()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// Model and ordinary-parameter rules applied by one Upstream API.
pub struct UpstreamApiModelRules {
    /// Context length the Upstream API may narrow further.
    pub context_length: ModelContextLength,
    /// Canonical reasoning profile the Upstream API may narrow further.
    pub reasoning: Option<ReasoningProfile>,
    /// Parameter names the Upstream API disables but cannot add.
    pub disabled_parameters: Vec<String>,
    /// Ordinary downstream parameters accepted by OpenBridge but omitted from upstream egress.
    pub ignored_parameters: Vec<IgnorableGenerationParameter>,
    /// Explicit mapping from standard downstream reasoning levels to this Upstream API's wire values.
    pub reasoning_level_mappings: Vec<ReasoningLevelMapping>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Credential-pool declaration shared by a Provider.
pub struct CredentialPoolConfig {
    /// Pool ID in the registry.
    pub id: String,
    /// Provider allowed to consume this pool.
    pub provider: ProviderKind,
    /// Credential type supported by the adapter.
    pub kind: CredentialKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One trusted deployment of a compile-time Provider family.
pub struct ProviderInstanceConfig {
    /// Stable Provider instance ID in the registry.
    pub id: String,
    /// Compile-time Provider family implemented by this instance.
    pub kind: ProviderKind,
    /// Sole trusted HTTPS base URL for this deployment.
    pub base_url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Upstream API capability configuration bound to one present, executable operation.
///
/// An unsupported operation is omitted from the Target API list instead of being represented by a
/// disabled capability payload.
pub enum UpstreamApiCapabilities {
    /// Chat Completions endpoint capabilities.
    ChatCompletions(ChatCompletionsCapabilities),
    /// Responses endpoint capabilities.
    Responses(ResponsesCapabilities),
    /// Embeddings Create operation capabilities.
    Embeddings(EmbeddingsCapabilities),
    /// Images Generations operation capabilities.
    ImagesGenerations(ImagesGenerationsCapabilities),
}

impl UpstreamApiCapabilities {
    /// Returns the native operation represented by this capability configuration.
    pub const fn operation(self) -> OperationKind {
        match self {
            Self::ChatCompletions(_) => OperationKind::ChatCompletions,
            Self::Responses(_) => OperationKind::Responses,
            Self::Embeddings(_) => OperationKind::EmbeddingsCreate,
            Self::ImagesGenerations(_) => OperationKind::ImagesGenerations,
        }
    }

    /// Returns the generation protocol when this capability can participate in the Protocol Bridge.
    pub const fn api_protocol(self) -> Option<ApiProtocol> {
        self.operation().api_protocol()
    }

    /// Returns common protocol capabilities without Responses-specific state.
    pub(crate) const fn generation_capabilities(self) -> Option<GenerationCapabilities> {
        match self {
            Self::ChatCompletions(capabilities) => Some(capabilities.generation_capabilities()),
            Self::Responses(capabilities) => Some(capabilities.generation_capabilities()),
            Self::Embeddings(_) | Self::ImagesGenerations(_) => None,
        }
    }

    /// Returns the complete capability set when this is an Embeddings configuration.
    pub const fn embeddings(self) -> Option<EmbeddingsCapabilities> {
        match self {
            Self::Embeddings(capabilities) => Some(capabilities),
            Self::ChatCompletions(_) | Self::Responses(_) | Self::ImagesGenerations(_) => None,
        }
    }

    /// Returns the complete capability set when this is a Responses configuration.
    pub const fn responses(self) -> Option<ResponsesCapabilities> {
        match self {
            Self::ChatCompletions(_) => None,
            Self::Responses(capabilities) => Some(capabilities),
            Self::Embeddings(_) | Self::ImagesGenerations(_) => None,
        }
    }

    /// Returns the complete capability set when this is an Images Generations configuration.
    pub const fn images_generations(self) -> Option<ImagesGenerationsCapabilities> {
        match self {
            Self::ImagesGenerations(capabilities) => Some(capabilities),
            Self::ChatCompletions(_) | Self::Responses(_) | Self::Embeddings(_) => None,
        }
    }

    /// Returns the reasoning output type declared by this Upstream API.
    pub const fn reasoning_output(self) -> crate::core::ReasoningOutput {
        match self {
            Self::ChatCompletions(capabilities) => capabilities.reasoning_output,
            Self::Responses(capabilities) => capabilities.reasoning_output,
            Self::Embeddings(_) | Self::ImagesGenerations(_) => {
                crate::core::ReasoningOutput::Unsupported
            }
        }
    }

    pub(super) fn is_subset_of(self, upper: ApiCapabilities) -> bool {
        match self {
            Self::ChatCompletions(capabilities) => upper
                .operation(OperationKind::ChatCompletions)
                .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
                .is_some_and(|upper| capabilities.is_subset_of(upper)),
            Self::Responses(capabilities) => upper
                .operation(OperationKind::Responses)
                .and_then(crate::core::ProviderOperationCapabilities::responses)
                .is_some_and(|upper| capabilities.is_subset_of(upper)),
            Self::Embeddings(capabilities) => upper
                .operation(OperationKind::EmbeddingsCreate)
                .and_then(crate::core::ProviderOperationCapabilities::embeddings)
                .is_some_and(|upper| capabilities.is_subset_of(upper)),
            Self::ImagesGenerations(capabilities) => upper
                .operation(OperationKind::ImagesGenerations)
                .and_then(crate::core::ProviderOperationCapabilities::images_generations)
                .is_some_and(|upper| capabilities.is_subset_of(upper)),
        }
    }
}

/// Conversion policy for a downstream non-streaming request when an Upstream API requires SSE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonStreamingConversion {
    /// Rejects non-streaming use of the streaming-only Upstream API before egress.
    Disabled,
    /// Buffers and validates a complete Responses SSE lifecycle before returning JSON downstream.
    BufferResponsesSse,
}

/// Declares whether an Upstream API accepts optional streaming or requires `stream: true`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpstreamStreamingPolicy {
    /// Preserves the downstream streaming mode because the Upstream API accepts both modes.
    Optional,
    /// Forces `stream: true` upstream and controls whether non-streaming delivery can be synthesized.
    Required {
        /// Trusted conversion policy for downstream requests that require one complete JSON body.
        non_streaming: NonStreamingConversion,
    },
}

impl UpstreamStreamingPolicy {
    /// Returns whether the Upstream API can satisfy a downstream non-streaming request.
    pub const fn supports_non_streaming(self) -> bool {
        matches!(
            self,
            Self::Optional
                | Self::Required {
                    non_streaming: NonStreamingConversion::BufferResponsesSse
                }
        )
    }

    /// Returns whether every upstream request must use streaming transport.
    pub const fn requires_streaming(self) -> bool {
        matches!(self, Self::Required { .. })
    }

    /// Returns whether a non-streaming request must buffer a typed Responses SSE lifecycle.
    pub const fn buffers_responses_sse(self) -> bool {
        matches!(
            self,
            Self::Required {
                non_streaming: NonStreamingConversion::BufferResponsesSse
            }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Native Upstream API exposed by a target.
pub struct UpstreamApiConfig {
    /// Typed operation/task identity selected for this concrete API.
    pub key: UpstreamApiKey,
    /// Actual model ID sent upstream.
    pub upstream_model: String,
    /// Upstream API-level narrowing rules for Model facts.
    pub model_rules: UpstreamApiModelRules,
    /// Single-protocol capability evidence.
    pub capabilities: UpstreamApiCapabilities,
    /// Upstream streaming requirement and optional downstream non-streaming conversion.
    pub streaming_policy: UpstreamStreamingPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Trusted upstream target eligible for Route selection.
pub struct UpstreamTargetConfig {
    /// Target ID in the registry.
    pub id: String,
    /// Referenced Provider instance ID.
    pub provider_instance: String,
    /// Canonical designer/model identity whose provider-independent facts are used by this target.
    pub canonical_model: String,
    /// Trusted provider/model identity used by the routing layer for this target.
    pub provider_model: String,
    /// Shared credential-pool ID referenced by the target.
    pub credential_pool: String,
    /// Optional explicit shared quota scope.
    pub quota_scope: Option<String>,
    /// Optional fault/cooldown domain.
    pub fault_domain: Option<String>,
    /// Timeout for one upstream request.
    pub request_timeout: Duration,
    /// Whether new stateless requests may select this target.
    pub enabled: bool,
    /// Protocol-level Native supplies provided by the target.
    pub upstream_apis: Vec<UpstreamApiConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Request handling mode for a Route.
pub enum RouteMode {
    /// Keeps downstream and upstream protocols natively identical.
    Native,
    /// Performs the declared restricted conversion between Generation protocols.
    GenerationBridge(GenerationBridgeDirection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Route binding a downstream protocol to an Upstream API.
pub struct RouteConfig {
    /// Route ID in the registry.
    pub id: String,
    /// Upstream Target ID referenced by the Route.
    pub upstream_target: String,
    /// Typed Upstream API operation referenced by the Route.
    pub upstream_operation: OperationKind,
    /// Downstream operation accepted by the Route.
    pub downstream_operation: OperationKind,
    /// Route handling mode.
    pub mode: RouteMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Model exposed downstream with ordered Route candidates.
pub struct PublicModelConfig {
    /// Stable model ID exposed downstream.
    pub id: String,
    /// Stable Unix seconds when the Public Model contract was first created.
    pub created: u64,
    /// Name shown to clients.
    pub display_name: String,
    /// Optional description shown to clients.
    pub description: Option<String>,
    /// Static Public Model lifecycle.
    pub lifecycle: ModelLifecycle,
    /// Static policy for resolving standard downstream reasoning levels.
    pub reasoning_level_policy: ReasoningLevelPolicy,
    /// Complete Route IDs ordered by priority.
    pub routes: Vec<String>,
}

/// Public Model lifecycle status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLifecycleStatus {
    /// The model accepts new requests.
    Active,
    /// The model remains callable, but callers should migrate.
    Deprecated,
    /// The model no longer accepts requests.
    Retired,
}

/// Static Public Model lifecycle information.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelLifecycle {
    /// Current lifecycle status.
    pub status: ModelLifecycleStatus,
    /// Optional Unix seconds when deprecation began.
    pub deprecated_at: Option<u64>,
    /// Optional Unix seconds when retirement began.
    pub retired_at: Option<u64>,
}

impl ModelLifecycle {
    /// Creates an active lifecycle with no deprecation or retirement time.
    pub const fn active() -> Self {
        Self {
            status: ModelLifecycleStatus::Active,
            deprecated_at: None,
            retired_at: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Complete definition required to compile the registry at startup.
pub struct RegistryConfig {
    /// Registry version used for reporting and audit.
    pub version: String,
    /// Complete model definitions.
    pub models: Vec<ModelConfig>,
    /// Complete trusted Provider instance definitions.
    pub provider_instances: Vec<ProviderInstanceConfig>,
    /// Complete credential-pool definitions.
    pub credential_pools: Vec<CredentialPoolConfig>,
    /// Complete Upstream Target definitions.
    pub upstream_targets: Vec<UpstreamTargetConfig>,
    /// Complete Route definitions.
    pub routes: Vec<RouteConfig>,
    /// Complete Public Model definitions.
    pub public_models: Vec<PublicModelConfig>,
}
