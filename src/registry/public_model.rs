//! Fixed downstream Public Model contracts and their private execution snapshot.
//!
//! Client-visible DTOs remain in this facade. Private submodules own execution candidates and
//! startup compilation so serialized responses cannot acquire Provider, Target, Route,
//! upstream-model, or credential topology by accident.

use serde::{Serialize, Serializer};

use crate::core::{
    AsrLanguage, AudioFormat, AudioInputCapabilities, AudioInputSource, EmbeddingDimensionDomain,
    EmbeddingEncoding, EmbeddingInputForm, ExecutableAudioProfile, GeneratedAudioCapabilities,
    ImageDetail, ImageDetailPolicy, ImageInputCapabilities, ImageInputSource, ImageMediaType,
    ImageSourceCapabilities, InlineImageInputProfile, JsonAudioFraming, ReasoningOutput,
    ResponseInclude, SseAudioFraming, StructuredOutputProfile,
};

pub use crate::core::{StructuredOutputMode, ToolChoiceMode};

use super::{
    CanonicalTaskKind, InputModality, ModelContextLength, ModelLifecycle, OutputModality,
    ReasoningLevel, ReasoningLevelPolicy, ReasoningSupport,
};

mod compiler;
mod execution;

pub(super) use compiler::{PublicRouteBinding, compile_public_model};
pub(crate) use execution::ModelExecutionInterface;
pub use execution::PublicModel;

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

/// Owned image-input contract compiled for one Native interface.
///
/// The private source and detail unions are the execution contract. Serialization projects this
/// contract into the established flat Models extension without turning zero values into state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageInputInterfaceCapabilities {
    max_parts: u32,
    sources: OwnedImageSourceCapabilities,
    detail: OwnedImageDetailPolicy,
}

/// Source-specific payload owned by one compiled Public Model interface.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedImageSourceCapabilities {
    /// Remote URLs with their applicable URL byte limit.
    Remote(OwnedRemoteImageInputLimits),
    /// Inline data URLs with media types and inline byte limits.
    Inline(OwnedInlineImageInputProfile),
    /// Both independently complete source payloads.
    RemoteAndInline {
        remote: OwnedRemoteImageInputLimits,
        data: OwnedInlineImageInputProfile,
    },
}

/// Owned remote-URL limits after conservative Public Model aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedRemoteImageInputLimits {
    max_url_length: u32,
}

/// Owned inline data-URL profile after media-type intersection.
#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedInlineImageInputProfile {
    media_types: Vec<ImageMediaType>,
    limits: OwnedInlineImageInputLimits,
}

/// Owned inline byte limits after cross-field reachability narrowing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedInlineImageInputLimits {
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

/// Closed omitted-versus-explicit detail policy owned by request preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedImageDetailPolicy {
    /// Clients may omit `detail`, but cannot submit an explicit value.
    OmittedOnly { default: Option<ImageDetail> },
    /// Clients may omit `detail` and may submit one value from the explicit set.
    Explicit {
        default: Option<ImageDetail>,
        allowed: Vec<ImageDetail>,
    },
}

/// Borrowed-free wire projection preserving the established Models JSON layout.
#[derive(Serialize)]
struct ImageInputInterfaceCapabilitiesWire {
    sources: Vec<ImageInputSource>,
    media_types: Vec<ImageMediaType>,
    detail: ImageDetailCapabilities,
    limits: ImageInputLimits,
}

/// Public source and size limits for one typed audio input profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioInputLimits {
    max_parts: u32,
    max_url_length: u32,
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

/// Typed audio source and format contract for one Native Chat interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioInputInterfaceCapabilities {
    sources: Vec<AudioInputSource>,
    formats: Vec<AudioFormat>,
    limits: AudioInputLimits,
}

impl AudioInputInterfaceCapabilities {
    /// Converts one static audio input profile into a downstream-safe owned contract.
    fn from_capabilities(value: AudioInputCapabilities) -> Self {
        Self {
            sources: value.sources().to_vec(),
            formats: value.formats().to_vec(),
            limits: AudioInputLimits {
                max_parts: value.max_parts(),
                max_url_length: value.max_url_length(),
                max_inline_encoded_bytes: value.max_inline_encoded_bytes(),
                max_inline_decoded_bytes: value.max_inline_decoded_bytes(),
                max_total_inline_encoded_bytes: value.max_total_inline_encoded_bytes(),
                max_total_inline_decoded_bytes: value.max_total_inline_decoded_bytes(),
            },
        }
    }

