//! Fixed downstream Public Model contracts and their private execution snapshot.
//!
//! Client-visible DTOs remain in this facade. Private submodules own execution candidates and
//! startup compilation so serialized responses cannot acquire Provider, Target, Route,
//! upstream-model, or credential topology by accident.

use serde::Serialize;

use crate::core::{
    AudioFormat, AudioInputCapabilities, AudioInputSource, AudioOutputCapabilities, AudioTask,
    EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, ImageDetail,
    ImageInputCapabilities, ImageInputSource, ImageMediaType, ReasoningOutput,
};

pub use crate::core::{StructuredOutputMode, ToolChoiceMode};

use super::{
    InputModality, ModelContextLength, ModelLifecycle, OutputModality, ReasoningLevel,
    ReasoningSupport,
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

/// Structured-output capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuredOutputCapabilities {
    support: SupportState,
    modes: Vec<StructuredOutputMode>,
    strict_schema: SupportState,
}

/// Reasoning capabilities of one downstream interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InterfaceReasoningCapabilities {
    support: SupportState,
    levels: Vec<ReasoningLevel>,
    output: ReasoningOutputMode,
}

/// Persistent-state capabilities of one downstream interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StateCapabilities {
    store: SupportState,
    previous_response_id: SupportState,
    background: SupportState,
}

/// Explicit and omitted `detail` behavior for one image-input interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImageDetailCapabilities {
    default: Option<ImageDetail>,
    allowed: Vec<ImageDetail>,
}

/// Request-local image profile ceilings compiled for one fixed execution interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImageInputLimits {
    max_parts: u32,
    max_url_length: u32,
    max_inline_encoded_bytes: u32,
    max_inline_decoded_bytes: u32,
    max_total_inline_encoded_bytes: u32,
    max_total_inline_decoded_bytes: u32,
}

/// Typed image source, format, detail, and limit contract for one Native interface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImageInputInterfaceCapabilities {
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
            sources: value.sources.to_vec(),
            formats: value.formats.to_vec(),
            limits: AudioInputLimits {
                max_parts: value.max_parts,
                max_url_length: value.max_url_length,
                max_inline_encoded_bytes: value.max_inline_encoded_bytes,
                max_inline_decoded_bytes: value.max_inline_decoded_bytes,
                max_total_inline_encoded_bytes: value.max_total_inline_encoded_bytes,
                max_total_inline_decoded_bytes: value.max_total_inline_decoded_bytes,
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
}

impl AudioOutputInterfaceCapabilities {
    /// Converts one static audio output profile into a downstream-safe owned contract.
    fn from_capabilities(value: AudioOutputCapabilities) -> Self {
        Self {
            formats: value.formats.to_vec(),
            streaming_formats: value.streaming_formats.to_vec(),
            voices: value
                .voices
                .iter()
                .map(|voice| (*voice).to_owned())
                .collect(),
            max_inline_encoded_bytes: value.max_inline_encoded_bytes,
            max_inline_decoded_bytes: value.max_inline_decoded_bytes,
            max_stream_decoded_bytes: value.max_stream_decoded_bytes,
        }
    }

    /// Intersects all candidate profiles without exposing Route or Provider identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = values.first()?.to_owned().clone();
        let mut result = first;
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

impl ImageInputInterfaceCapabilities {
    /// Converts one static Upstream API image profile into a downstream-safe owned contract.
    fn from_capabilities(value: ImageInputCapabilities) -> Self {
        Self {
            sources: value.sources.to_vec(),
            media_types: value.media_types.to_vec(),
            detail: ImageDetailCapabilities {
                default: value.detail_default,
                allowed: value.allowed_details.to_vec(),
            },
            limits: ImageInputLimits {
                max_parts: value.max_parts,
                max_url_length: value.max_url_length,
                max_inline_encoded_bytes: value.max_inline_encoded_bytes,
                max_inline_decoded_bytes: value.max_inline_decoded_bytes,
                max_total_inline_encoded_bytes: value.max_total_inline_encoded_bytes,
                max_total_inline_decoded_bytes: value.max_total_inline_decoded_bytes,
            },
        }
    }

    /// Intersects every complete Route profile without retaining Provider or Route identity.
    fn intersection<'a>(values: impl Iterator<Item = Option<&'a Self>>) -> Option<Self> {
        // Require every executable candidate to contribute a typed image profile.
        let values = values.collect::<Option<Vec<_>>>()?;
        let first = values.first()?.to_owned().clone();

        // Intersect source, media-type, and explicit-detail sets in stable enum order.
        let mut result = first;
        result
            .sources
            .retain(|source| values.iter().all(|value| value.sources.contains(source)));
        result.media_types.retain(|media_type| {
            values
                .iter()
                .all(|value| value.media_types.contains(media_type))
        });
        result.detail.allowed.retain(|detail| {
            values
                .iter()
                .all(|value| value.detail.allowed.contains(detail))
        });
        if values
            .iter()
            .any(|value| value.detail.default != result.detail.default)
        {
            return None;
        }

        // A data URL is unusable when no inline media type survives the candidate intersection.
        if result.media_types.is_empty() {
            result
                .sources
                .retain(|source| *source != ImageInputSource::DataUrl);
        }

        // Narrow every numeric limit to the minimum guaranteed by all candidates.
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

        // An empty source intersection cannot advertise image input.
        (!result.sources.is_empty()).then_some(result)
    }

