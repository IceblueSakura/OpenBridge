//! Request facts and Route execution-plan data types.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    bridge::BridgePlan,
    core::{
        ApiProtocol, ApiRequest, AsrLanguage, AudioFormat, AudioInputSource, ChatStreamUsage,
        DashScopePromptExtendMode, EmbeddingEncoding, EmbeddingInputForm, EmbeddingRequest,
        FileDetail, FileInlineEncoding, FileMediaType, GenerationRequestField, ImageDetail,
        ImageInputSource, ImageMediaType, ImagesOutputFormat, ImagesRequest, ImagesResponseFormat,
        OperationKind, ResponseInclude, ToolChoiceMode,
    },
    registry::{OperationResponseBudget, ReasoningLevel, UpstreamApiKey},
};

/// Registry-independent request facts extracted from a downstream request.
#[derive(Debug)]
pub struct RequestRequirements {
    pub(super) public_model: String,
    pub(super) protocol: ApiProtocol,
    pub(super) is_streaming: bool,
    pub(super) chat_stream_usage: ChatStreamUsage,
    pub(super) requested_output_tokens: Option<u64>,
    pub(super) requested_parameters: BTreeSet<GenerationRequestField>,
    pub(super) requested_instructions: RequestedInstructions,
    pub(super) requested_capabilities: RequestedCapabilities,
}

/// Client-owned instruction fact extracted before registry preflight.
#[derive(Debug)]
pub(super) enum RequestedInstructions {
    /// Exact non-blank client text; surrounding whitespace remains significant.
    Client(String),
    /// No eligible client source was present, so planning must use the startup fallback.
    Default,
}

/// Registry-independent facts extracted from one strict Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRequestRequirements {
    pub(super) public_model: String,
    pub(super) input_form: EmbeddingInputForm,
    pub(super) input_count: u32,
    pub(super) token_counts: Option<Vec<u32>>,
    pub(super) requested_encoding: Option<EmbeddingEncoding>,
    pub(super) requested_dimensions: Option<u32>,
    pub(super) user_present: bool,
}

/// Single-candidate Native execution plan for an Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRoutePlan {
    pub(super) candidate: EmbeddingRouteCandidate,
    pub(super) input_count: u32,
    pub(super) encoding: EmbeddingEncoding,
    pub(super) dimensions: u32,
    pub(super) response_budget: OperationResponseBudget,
}

/// Trusted Native Embeddings Route candidate bound to one target and Upstream API.
#[derive(Debug)]
pub struct EmbeddingRouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_api_key: UpstreamApiKey,
    pub(super) request: EmbeddingRequest,
}

/// Registry-independent facts extracted from one strict Images Generations request.
#[derive(Debug)]
pub struct ImagesRequestRequirements {
    pub(super) public_model: String,
    pub(super) prompt_length: u32,
    pub(super) requested_outputs: Option<u32>,
    pub(super) requested_size: Option<ImagesRequestedSize>,
    pub(super) requested_response_format: Option<ImagesResponseFormat>,
    pub(super) requested_output_format: Option<ImagesOutputFormat>,
    pub(super) requested_stream: Option<bool>,
    pub(super) unsupported_standard_fields: Vec<ImagesUnsupportedStandardField>,
    pub(super) dashscope: DashScopeImagesRequestRequirements,
    pub(super) user_present: bool,
}

/// Frozen DashScope-only extension facts without retaining negative-prompt content.
#[derive(Debug, Default)]
pub(super) struct DashScopeImagesRequestRequirements {
    pub(super) prompt_extend: Option<bool>,
    pub(super) prompt_extend_mode: Option<DashScopePromptExtendMode>,
    pub(super) enable_thinking: Option<bool>,
    pub(super) negative_prompt_present: bool,
    pub(super) seed: Option<u32>,
    pub(super) watermark: Option<bool>,
}