    /// Intersects all candidate profiles without exposing Route or Provider identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = values.first()?.to_owned().clone();
        let mut result = first;
        result
            .sources
            .retain(|source| values.iter().all(|value| value.sources.contains(source)));
        result
            .formats
            .retain(|format| values.iter().all(|value| value.formats.contains(format)));
        result.limits.max_parts = values.iter().map(|value| value.limits.max_parts).min()?;
        result.limits.max_url_length = values
            .iter()
            .map(|value| value.limits.max_url_length)
            .min()?;
        result.limits.max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.limits.max_inline_encoded_bytes)
            .min()?;
        result.limits.max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.limits.max_inline_decoded_bytes)
            .min()?;
        result.limits.max_total_inline_encoded_bytes = values
            .iter()
            .map(|value| value.limits.max_total_inline_encoded_bytes)
            .min()?;
        result.limits.max_total_inline_decoded_bytes = values
            .iter()
            .map(|value| value.limits.max_total_inline_decoded_bytes)
            .min()?;
        (!result.sources.is_empty() && !result.formats.is_empty()).then_some(result)
    }

    /// Returns whether this interface accepts one audio source.
    pub(crate) fn supports_source(&self, source: AudioInputSource) -> bool {
        self.sources.contains(&source)
    }

    /// Returns whether this interface accepts one audio format.
    pub(crate) fn supports_format(&self, format: AudioFormat) -> bool {
        self.formats.contains(&format)
    }

    /// Returns the maximum number of audio parts accepted by one request.
    pub(crate) const fn max_parts(&self) -> u32 {
        self.limits.max_parts
    }

    /// Returns the maximum URL length accepted by this interface.
    pub(crate) const fn max_url_length(&self) -> u32 {
        self.limits.max_url_length
    }

    /// Returns the maximum encoded inline input size.
    pub(crate) const fn max_inline_encoded_bytes(&self) -> u32 {
        self.limits.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded inline input size.
    pub(crate) const fn max_inline_decoded_bytes(&self) -> u32 {
        self.limits.max_inline_decoded_bytes
    }

    /// Returns the cumulative encoded inline input limit.
    pub(crate) const fn max_total_inline_encoded_bytes(&self) -> u32 {
        self.limits.max_total_inline_encoded_bytes
    }

    /// Returns the cumulative decoded inline input limit.
    pub(crate) const fn max_total_inline_decoded_bytes(&self) -> u32 {
        self.limits.max_total_inline_decoded_bytes
    }
}

/// Public output formats, voices, and response budgets for one TTS-like interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioOutputInterfaceCapabilities {
    formats: Vec<AudioFormat>,
    streaming_formats: Vec<AudioFormat>,
    voices: Vec<String>,
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_stream_decoded_bytes: u32,
    #[serde(skip)]
    json_framing: JsonAudioFraming,
    #[serde(skip)]
    sse_framing: SseAudioFraming,
}

impl AudioOutputInterfaceCapabilities {
    /// Converts one complete generated-audio profile into a downstream-safe owned contract.
    fn from_capabilities(value: GeneratedAudioCapabilities, voices: &[&str]) -> Self {
        let json = value.json();
        let sse = value.sse();
        Self {
            formats: json.formats().to_vec(),
            streaming_formats: sse.formats().to_vec(),
            voices: voices.iter().map(|voice| (*voice).to_owned()).collect(),
            max_inline_encoded_bytes: json.max_inline_encoded_bytes(),
            max_inline_decoded_bytes: json.max_inline_decoded_bytes(),
            max_stream_decoded_bytes: sse.max_stream_decoded_bytes(),
            json_framing: json.framing(),
            sse_framing: sse.framing(),
        }
    }

    /// Intersects all candidate profiles without exposing Route or Provider identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = values.first()?.to_owned().clone();
        let mut result = first;
        if values.iter().any(|value| {
            value.json_framing != result.json_framing || value.sse_framing != result.sse_framing
        }) {
            return None;
        }
        result
            .formats
            .retain(|format| values.iter().all(|value| value.formats.contains(format)));
        result.streaming_formats.retain(|format| {
            values
                .iter()
                .all(|value| value.streaming_formats.contains(format))
        });
        result
            .voices
            .retain(|voice| values.iter().all(|value| value.voices.contains(voice)));
        result.max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_inline_encoded_bytes)
            .min()?;
        result.max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_inline_decoded_bytes)
            .min()?;
        result.max_stream_decoded_bytes = values
            .iter()
            .map(|value| value.max_stream_decoded_bytes)
            .min()?;
        (!result.formats.is_empty() && !result.streaming_formats.is_empty()).then_some(result)
    }

    /// Returns whether this interface accepts one output format for the selected streaming mode.
    pub(crate) fn supports_format(&self, format: AudioFormat, streaming: bool) -> bool {
        let formats = if streaming {
            &self.streaming_formats
        } else {
            &self.formats
        };
        formats.contains(&format)
    }

    /// Returns whether this interface accepts one preset voice name.
    pub(crate) fn supports_voice(&self, voice: &str) -> bool {
        self.voices.iter().any(|candidate| candidate == voice)
    }

    /// Returns the non-streaming encoded audio response limit.
    pub(crate) const fn max_inline_encoded_bytes(&self) -> u32 {
        self.max_inline_encoded_bytes
    }

    /// Returns the non-streaming decoded audio response limit.
    pub(crate) const fn max_inline_decoded_bytes(&self) -> u32 {
        self.max_inline_decoded_bytes
    }

    /// Returns the cumulative decoded audio streaming limit.
    pub(crate) const fn max_stream_decoded_bytes(&self) -> u32 {
        self.max_stream_decoded_bytes
    }
}

/// Closed executable audio contract retained by one compiled Public Model interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioInterfaceCapabilities {
    /// General audio content is understood and answered with text.
    AudioUnderstanding {
        /// Accepted business-audio input profile.
        input: AudioInputInterfaceCapabilities,
    },
    /// One audio input is transcribed with an optional supported language selection.
    SpeechRecognition {
        /// Accepted speech input profile.
        input: AudioInputInterfaceCapabilities,
        /// Accepted ASR language selections.
        languages: Vec<AsrLanguage>,
    },
    /// Text is synthesized with one of the advertised preset voices.
    SpeechSynthesis {
        /// Generated-audio delivery and preset-voice profile.
        output: AudioOutputInterfaceCapabilities,
    },
    /// Text is synthesized with a voice described by a user message.
    VoiceDesign {
        /// Generated-audio delivery profile.
        output: AudioOutputInterfaceCapabilities,
    },
    /// Text is synthesized with a voice conditioned by reference audio.
    VoiceClone {
        /// Accepted reference-voice input profile.
        conditioning: AudioInputInterfaceCapabilities,
        /// Generated-audio delivery profile.
        output: AudioOutputInterfaceCapabilities,
    },
}