    /// Returns whether this interface accepts one source kind.
    pub(crate) fn supports_source(&self, source: ImageInputSource) -> bool {
        self.sources.contains(&source)
    }

    /// Returns whether this interface accepts one inline media type.
    pub(crate) fn supports_media_type(&self, media_type: ImageMediaType) -> bool {
        self.media_types.contains(&media_type)
    }

    /// Returns whether this interface accepts one explicit detail value.
    pub(crate) fn supports_detail(&self, detail: ImageDetail) -> bool {
        self.detail.allowed.contains(&detail)
    }

    /// Returns the per-request image part limit.
    pub(crate) const fn max_parts(&self) -> u32 {
        self.limits.max_parts
    }

    /// Returns the maximum UTF-8 byte length of one remote URL.
    pub(crate) const fn max_url_length(&self) -> u32 {
        self.limits.max_url_length
    }

    /// Returns the maximum Base64 payload length for one inline image.
    pub(crate) const fn max_inline_encoded_bytes(&self) -> u32 {
        self.limits.max_inline_encoded_bytes
    }

    /// Returns the maximum decoded length for one inline image.
    pub(crate) const fn max_inline_decoded_bytes(&self) -> u32 {
        self.limits.max_inline_decoded_bytes
    }

    /// Returns the cumulative Base64 payload limit for inline images.
    pub(crate) const fn max_total_inline_encoded_bytes(&self) -> u32 {
        self.limits.max_total_inline_encoded_bytes
    }

    /// Returns the cumulative decoded-byte limit for inline images.
    pub(crate) const fn max_total_inline_decoded_bytes(&self) -> u32 {
        self.limits.max_total_inline_decoded_bytes
    }
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaceCapabilities {
    context_window: ContextWindow,
    modalities: ModelModalities,
    multimodal_input: MultimodalInputCapabilities,
    multimodal_output: MultimodalOutputCapabilities,
    audio_task: Option<AudioTask>,
    supported_parameters: Vec<String>,
    streaming: SupportState,
    non_streaming: SupportState,
    system_messages: SupportState,
    tools: ToolCapabilities,
    structured_outputs: StructuredOutputCapabilities,
    reasoning: InterfaceReasoningCapabilities,
    prompt_caching: SupportState,
    state: StateCapabilities,
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
        self.multimodal_input.image.as_ref()
    }

    /// Returns the typed business-audio input profile guaranteed by every candidate.
    pub(crate) fn audio_input(&self) -> Option<&AudioInputInterfaceCapabilities> {
        self.multimodal_input.audio.as_ref()
    }

    /// Returns the typed reference-voice conditioning profile guaranteed by every candidate.
    pub(crate) fn voice_conditioning(&self) -> Option<&AudioInputInterfaceCapabilities> {
        self.multimodal_input.voice_conditioning.as_ref()
    }

    /// Returns the typed generated-audio output profile guaranteed by every candidate.
    pub(crate) fn audio_output(&self) -> Option<&AudioOutputInterfaceCapabilities> {
        self.multimodal_output.audio.as_ref()
    }

    /// Returns the task identity fixed by this Chat Native audio interface.
    pub(crate) const fn audio_task(&self) -> Option<AudioTask> {
        self.audio_task
    }

    /// Returns whether one structured-output mode is guaranteed.
    pub(crate) fn supports_structured_output_mode(&self, mode: StructuredOutputMode) -> bool {
        self.structured_outputs.support.is_supported()
            && self.structured_outputs.modes.contains(&mode)
    }

    /// Returns whether strict structured-output JSON Schema is guaranteed.
    pub(crate) const fn supports_strict_structured_outputs(&self) -> bool {
        self.structured_outputs.strict_schema.is_supported()
    }

    /// Returns whether the interface guarantees `store: true`.
    pub(crate) const fn supports_store(&self) -> bool {
        self.state.store.is_supported()
    }

    /// Returns whether the interface guarantees `previous_response_id`.
    pub(crate) const fn supports_previous_response_id(&self) -> bool {
        self.state.previous_response_id.is_supported()
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

    /// Returns the reasoning levels guaranteed by the interface.
    pub(crate) fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &self.reasoning.levels
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