impl DashScopeImagesRequestRequirements {
    /// Returns the first present extension field for field-level capability errors.
    pub(super) const fn first_present_parameter(&self) -> Option<&'static str> {
        if self.prompt_extend.is_some() {
            Some("prompt_extend")
        } else if self.prompt_extend_mode.is_some() {
            Some("prompt_extend_mode")
        } else if self.enable_thinking.is_some() {
            Some("enable_thinking")
        } else if self.negative_prompt_present {
            Some("negative_prompt")
        } else if self.seed.is_some() {
            Some("seed")
        } else if self.watermark.is_some() {
            Some("watermark")
        } else {
            None
        }
    }
}

/// Structurally valid OpenAI Images fields with no qwen execution semantics in this focus.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ImagesUnsupportedStandardField {
    Background,
    Moderation,
    OutputCompression,
    PartialImages,
    Quality,
    Style,
}

impl ImagesUnsupportedStandardField {
    /// Returns the exact downstream parameter name used in public errors.
    pub(super) const fn parameter(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Moderation => "moderation",
            Self::OutputCompression => "output_compression",
            Self::PartialImages => "partial_images",
            Self::Quality => "quality",
            Self::Style => "style",
        }
    }
}

/// One parsed OpenAI size request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImagesRequestedSize {
    /// Let the selected model choose an output size.
    Auto,
    /// Request exact positive pixel dimensions.
    Exact {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}

/// Single-candidate Native execution plan for an Images Generations request.
#[derive(Debug)]
pub struct ImagesRoutePlan {
    pub(super) candidate: ImagesRouteCandidate,
    pub(super) outputs: u32,
    pub(super) size: Option<ImagesRequestedSize>,
    pub(super) response_format: ImagesResponseFormat,
    pub(super) response_budget: OperationResponseBudget,
}

/// Trusted Native Images Route candidate bound to one target and Upstream API.
#[derive(Debug)]
pub struct ImagesRouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_api_key: UpstreamApiKey,
    pub(super) request: ImagesRequest,
}

/// Execution plan that passed the Public Model fixed contract and binds ordered Routes.
///
/// Candidates retain Route configuration order. `allows_fallback` is not a general retry switch;
/// it prevents Provider-issued opaque state such as `previous_response_id` from being replayed to another target.
#[derive(Debug)]
pub struct RoutePlan {
    pub(super) candidates: Vec<RouteCandidate>,
    pub(super) is_streaming: bool,
    pub(super) allows_fallback: bool,
    pub(super) response_budget: OperationResponseBudget,
}

/// Execution candidate inheriting Public Model preflight and bound to one target/Upstream API.
#[derive(Debug)]
pub struct RouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_api_key: UpstreamApiKey,
    pub(super) request: ApiRequest,
    pub(super) bridge: Option<BridgePlan>,
    pub(super) stream_response_conversion: Option<StreamResponseConversion>,
}

/// Trusted response takeover required when one streaming-only upstream satisfies a non-streaming request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StreamResponseConversion {
    /// Buffers a complete typed Responses SSE lifecycle and returns its terminal response as JSON.
    BufferResponsesSse,
}

/// Capabilities actually used by one request, independent of any Upstream API configuration.
///
/// Common generation requirements are explicit fields; Responses-only state remains separate so
/// request analysis cannot conflate the two fixed protocol contracts.
#[derive(Debug)]
pub(super) struct RequestedCapabilities {
    pub(super) streaming: bool,
    pub(super) function_tool_choice: Option<ToolChoiceMode>,
    pub(super) unknown_tool_choice: bool,
    pub(super) function_tool_strict_schema: bool,
    pub(super) parallel_tool_calls: bool,
    pub(super) image_input: Option<ImageInputRequirements>,
    pub(super) file_input: Option<FileInputRequirements>,
    pub(super) audio: Option<RequestedAudio>,
    pub(super) structured_output: RequestedStructuredOutput,
    pub(super) unmodeled_tools: bool,
    pub(super) reasoning: RequestedReasoning,
    pub(super) reasoning_summary: RequestedReasoningSummary,
    pub(super) previous_response_id: bool,
    pub(super) background: bool,
    pub(super) response_includes: BTreeSet<ResponseInclude>,
}