impl AudioInterfaceCapabilities {
    /// Converts one concrete Target profile into its owned request-time contract.
    fn from_capabilities(value: ExecutableAudioProfile) -> Self {
        match value {
            ExecutableAudioProfile::AudioUnderstanding(profile) => Self::AudioUnderstanding {
                input: AudioInputInterfaceCapabilities::from_capabilities(profile.input()),
            },
            ExecutableAudioProfile::SpeechRecognition(profile) => Self::SpeechRecognition {
                input: AudioInputInterfaceCapabilities::from_capabilities(profile.input()),
                languages: profile.languages().to_vec(),
            },
            ExecutableAudioProfile::SpeechSynthesis(profile) => Self::SpeechSynthesis {
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    profile.preset_voices().values(),
                ),
            },
            ExecutableAudioProfile::VoiceDesign(profile) => Self::VoiceDesign {
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    &[],
                ),
            },
            ExecutableAudioProfile::VoiceClone(profile) => Self::VoiceClone {
                conditioning: AudioInputInterfaceCapabilities::from_capabilities(
                    profile.voice_conditioning(),
                ),
                output: AudioOutputInterfaceCapabilities::from_capabilities(
                    profile.generated_audio(),
                    &[],
                ),
            },
        }
    }

    /// Intersects same-variant candidate payloads and rejects an empty executable profile.
    fn intersection<'a>(
        values: impl Iterator<Item = Option<&'a Self>>,
    ) -> Result<Option<Self>, ()> {
        // Preserve optional-capability narrowing when any executable candidate omits audio.
        let values = values.collect::<Vec<_>>();
        if values.is_empty() || values.iter().any(|value| value.is_none()) {
            return Ok(None);
        }

        // Fold pairwise intersections so variant mismatch and empty payload sets share one typed boundary.
        let mut values = values.into_iter().flatten();
        let first = values.next().ok_or(())?.clone();
        values
            .try_fold(first, |current, value| current.intersect(value).ok_or(()))
            .map(Some)
    }

    /// Intersects two complete profiles only when their variants and required payloads agree.
    fn intersect(self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                Self::AudioUnderstanding { input: left },
                Self::AudioUnderstanding { input: right },
            ) => AudioInputInterfaceCapabilities::intersection(
                [Some(&left), Some(right)].into_iter(),
            )
            .map(|input| Self::AudioUnderstanding { input }),
            (
                Self::SpeechRecognition {
                    input: left,
                    mut languages,
                },
                Self::SpeechRecognition {
                    input: right,
                    languages: other_languages,
                },
            ) => {
                let input = AudioInputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )?;
                languages.retain(|language| other_languages.contains(language));
                (!languages.is_empty()).then_some(Self::SpeechRecognition { input, languages })
            }
            (Self::SpeechSynthesis { output: left }, Self::SpeechSynthesis { output: right }) => {
                let output = AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )?;
                (!output.voices.is_empty()).then_some(Self::SpeechSynthesis { output })
            }
            (Self::VoiceDesign { output: left }, Self::VoiceDesign { output: right }) => {
                AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left), Some(right)].into_iter(),
                )
                .map(|output| Self::VoiceDesign { output })
            }
            (
                Self::VoiceClone {
                    conditioning: left_conditioning,
                    output: left_output,
                },
                Self::VoiceClone {
                    conditioning: right_conditioning,
                    output: right_output,
                },
            ) => {
                let conditioning = AudioInputInterfaceCapabilities::intersection(
                    [Some(&left_conditioning), Some(right_conditioning)].into_iter(),
                )?;
                let output = AudioOutputInterfaceCapabilities::intersection(
                    [Some(&left_output), Some(right_output)].into_iter(),
                )?;
                Some(Self::VoiceClone {
                    conditioning,
                    output,
                })
            }
            _ => None,
        }
    }

    /// Returns whether this profile contributes an audio input modality.
    const fn has_input(&self) -> bool {
        matches!(
            self,
            Self::AudioUnderstanding { .. }
                | Self::SpeechRecognition { .. }
                | Self::VoiceClone { .. }
        )
    }

    /// Returns whether this profile contributes an audio output modality.
    const fn has_output(&self) -> bool {
        matches!(
            self,
            Self::SpeechSynthesis { .. } | Self::VoiceDesign { .. } | Self::VoiceClone { .. }
        )
    }

    /// Returns the stable Models extension task projection for this executable profile.
    const fn task_projection(&self) -> AudioTaskProjection {
        match self {
            Self::AudioUnderstanding { .. } => AudioTaskProjection::AudioUnderstanding,
            Self::SpeechRecognition { .. } => AudioTaskProjection::Asr,
            Self::SpeechSynthesis { .. } => AudioTaskProjection::Tts,
            Self::VoiceDesign { .. } => AudioTaskProjection::VoiceDesign,
            Self::VoiceClone { .. } => AudioTaskProjection::VoiceClone,
        }
    }

    /// Projects this union into the existing downstream multimodal input object.
    fn multimodal_input(
        &self,
    ) -> (
        Option<AudioInputInterfaceCapabilities>,
        Option<AudioInputInterfaceCapabilities>,
    ) {
        match self {
            Self::AudioUnderstanding { input } | Self::SpeechRecognition { input, .. } => {
                (Some(input.clone()), None)
            }
            Self::VoiceClone { conditioning, .. } => (None, Some(conditioning.clone())),
            Self::SpeechSynthesis { .. } | Self::VoiceDesign { .. } => (None, None),
        }
    }

    /// Projects this union into the existing downstream multimodal output object.
    fn multimodal_output(&self) -> Option<AudioOutputInterfaceCapabilities> {
        match self {
            Self::SpeechSynthesis { output }
            | Self::VoiceDesign { output }
            | Self::VoiceClone { output, .. } => Some(output.clone()),
            Self::AudioUnderstanding { .. } | Self::SpeechRecognition { .. } => None,
        }
    }
}

