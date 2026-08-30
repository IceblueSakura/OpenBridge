//! Generation request facts and ordered Route execution-plan types.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    bridge::BridgePlan,
    core::{
        ApiProtocol, ApiRequest, AsrLanguage, AudioFormat, AudioInputSource, ChatStreamUsage,
        FileDetail, FileInlineEncoding, FileMediaType, GenerationRequestField, ImageDetail,
        ImageInputSource, ImageMediaType, OperationKind, ResponseInclude, ToolChoiceMode,
    },
    registry::{OperationResponseBudget, ReasoningLevel, UpstreamApiKey},
};

/// Registry-independent request facts extracted from a downstream request.
#[derive(Debug)]
pub struct RequestRequirements {
    pub(in crate::pipeline) public_model: String,
    pub(in crate::pipeline) protocol: ApiProtocol,
    pub(in crate::pipeline) is_streaming: bool,
    pub(in crate::pipeline) chat_stream_usage: ChatStreamUsage,
    pub(in crate::pipeline) requested_output_tokens: Option<RequestedOutputTokens>,
    pub(in crate::pipeline) requested_parameters: BTreeSet<GenerationRequestField>,
    pub(in crate::pipeline) requested_instructions: RequestedInstructions,
    pub(in crate::pipeline) requested_capabilities: RequestedCapabilities,
}

/// Maximum Generation output request with the deterministic standard field that supplied it.
#[derive(Clone, Copy, Debug)]
pub(in crate::pipeline) struct RequestedOutputTokens {
    pub(in crate::pipeline) value: u64,
    pub(in crate::pipeline) param: &'static str,
}

/// Client-owned instruction fact extracted before registry preflight.
#[derive(Debug)]
pub(in crate::pipeline) enum RequestedInstructions {
    /// Exact non-blank client text; surrounding whitespace remains significant.
    Client(String),
    /// No eligible client source was present, so planning must use the startup fallback.
    Default,
}

/// Execution plan that passed the Public Model fixed contract and binds ordered Routes.
///
/// Candidates retain Route configuration order. `allows_fallback` is not a general retry switch;
/// it prevents Provider-issued opaque state such as `previous_response_id` from being replayed to another target.
#[derive(Debug)]
pub struct RoutePlan {
    pub(in crate::pipeline) candidates: Vec<RouteCandidate>,
    pub(in crate::pipeline) is_streaming: bool,
    pub(in crate::pipeline) allows_fallback: bool,
    pub(in crate::pipeline) response_budget: OperationResponseBudget,
}

/// Execution candidate inheriting Public Model preflight and bound to one target/Upstream API.
#[derive(Debug)]
pub struct RouteCandidate {
    pub(in crate::pipeline) upstream_target_id: String,
    pub(in crate::pipeline) upstream_api_key: UpstreamApiKey,
    pub(in crate::pipeline) request: ApiRequest,
    pub(in crate::pipeline) upstream_streaming: bool,
    pub(in crate::pipeline) generation_plan: BridgePlan,
    pub(in crate::pipeline) stream_response_conversion: Option<StreamResponseConversion>,
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
pub(in crate::pipeline) struct RequestedCapabilities {
    pub(in crate::pipeline) streaming: bool,
    pub(in crate::pipeline) function_tools: bool,
    pub(in crate::pipeline) function_tool_choice: Option<ToolChoiceMode>,
    pub(in crate::pipeline) unknown_tool_choice: bool,
    pub(in crate::pipeline) function_tool_strict_schema: bool,
    pub(in crate::pipeline) parallel_tool_calls: RequestedParallelToolCalls,
    pub(in crate::pipeline) image_input: Option<ImageInputRequirements>,
    pub(in crate::pipeline) file_input: Option<FileInputRequirements>,
    pub(in crate::pipeline) audio: Option<RequestedAudio>,
    pub(in crate::pipeline) structured_output: RequestedStructuredOutput,
    pub(in crate::pipeline) unmodeled_tools: bool,
    pub(in crate::pipeline) reasoning: RequestedReasoning,
    pub(in crate::pipeline) reasoning_summary: RequestedReasoningSummary,
    pub(in crate::pipeline) previous_response_id: bool,
    pub(in crate::pipeline) background: bool,
    pub(in crate::pipeline) response_includes: BTreeSet<ResponseInclude>,
}

/// Value-sensitive `parallel_tool_calls` requirement after function-tool analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum RequestedParallelToolCalls {
    /// The field is omitted, nullable where allowed, or cannot affect any executable function tool.
    Inactive,
    /// Active function tools may be emitted in parallel.
    Allow,
    /// Active function tools must remain serial.
    RequireSerial,
}

/// Closed structured-output requirement extracted from one downstream generation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum RequestedStructuredOutput {
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
pub(in crate::pipeline) enum RequestedJsonSchemaStrictness {
    /// The request omits `strict: true`.
    NonStrict,
    /// The request explicitly enables strict JSON Schema constraints.
    Strict,
}