/// Closed structured-output requirement extracted from one downstream generation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestedStructuredOutput {
    /// The request omits a format constraint or explicitly requests plain text.
    Unconstrained,
    /// The request asks for a syntactically valid JSON object.
    JsonObject,
    /// The request asks for JSON Schema output with one exact strictness requirement.
    JsonSchema(RequestedJsonSchemaStrictness),
    /// The request carries an unknown format or conflicting standard format locations.
    Unknown,
}

/// Strictness requested for a JSON Schema structured-output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestedJsonSchemaStrictness {
    /// The request omits `strict: true`.
    NonStrict,
    /// The request explicitly enables strict JSON Schema constraints.
    Strict,
}

/// Frozen protocol shape for one request that uses Chat Native audio fields.
#[derive(Debug)]
pub(super) enum RequestedAudio {
    /// Content-understanding or speech-recognition input and optional ASR controls.
    Input {
        /// Bounded audio resources carried by user content parts.
        resources: AudioInputRequirements,
        /// Task-neutral classification of the complete Chat message envelope.
        message_shape: InputAudioMessageShape,
        /// Presence and language facts from the optional ASR control object.
        asr_options: RequestedAsrOptions,
    },
    /// Generated-audio delivery controls and optional voice selection or conditioning.
    Generated {
        /// Requested output format; the streaming flag remains a shared request fact.
        delivery: RequestedAudioDelivery,
        /// Task-neutral classification of the complete Chat message envelope.
        message_shape: GeneratedAudioMessageShape,
        /// Preset or reference-voice request shape.
        voice: RequestedVoice,
    },
}

/// Closed Chat message shapes for requests carrying `input_audio` content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InputAudioMessageShape {
    /// The envelope contains exactly one user message whose sole part is the audio resource.
    SingleUserAudioOnly,
    /// The envelope contains instructions, additional parts, or any other message arrangement.
    GeneralConversation,
}

/// Closed Chat message shapes for requests asking the model to generate audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GeneratedAudioMessageShape {
    /// The envelope contains exactly one assistant target-text message.
    AssistantTextOnly,
    /// One user text message is followed by one assistant target-text message.
    UserTextThenAssistantText,
    /// The envelope is missing required text or contains any additional or differently ordered message.
    Other,
}

/// Presence and optional language carried by `asr_options`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestedAsrOptions {
    /// The request omitted `asr_options` or set it to null.
    Absent,
    /// The request supplied a syntactically valid ASR control object.
    Present {
        /// Optional known or unsupported language value.
        language: Option<RequestedAsrLanguage>,
    },
}

/// Closed classification of one syntactically valid ASR language string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestedAsrLanguage {
    /// Language represented by the typed executable profile.
    Known(AsrLanguage),
    /// Well-formed string that the current typed profile cannot accept.
    Unsupported,
}

/// Requested output format for generated Chat audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RequestedAudioDelivery {
    pub(super) format: AudioFormat,
}

/// Requested generated-audio voice shape without any Provider interpretation.
#[derive(Debug)]
pub(super) enum RequestedVoice {
    /// The request omitted the voice field.
    Unspecified,
    /// A non-empty preset voice identifier.
    Preset(String),
    /// Inline reference audio used only as voice conditioning.
    ReferenceVoice(AudioInputRequirements),
}

/// Frozen source, format, and size facts for one audio resource set.
#[derive(Debug, Default)]
pub(super) struct AudioInputRequirements {
    pub(super) sources: BTreeMap<AudioInputSource, AudioInputSourceRequirements>,
    pub(super) part_count: u32,
    pub(super) total_inline_encoded_bytes: u32,
    pub(super) total_inline_decoded_bytes: u32,
}

/// Source-owned request facts retained without media bytes or URLs.
#[derive(Debug, Default)]
pub(super) struct AudioInputSourceRequirements {
    pub(super) formats: BTreeSet<AudioFormat>,
    pub(super) max_url_length: u32,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) total_inline_encoded_bytes: u32,
    pub(super) total_inline_decoded_bytes: u32,
}