/// Existing downstream audio task labels derived from the private executable profile union.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum AudioTaskProjection {
    AudioUnderstanding,
    Asr,
    Tts,
    VoiceDesign,
    VoiceClone,
}

impl ImageInputInterfaceCapabilities {
    /// Converts one checked Upstream API image profile into an owned execution contract.
    fn from_capabilities(value: ImageInputCapabilities) -> Self {
        // Copy the static source and detail payloads into the Public Model-owned contract.
        let sources =
            OwnedImageSourceCapabilities::from_capabilities(value.sources(), value.max_parts())
                .expect("checked core image source profile must remain valid");
        let detail = OwnedImageDetailPolicy::from_capabilities(value.detail_policy())
            .expect("checked core image detail policy must remain valid");

        // Revalidate the complete envelope at the ownership boundary.
        Self::new(value.max_parts(), sources, detail)
            .expect("checked core image profile must remain valid")
    }

    /// Intersects every complete Route profile without retaining Provider or Route identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        // Require every executable candidate to contribute a typed image profile.
        let values = values.collect::<Option<Vec<_>>>()?;
        let max_parts = values.iter().map(|value| value.max_parts).min()?;

        // Intersect each source only across candidates that all carry that same source payload.
        let remote = values
            .iter()
            .map(|value| value.sources.remote())
            .collect::<Option<Vec<_>>>()
            .and_then(|values| OwnedRemoteImageInputLimits::intersection(&values));
        let data = values
            .iter()
            .map(|value| value.sources.data())
            .collect::<Option<Vec<_>>>()
            .and_then(|values| OwnedInlineImageInputProfile::intersection(&values, max_parts));
        let sources = match (remote, data) {
            (Some(remote), Some(data)) => {
                OwnedImageSourceCapabilities::RemoteAndInline { remote, data }
            }
            (Some(remote), None) => OwnedImageSourceCapabilities::Remote(remote),
            (None, Some(data)) => OwnedImageSourceCapabilities::Inline(data),
            (None, None) => return None,
        };

        // Apply the full omitted-versus-explicit detail matrix before rebuilding the envelope.
        let detail =
            OwnedImageDetailPolicy::intersection(values.iter().map(|value| &value.detail))?;
        Self::new(max_parts, sources, detail)
    }

    /// Creates a checked owned image envelope.
    fn new(
        max_parts: u32,
        sources: OwnedImageSourceCapabilities,
        detail: OwnedImageDetailPolicy,
    ) -> Option<Self> {
        // Reject an unreachable source payload or invalid detail domain at the final ownership boundary.
        if max_parts == 0 || !sources.is_valid(max_parts) || !detail.is_valid() {
            return None;
        }

        // Store only the closed source and detail unions used by request preflight.
        Some(Self {
            max_parts,
            sources,
            detail,
        })
    }

    /// Projects the owned execution contract into the established flat Models JSON shape.
    fn wire_projection(&self) -> ImageInputInterfaceCapabilitiesWire {
        // Derive source tags and source-specific payloads without persisting flat zero sentinels.
        let sources = match self.sources {
            OwnedImageSourceCapabilities::Remote(_) => vec![ImageInputSource::RemoteUrl],
            OwnedImageSourceCapabilities::Inline(_) => vec![ImageInputSource::DataUrl],
            OwnedImageSourceCapabilities::RemoteAndInline { .. } => {
                vec![ImageInputSource::RemoteUrl, ImageInputSource::DataUrl]
            }
        };
        let remote = self.sources.remote();
        let data = self.sources.data();

        // Flatten absent source payloads to wire-only zero values for the existing DTO schema.
        ImageInputInterfaceCapabilitiesWire {
            sources,
            media_types: data.map_or_else(Vec::new, |profile| profile.media_types.clone()),
            detail: self.detail.wire_projection(),
            limits: ImageInputLimits {
                max_parts: self.max_parts,
                max_url_length: remote.map_or(0, |limits| limits.max_url_length),
                max_inline_encoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_inline_encoded_bytes),
                max_inline_decoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_inline_decoded_bytes),
                max_total_inline_encoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_total_inline_encoded_bytes),
                max_total_inline_decoded_bytes: data
                    .map_or(0, |profile| profile.limits.max_total_inline_decoded_bytes),
            },
        }
    }

    /// Returns whether this interface accepts one source kind.
    pub(crate) fn supports_source(&self, source: ImageInputSource) -> bool {
        match source {
            ImageInputSource::RemoteUrl => self.sources.remote().is_some(),
            ImageInputSource::DataUrl => self.sources.data().is_some(),
            ImageInputSource::FileId => false,
        }
    }

    /// Returns whether this interface accepts one inline media type.
    pub(crate) fn supports_media_type(&self, media_type: ImageMediaType) -> bool {
        self.sources
            .data()
            .is_some_and(|profile| profile.media_types.contains(&media_type))
    }

    /// Returns whether this interface accepts one explicit detail value.
    pub(crate) fn supports_detail(&self, detail: ImageDetail) -> bool {
        self.detail.supports_explicit(detail)
    }

    /// Returns the per-request image part limit.
    pub(crate) const fn max_parts(&self) -> u32 {
        self.max_parts
    }

    /// Returns the remote URL limit only when the source union accepts remote URLs.
    pub(crate) fn max_url_length(&self) -> Option<u32> {
        self.sources.remote().map(|limits| limits.max_url_length)
    }

    /// Returns the per-item encoded limit only when the source union accepts data URLs.
    pub(crate) fn max_inline_encoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_inline_encoded_bytes)
    }

    /// Returns the per-item decoded limit only when the source union accepts data URLs.
    pub(crate) fn max_inline_decoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_inline_decoded_bytes)
    }

    /// Returns the cumulative encoded limit only when the source union accepts data URLs.
    pub(crate) fn max_total_inline_encoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_total_inline_encoded_bytes)
    }

    /// Returns the cumulative decoded limit only when the source union accepts data URLs.
    pub(crate) fn max_total_inline_decoded_bytes(&self) -> Option<u32> {
        self.sources
            .data()
            .map(|profile| profile.limits.max_total_inline_decoded_bytes)
    }
}