/// Frozen protocol shape for one request that uses Chat Native audio fields.
#[derive(Debug)]
pub(in crate::pipeline) enum RequestedAudio {
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
pub(in crate::pipeline) enum InputAudioMessageShape {
    /// The envelope contains exactly one user message whose sole part is the audio resource.
    SingleUserAudioOnly,
    /// The envelope contains instructions, additional parts, or any other message arrangement.
    GeneralConversation,
}

/// Closed Chat message shapes for requests asking the model to generate audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum GeneratedAudioMessageShape {
    /// The envelope contains exactly one assistant target-text message.
    AssistantTextOnly,
    /// One user text message is followed by one assistant target-text message.
    UserTextThenAssistantText,
    /// The envelope is missing required text or contains any additional or differently ordered message.
    Other,
}

/// Presence and optional language carried by `asr_options`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum RequestedAsrOptions {
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
pub(in crate::pipeline) enum RequestedAsrLanguage {
    /// Language represented by the typed executable profile.
    Known(AsrLanguage),
    /// Well-formed string that the current typed profile cannot accept.
    Unsupported,
}

/// Requested output format for generated Chat audio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) struct RequestedAudioDelivery {
    pub(in crate::pipeline) format: AudioFormat,
}

/// Requested generated-audio voice shape without any Provider interpretation.
#[derive(Debug)]
pub(in crate::pipeline) enum RequestedVoice {
    /// The request omitted the voice field.
    Unspecified,
    /// A non-empty preset voice identifier.
    Preset(String),
    /// Inline reference audio used only as voice conditioning.
    ReferenceVoice(AudioInputRequirements),
}

/// Frozen source, format, and size facts for one audio resource set.
#[derive(Debug, Default)]
pub(in crate::pipeline) struct AudioInputRequirements {
    pub(in crate::pipeline) sources: BTreeMap<AudioInputSource, AudioInputSourceRequirements>,
    pub(in crate::pipeline) part_count: u32,
    pub(in crate::pipeline) total_inline_encoded_bytes: u32,
    pub(in crate::pipeline) total_inline_decoded_bytes: u32,
}

/// Source-owned request facts retained without media bytes or URLs.
#[derive(Debug, Default)]
pub(in crate::pipeline) struct AudioInputSourceRequirements {
    pub(in crate::pipeline) formats: BTreeSet<AudioFormat>,
    pub(in crate::pipeline) max_url_length: u32,
    pub(in crate::pipeline) max_inline_encoded_bytes: u32,
    pub(in crate::pipeline) max_inline_decoded_bytes: u32,
    pub(in crate::pipeline) total_inline_encoded_bytes: u32,
    pub(in crate::pipeline) total_inline_decoded_bytes: u32,
}

/// Frozen image-input facts extracted without selecting or inspecting any Route.
#[derive(Debug, Default)]
pub(in crate::pipeline) struct ImageInputRequirements {
    pub(in crate::pipeline) sources: BTreeSet<ImageInputSource>,
    pub(in crate::pipeline) media_types: BTreeSet<ImageMediaType>,
    pub(in crate::pipeline) details: BTreeSet<ImageDetail>,
    pub(in crate::pipeline) unsupported_media_type: bool,
    pub(in crate::pipeline) part_count: u32,
    pub(in crate::pipeline) max_url_length: u32,
    pub(in crate::pipeline) max_inline_encoded_bytes: u32,
    pub(in crate::pipeline) max_inline_decoded_bytes: u32,
    pub(in crate::pipeline) total_inline_encoded_bytes: u32,
    pub(in crate::pipeline) total_inline_decoded_bytes: u32,
}

/// Frozen file-input facts extracted without retaining filenames, URLs, or media payloads.
#[derive(Debug, Default)]
pub(in crate::pipeline) struct FileInputRequirements {
    pub(in crate::pipeline) sources: BTreeSet<crate::registry::FileInputSource>,
    pub(in crate::pipeline) encodings: BTreeSet<FileInlineEncoding>,
    pub(in crate::pipeline) media_types: BTreeSet<FileMediaType>,
    pub(in crate::pipeline) details: BTreeSet<FileDetail>,
    pub(in crate::pipeline) part_count: u32,
    pub(in crate::pipeline) max_filename_length: u32,
    pub(in crate::pipeline) max_url_length: u32,
    pub(in crate::pipeline) max_inline_encoded_bytes: u32,
    pub(in crate::pipeline) max_inline_decoded_bytes: u32,
    pub(in crate::pipeline) total_inline_encoded_bytes: u32,
    pub(in crate::pipeline) total_inline_decoded_bytes: u32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::pipeline) enum RequestedReasoning {
    None,
    Unspecified,
    Level(ReasoningLevel),
    UnknownLevel,
    Conflicting,
}

/// Closed Responses reasoning-summary request shape accepted by the gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::pipeline) enum RequestedReasoningSummary {
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

    /// Returns whether trusted planning requires an upstream SSE response.
    pub(crate) const fn upstream_streaming(&self) -> bool {
        self.upstream_streaming
    }

    /// Returns the canonical Static/Event plan shared by Native and Bridged Routes.
    pub fn generation_plan(&self) -> &BridgePlan {
        &self.generation_plan
    }

    /// Returns the trusted streaming-response conversion selected during planning.
    pub(crate) const fn stream_response_conversion(&self) -> Option<StreamResponseConversion> {
        self.stream_response_conversion
    }
}