/// Frozen image-input facts extracted without selecting or inspecting any Route.
#[derive(Debug, Default)]
pub(super) struct ImageInputRequirements {
    pub(super) sources: BTreeSet<ImageInputSource>,
    pub(super) media_types: BTreeSet<ImageMediaType>,
    pub(super) details: BTreeSet<ImageDetail>,
    pub(super) unsupported_media_type: bool,
    pub(super) part_count: u32,
    pub(super) max_url_length: u32,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) total_inline_encoded_bytes: u32,
    pub(super) total_inline_decoded_bytes: u32,
}

/// Frozen file-input facts extracted without retaining filenames, URLs, or media payloads.
#[derive(Debug, Default)]
pub(super) struct FileInputRequirements {
    pub(super) sources: BTreeSet<crate::registry::FileInputSource>,
    pub(super) encodings: BTreeSet<FileInlineEncoding>,
    pub(super) media_types: BTreeSet<FileMediaType>,
    pub(super) details: BTreeSet<FileDetail>,
    pub(super) part_count: u32,
    pub(super) max_filename_length: u32,
    pub(super) max_url_length: u32,
    pub(super) max_inline_encoded_bytes: u32,
    pub(super) max_inline_decoded_bytes: u32,
    pub(super) total_inline_encoded_bytes: u32,
    pub(super) total_inline_decoded_bytes: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RequestedReasoning {
    None,
    Unspecified,
    Level(ReasoningLevel),
    UnknownLevel,
    Conflicting,
}

/// Closed Responses reasoning-summary request shape accepted by the gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestedReasoningSummary {
    /// The request omits the summary child field.
    Absent,
    /// The compatibility boolean explicitly declines a summary without disabling reasoning.
    Disabled,
    /// The request asks the upstream Responses API to choose an automatic summary.
    Auto,
    /// The request uses an unsupported string or malformed value.
    Invalid,
}

impl RequestRequirements {
    /// Returns the Public Model selected by the downstream request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the native protocol used by the request.
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// Returns whether the request requires a streaming response.
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }
}

impl EmbeddingRequestRequirements {
    /// Returns the Public Model selected by the downstream Embeddings request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the exact analyzed input form.
    pub fn input_form(&self) -> EmbeddingInputForm {
        self.input_form
    }

    /// Returns the number of logical embedding inputs.
    pub fn input_count(&self) -> u32 {
        self.input_count
    }
}

impl EmbeddingRoutePlan {
    /// Returns the single trusted Embeddings candidate.
    pub fn candidate(&self) -> &EmbeddingRouteCandidate {
        &self.candidate
    }

    /// Returns the expected number of response vectors.
    pub fn input_count(&self) -> u32 {
        self.input_count
    }

    /// Returns the effective output encoding after fixed-interface preflight.
    pub fn encoding(&self) -> EmbeddingEncoding {
        self.encoding
    }

    /// Returns the effective vector dimension after fixed-interface preflight.
    pub fn dimensions(&self) -> u32 {
        self.dimensions
    }

    /// Returns the JSON response limit compiled with the Embeddings interface.
    pub(crate) const fn max_json_response_body_bytes(&self) -> usize {
        self.response_budget.max_json_body_bytes()
    }
}

impl EmbeddingRouteCandidate {
    /// Returns the candidate Route ID.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the trusted Upstream Target ID.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the trusted typed Upstream API operation.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_api_key.operation()
    }

    /// Returns the complete trusted Upstream API identity.
    pub fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the preserved Native Embeddings request.
    pub fn request(&self) -> &EmbeddingRequest {
        &self.request
    }
}

impl ImagesRequestRequirements {
    /// Returns the Public Model selected by the downstream Images request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the non-blank prompt length frozen by strict analysis.
    pub fn prompt_length(&self) -> u32 {
        self.prompt_length
    }

    /// Returns the explicit requested output count when present.
    pub fn requested_outputs(&self) -> Option<u32> {
        self.requested_outputs
    }