impl OwnedImageSourceCapabilities {
    /// Copies one checked core source union into its owned Public Model representation.
    fn from_capabilities(value: ImageSourceCapabilities, max_parts: u32) -> Option<Self> {
        match value {
            ImageSourceCapabilities::RemoteUrl(remote) => Some(Self::Remote(
                OwnedRemoteImageInputLimits::new(remote.max_url_length())?,
            )),
            ImageSourceCapabilities::DataUrl(data) => Some(Self::Inline(
                OwnedInlineImageInputProfile::from_capabilities(data, max_parts)?,
            )),
            ImageSourceCapabilities::RemoteUrlAndDataUrl { remote, data } => {
                Some(Self::RemoteAndInline {
                    remote: OwnedRemoteImageInputLimits::new(remote.max_url_length())?,
                    data: OwnedInlineImageInputProfile::from_capabilities(data, max_parts)?,
                })
            }
        }
    }

    /// Returns the remote payload when the union accepts remote URLs.
    const fn remote(&self) -> Option<&OwnedRemoteImageInputLimits> {
        match self {
            Self::Remote(remote) | Self::RemoteAndInline { remote, .. } => Some(remote),
            Self::Inline(_) => None,
        }
    }

    /// Returns the inline payload when the union accepts data URLs.
    const fn data(&self) -> Option<&OwnedInlineImageInputProfile> {
        match self {
            Self::Inline(data) | Self::RemoteAndInline { data, .. } => Some(data),
            Self::Remote(_) => None,
        }
    }

    /// Returns whether every retained source payload is complete and reachable.
    fn is_valid(&self, max_parts: u32) -> bool {
        match self {
            Self::Remote(remote) => remote.is_valid(),
            Self::Inline(data) => data.is_valid(max_parts),
            Self::RemoteAndInline { remote, data } => remote.is_valid() && data.is_valid(max_parts),
        }
    }
}

impl OwnedRemoteImageInputLimits {
    /// Creates a positive remote-URL limit.
    const fn new(max_url_length: u32) -> Option<Self> {
        if max_url_length == 0 {
            return None;
        }
        Some(Self { max_url_length })
    }

    /// Intersects complete remote payloads by their conservative URL limit.
    fn intersection(values: &[&Self]) -> Option<Self> {
        Self::new(values.iter().map(|value| value.max_url_length).min()?)
    }

    /// Returns whether this remote payload remains valid.
    const fn is_valid(self) -> bool {
        self.max_url_length > 0
    }
}

impl OwnedInlineImageInputProfile {
    /// Copies one checked static inline profile into owned storage.
    fn from_capabilities(value: InlineImageInputProfile, max_parts: u32) -> Option<Self> {
        let limits = value.limits();
        Self::new(
            value.media_types().to_vec(),
            OwnedInlineImageInputLimits::new(
                limits.max_inline_encoded_bytes(),
                limits.max_inline_decoded_bytes(),
                limits.max_total_inline_encoded_bytes(),
                limits.max_total_inline_decoded_bytes(),
                max_parts,
            )?,
            max_parts,
        )
    }

    /// Intersects inline media types and clamps cross-candidate totals to reachable capacities.
    fn intersection(values: &[&Self], max_parts: u32) -> Option<Self> {
        // Retain only media types supported by every candidate's data-URL payload.
        let mut media_types = values.first()?.media_types.clone();
        media_types.retain(|media_type| {
            values
                .iter()
                .all(|value| value.media_types.contains(media_type))
        });
        if media_types.is_empty() {
            return None;
        }

        // Clamp independently minimized totals and revalidate them with the narrowed part count.
        let limits = OwnedInlineImageInputLimits::intersection(
            &values.iter().map(|value| &value.limits).collect::<Vec<_>>(),
            max_parts,
        )?;
        Self::new(media_types, limits, max_parts)
    }

    /// Creates a checked owned inline profile.
    fn new(
        media_types: Vec<ImageMediaType>,
        limits: OwnedInlineImageInputLimits,
        max_parts: u32,
    ) -> Option<Self> {
        if !is_nonempty_unique(&media_types) || !limits.is_valid(max_parts) {
            return None;
        }
        Some(Self {
            media_types,
            limits,
        })
    }

    /// Returns whether this inline payload remains complete and reachable.
    fn is_valid(&self, max_parts: u32) -> bool {
        is_nonempty_unique(&self.media_types) && self.limits.is_valid(max_parts)
    }
}

impl OwnedInlineImageInputLimits {
    /// Creates positive, internally coherent, and reachable inline limits.
    fn new(
        max_inline_encoded_bytes: u32,
        max_inline_decoded_bytes: u32,
        max_total_inline_encoded_bytes: u32,
        max_total_inline_decoded_bytes: u32,
        max_parts: u32,
    ) -> Option<Self> {
        // Calculate both reachable totals with checked wide arithmetic.
        let reachable_encoded =
            u64::from(max_inline_encoded_bytes).checked_mul(u64::from(max_parts))?;
        let reachable_decoded =
            u64::from(max_inline_decoded_bytes).checked_mul(u64::from(max_parts))?;

        // Reject zero, one-item-incoherent, or unreachable limit combinations.
        if max_inline_encoded_bytes == 0
            || max_inline_decoded_bytes == 0
            || max_total_inline_encoded_bytes < max_inline_encoded_bytes
            || max_total_inline_decoded_bytes < max_inline_decoded_bytes
            || u64::from(max_total_inline_encoded_bytes) > reachable_encoded
            || u64::from(max_total_inline_decoded_bytes) > reachable_decoded
        {
            return None;
        }
        Some(Self {
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
        })
    }

    /// Intersects every numeric dimension and clamps totals to the narrowed reachable capacity.
    fn intersection(values: &[&Self], max_parts: u32) -> Option<Self> {
        // Compute the raw conservative minima for all four inline budget dimensions.
        let max_inline_encoded_bytes = values
            .iter()
            .map(|value| value.max_inline_encoded_bytes)
            .min()?;
        let max_inline_decoded_bytes = values
            .iter()
            .map(|value| value.max_inline_decoded_bytes)
            .min()?;
        let raw_total_encoded = values
            .iter()
            .map(|value| value.max_total_inline_encoded_bytes)
            .min()?;
        let raw_total_decoded = values
            .iter()
            .map(|value| value.max_total_inline_decoded_bytes)
            .min()?;

        // Narrow each total to what the independently minimized per-item and part limits can reach.
        let reachable_encoded =
            u64::from(max_inline_encoded_bytes).checked_mul(u64::from(max_parts))?;
        let reachable_decoded =
            u64::from(max_inline_decoded_bytes).checked_mul(u64::from(max_parts))?;
        let max_total_inline_encoded_bytes =
            u32::try_from(u64::from(raw_total_encoded).min(reachable_encoded)).ok()?;
        let max_total_inline_decoded_bytes =
            u32::try_from(u64::from(raw_total_decoded).min(reachable_decoded)).ok()?;

        // Rebuild through the same checked constructor used by static profile conversion.
        Self::new(
            max_inline_encoded_bytes,
            max_inline_decoded_bytes,
            max_total_inline_encoded_bytes,
            max_total_inline_decoded_bytes,
            max_parts,
        )
    }

    /// Returns whether all inline limits remain positive, coherent, and reachable.
    fn is_valid(self, max_parts: u32) -> bool {
        Self::new(
            self.max_inline_encoded_bytes,
            self.max_inline_decoded_bytes,
            self.max_total_inline_encoded_bytes,
            self.max_total_inline_decoded_bytes,
            max_parts,
        )
        .is_some()
    }
}

impl OwnedImageDetailPolicy {
    /// Copies one checked core detail policy into its owned Public Model representation.
    fn from_capabilities(value: ImageDetailPolicy) -> Option<Self> {
        match value {
            ImageDetailPolicy::OmittedOnly { default } => Some(Self::OmittedOnly { default }),
            ImageDetailPolicy::Explicit(profile) => {
                Self::explicit(profile.default(), profile.allowed().to_vec())
            }
        }
    }

    /// Intersects omitted behavior and explicit domains according to the complete policy matrix.
    fn intersection<'a>(values: impl Iterator<Item = &'a Self>) -> Option<Self> {
        // Require one common omission default before considering explicit request values.
        let values = values.collect::<Vec<_>>();
        let default = values.first()?.default();
        if values.iter().any(|value| value.default() != default) {
            return None;
        }

        // Any omitted-only candidate narrows the aggregate to omission with the common default.
        if values
            .iter()
            .any(|value| matches!(value, Self::OmittedOnly { .. }))
        {
            return Some(Self::OmittedOnly { default });
        }

        // Intersect every explicit domain; an empty domain safely downgrades to omission-only.
        let mut allowed = values.first()?.allowed().to_vec();
        allowed.retain(|detail| values.iter().all(|value| value.allowed().contains(detail)));
        if allowed.is_empty() {
            Some(Self::OmittedOnly { default })
        } else {
            Self::explicit(default, allowed)
        }
    }

    /// Creates a checked explicit detail policy.
    fn explicit(default: Option<ImageDetail>, allowed: Vec<ImageDetail>) -> Option<Self> {
        is_nonempty_unique(&allowed).then_some(Self::Explicit { default, allowed })
    }

    /// Returns the known effective detail when the request omits the field.
    const fn default(&self) -> Option<ImageDetail> {
        match self {
            Self::OmittedOnly { default } | Self::Explicit { default, .. } => *default,
        }
    }

    /// Returns the explicit detail domain, or an empty slice for omission-only.
    fn allowed(&self) -> &[ImageDetail] {
        match self {
            Self::OmittedOnly { .. } => &[],
            Self::Explicit { allowed, .. } => allowed,
        }
    }

    /// Returns whether the detail policy remains a valid closed state.
    fn is_valid(&self) -> bool {
        match self {
            Self::OmittedOnly { .. } => true,
            Self::Explicit { allowed, .. } => is_nonempty_unique(allowed),
        }
    }

    /// Returns whether one explicit detail value is accepted.
    fn supports_explicit(&self, detail: ImageDetail) -> bool {
        self.allowed().contains(&detail)
    }

    /// Projects the closed detail policy into the existing flat detail object.
    fn wire_projection(&self) -> ImageDetailCapabilities {
        ImageDetailCapabilities {
            default: self.default(),
            allowed: self.allowed().to_vec(),
        }
    }
}