    /// Returns the explicit requested `WxH` size when present.
    pub fn requested_size(&self) -> Option<ImagesRequestedSize> {
        self.requested_size
    }

    /// Returns the explicit requested response format when present.
    pub fn requested_response_format(&self) -> Option<ImagesResponseFormat> {
        self.requested_response_format
    }
}

impl ImagesRequestedSize {
    /// Returns the width component in pixels.
    pub fn width(&self) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Exact { width, .. } => Some(*width),
        }
    }

    /// Returns the height component in pixels.
    pub fn height(&self) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Exact { height, .. } => Some(*height),
        }
    }
}

impl ImagesRoutePlan {
    /// Returns the single trusted Images candidate.
    pub fn candidate(&self) -> &ImagesRouteCandidate {
        &self.candidate
    }

    /// Returns the resolved output count after fixed-interface preflight.
    pub fn outputs(&self) -> u32 {
        self.outputs
    }

    /// Returns the resolved effective size after fixed-interface preflight.
    pub fn size(&self) -> Option<ImagesRequestedSize> {
        self.size
    }

    /// Returns the resolved response format after fixed-interface preflight.
    pub fn response_format(&self) -> ImagesResponseFormat {
        self.response_format
    }

    /// Returns the JSON response limit compiled with the Images interface.
    pub(crate) const fn max_json_response_body_bytes(&self) -> usize {
        self.response_budget.max_json_body_bytes()
    }
}

impl ImagesRouteCandidate {
    /// Returns the candidate Route ID.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the trusted Upstream Target ID.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the complete trusted Upstream API identity.
    pub fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the preserved Native Images request.
    pub fn request(&self) -> &ImagesRequest {
        &self.request
    }
}

impl RoutePlan {
    /// Returns the highest-priority target ID.
    pub fn upstream_target_id(&self) -> &str {
        self.primary().upstream_target_id()
    }

    /// Returns the request for the highest-priority candidate.
    pub fn request(&self) -> &ApiRequest {
        self.primary().request()
    }

    /// Returns execution candidates ordered by configured Routes.
    pub fn candidates(&self) -> &[RouteCandidate] {
        &self.candidates
    }

    /// Returns whether the original request requires streaming.
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    /// Returns the JSON response limit compiled with this generation interface.
    pub(crate) const fn max_json_response_body_bytes(&self) -> usize {
        self.response_budget.max_json_body_bytes()
    }

    /// Returns the SSE event limit compiled with this generation interface.
    pub(crate) fn max_sse_event_bytes(&self) -> usize {
        match self.response_budget.max_sse_event_bytes() {
            Some(limit) => limit,
            None => unreachable!("generation plans always own an SSE response budget"),
        }
    }

    /// Returns whether cross-target fallback is allowed.
    pub fn allows_fallback(&self) -> bool {
        self.allows_fallback
    }

    /// Consumes the plan and returns its highest-priority candidate request.
    pub fn into_request(self) -> ApiRequest {
        self.candidates
            .into_iter()
            .next()
            .expect("route plan always has a candidate")
            .request
    }

    /// Returns the guaranteed highest-priority candidate.
    fn primary(&self) -> &RouteCandidate {
        self.candidates
            .first()
            .expect("route plan always has a candidate")
    }
}

impl RouteCandidate {
    /// Returns the candidate Route ID.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the Upstream Target ID bound to the candidate.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the typed Upstream API operation bound to the candidate.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_api_key.operation()
    }

    /// Returns the complete typed Upstream API identity bound to the candidate.
    pub fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the Native request for the candidate.
    pub fn request(&self) -> &ApiRequest {
        &self.request
    }

    /// Returns the response conversion plan for a Bridged Route; a Native candidate returns `None`.
    pub fn bridge(&self) -> Option<&BridgePlan> {
        self.bridge.as_ref()
    }

    /// Returns the trusted streaming-response conversion selected during planning.
    pub(crate) const fn stream_response_conversion(&self) -> Option<StreamResponseConversion> {
        self.stream_response_conversion
    }
}