impl Serialize for ImageInputInterfaceCapabilities {
    /// Serializes only the downstream-safe flat projection, never the owned execution union.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.wire_projection().serialize(serializer)
    }
}

/// Returns whether a slice is non-empty and duplicate-free without changing its stable order.
fn is_nonempty_unique<T: Eq>(values: &[T]) -> bool {
    !values.is_empty()
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| !values[index + 1..].contains(value))
}

/// Typed multimodal inputs guaranteed by one protocol interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MultimodalInputCapabilities {
    image: Option<ImageInputInterfaceCapabilities>,
    audio: Option<AudioInputInterfaceCapabilities>,
    voice_conditioning: Option<AudioInputInterfaceCapabilities>,
}

/// Typed multimodal output profiles guaranteed by one protocol interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MultimodalOutputCapabilities {
    audio: Option<AudioOutputInterfaceCapabilities>,
}

/// Unique, fixed capability contract for one protocol interface, used directly by request preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInterfaceCapabilities {
    context_window: ContextWindow,
    modalities: ModelModalities,
    image_input: Option<ImageInputInterfaceCapabilities>,
    audio: Option<AudioInterfaceCapabilities>,
    supported_parameters: Vec<String>,
    streaming: SupportState,
    non_streaming: SupportState,
    system_messages: SupportState,
    tools: ToolCapabilities,
    structured_outputs: Option<StructuredOutputProfile>,
    reasoning: InterfaceReasoningCapabilities,
    response_includes: Vec<ResponseInclude>,
    state: StateCapabilities,
}

/// Transient Models projection derived from the closed execution profile.
#[derive(Serialize)]
struct StructuredOutputCapabilitiesWire {
    support: SupportState,
    modes: &'static [StructuredOutputMode],
    strict_schema: SupportState,
}

impl From<Option<StructuredOutputProfile>> for StructuredOutputCapabilitiesWire {
    /// Projects the execution profile without retaining independently mutable DTO state.
    fn from(profile: Option<StructuredOutputProfile>) -> Self {
        match profile {
            Some(profile) => Self {
                support: SupportState::Supported,
                modes: profile.modes(),
                strict_schema: SupportState::from_bool(profile.supports_strict_schema()),
            },
            None => Self {
                support: SupportState::Unsupported,
                modes: &[],
                strict_schema: SupportState::Unsupported,
            },
        }
    }
}

/// Borrowed wire projection that preserves the established Models extension layout.
#[derive(Serialize)]
struct ModelInterfaceCapabilitiesWire<'a> {
    context_window: &'a ContextWindow,
    modalities: &'a ModelModalities,
    multimodal_input: MultimodalInputCapabilities,
    multimodal_output: MultimodalOutputCapabilities,
    audio_task: Option<AudioTaskProjection>,
    supported_parameters: &'a [String],
    streaming: SupportState,
    non_streaming: SupportState,
    system_messages: SupportState,
    tools: &'a ToolCapabilities,
    structured_outputs: StructuredOutputCapabilitiesWire,
    reasoning: &'a InterfaceReasoningCapabilities,
    response_includes: &'a [ResponseInclude],
    state: &'a StateCapabilities,
}

impl Serialize for ModelInterfaceCapabilities {
    /// Serializes the private unions through the stable downstream-safe projection only.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Derive private execution unions into transient downstream-safe wire projections.
        let (audio, voice_conditioning, audio_output, audio_task) =
            self.audio
                .as_ref()
                .map_or((None, None, None, None), |audio| {
                    let (input, conditioning) = audio.multimodal_input();
                    (
                        input,
                        conditioning,
                        audio.multimodal_output(),
                        Some(audio.task_projection()),
                    )
                });
        let structured_outputs = StructuredOutputCapabilitiesWire::from(self.structured_outputs);

        // Build the stable wire object only after deriving every transient capability projection.
        ModelInterfaceCapabilitiesWire {
            context_window: &self.context_window,
            modalities: &self.modalities,
            multimodal_input: MultimodalInputCapabilities {
                image: self.image_input.clone(),
                audio,
                voice_conditioning,
            },
            multimodal_output: MultimodalOutputCapabilities {
                audio: audio_output,
            },
            audio_task,
            supported_parameters: &self.supported_parameters,
            streaming: self.streaming,
            non_streaming: self.non_streaming,
            system_messages: self.system_messages,
            tools: &self.tools,
            structured_outputs,
            reasoning: &self.reasoning,
            response_includes: &self.response_includes,
            state: &self.state,
        }
        .serialize(serializer)
    }
}

/// Encoding contract exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingEncodingCapabilities {
    default: EmbeddingEncoding,
    allowed: Option<Vec<EmbeddingEncoding>>,
}

/// Dimension contract exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingDimensionCapabilities {
    default: u32,
    allowed: Option<EmbeddingDimensionDomain>,
}

/// Request limits exposed by one Embeddings execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingLimits {
    max_inputs: u32,
    max_tokens_per_input: Option<u32>,
    max_total_tokens: Option<u32>,
    locally_counted_input_forms: Vec<EmbeddingInputForm>,
}

/// Unique fixed capability contract for the Embeddings Create operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EmbeddingInterfaceCapabilities {
    input_forms: Vec<EmbeddingInputForm>,
    encoding: EmbeddingEncodingCapabilities,
    dimensions: EmbeddingDimensionCapabilities,
    limits: EmbeddingLimits,
    supported_parameters: Vec<String>,
}

impl EmbeddingInterfaceCapabilities {
    /// Returns whether this interface accepts the analyzed input form.
    pub(crate) fn supports_input_form(&self, input_form: EmbeddingInputForm) -> bool {
        self.input_forms.contains(&input_form)
    }

    /// Resolves an omitted or explicit encoding without adding a local conversion.
    pub(crate) fn resolve_encoding(
        &self,
        requested: Option<EmbeddingEncoding>,
    ) -> Option<EmbeddingEncoding> {
        match requested {
            None => Some(self.encoding.default),
            Some(requested)
                if self
                    .encoding
                    .allowed
                    .as_ref()
                    .is_some_and(|allowed| allowed.contains(&requested)) =>
            {
                Some(requested)
            }
            Some(_) => None,
        }
    }

    /// Resolves an omitted or explicit positive dimension against the fixed domain.
    pub(crate) fn resolve_dimensions(&self, requested: Option<u32>) -> Option<u32> {
        match requested {
            None => Some(self.dimensions.default),
            Some(requested)
                if self
                    .dimensions
                    .allowed
                    .is_some_and(|allowed| allowed.contains(requested)) =>
            {
                Some(requested)
            }
            Some(_) => None,
        }
    }

    /// Returns whether this interface exposes an optional top-level request parameter.
    pub(crate) fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|supported| supported == parameter)
    }

    /// Returns the maximum number of input items accepted by one request.
    pub(crate) const fn max_inputs(&self) -> u32 {
        self.limits.max_inputs
    }

    /// Returns the optional maximum token count for one locally countable input.
    pub(crate) const fn max_tokens_per_input(&self) -> Option<u32> {
        self.limits.max_tokens_per_input
    }

    /// Returns the optional maximum total token count for locally countable inputs.
    pub(crate) const fn max_total_tokens(&self) -> Option<u32> {
        self.limits.max_total_tokens
    }

    /// Returns whether this input form's token counts are enforced before egress.
    pub(crate) fn counts_tokens_locally(&self, input_form: EmbeddingInputForm) -> bool {
        self.limits
            .locally_counted_input_forms
            .contains(&input_form)
    }
}

impl ModelInterfaceCapabilities {
    /// Returns whether this generation interface accepts one optional top-level request parameter.
    pub(crate) fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|supported| supported == parameter)
    }

    /// Returns whether this Responses interface guarantees one additional output projection.
    pub(crate) fn supports_response_include(&self, include: ResponseInclude) -> bool {
        self.response_includes.contains(&include)
    }

    /// Returns whether the interface guarantees streaming support.
    pub(crate) const fn supports_streaming(&self) -> bool {
        self.streaming.is_supported()
    }

    /// Returns whether the interface guarantees one complete non-streaming JSON response.
    pub(crate) const fn supports_non_streaming(&self) -> bool {
        self.non_streaming.is_supported()
    }

    /// Returns whether the interface guarantees one function-tool choice mode.
    pub(crate) fn supports_tool_choice(&self, mode: ToolChoiceMode) -> bool {
        self.tools.support.is_supported() && self.tools.tool_choice_modes.contains(&mode)
    }

    /// Returns whether the interface guarantees parallel function calls.
    pub(crate) const fn supports_parallel_tool_calls(&self) -> bool {
        self.tools.parallel_calls.is_supported()
    }

    /// Returns whether strict function-tool JSON Schema is guaranteed.
    pub(crate) const fn supports_strict_tool_schema(&self) -> bool {
        self.tools.strict_schema.is_supported()
    }

    /// Returns the typed image-input profile guaranteed by every interface candidate.
    pub(crate) fn image_input(&self) -> Option<&ImageInputInterfaceCapabilities> {
        self.image_input.as_ref()
    }

    /// Returns the closed audio contract guaranteed by every interface candidate.
    pub(crate) const fn audio(&self) -> Option<&AudioInterfaceCapabilities> {
        self.audio.as_ref()
    }

    /// Returns the closed structured-output profile guaranteed by every interface candidate.
    pub(crate) const fn structured_outputs(&self) -> Option<StructuredOutputProfile> {
        self.structured_outputs
    }

    /// Returns whether the interface guarantees `store: true`.
    pub(crate) const fn supports_store(&self) -> bool {
        self.state.store.is_supported()
    }

    /// Returns whether the interface guarantees background responses.
    pub(crate) const fn supports_background(&self) -> bool {
        self.state.background.is_supported()
    }

    /// Returns the maximum output-token count guaranteed by the interface.
    pub(crate) const fn max_output_tokens(&self) -> Option<u32> {
        self.context_window.max_output_tokens()
    }

    /// Returns the interface's reasoning evidence state.
    pub(crate) const fn reasoning_support(&self) -> SupportState {
        self.reasoning.support
    }

    /// Resolves one requested reasoning level against the fixed interface input policy.
    pub(crate) fn resolve_reasoning_level(
        &self,
        requested: ReasoningLevel,
    ) -> Option<ReasoningLevel> {
        // Unknown or unsupported reasoning cannot acquire an executable level through normalization.
        if !self.reasoning.support.is_supported() {
            return None;
        }

        // Apply only the Public Model policy compiled beside the executable level intersection.
        self.reasoning
            .input_policy
            .resolve(requested, &self.reasoning.levels)
    }
}

/// Typed OpenAI-compatible operation contracts of a Public Model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaces {
    chat_completions: Option<ModelInterfaceCapabilities>,
    responses: Option<ModelInterfaceCapabilities>,
    embeddings: Option<EmbeddingInterfaceCapabilities>,
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
