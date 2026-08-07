//! Chat Completions and Responses capability ceilings.
//!
//! This module owns generation-only fields and the common projection used for subset checks.
//! Reserved protocol positions fail closed until their runtime semantics exist.

use serde::Serialize;

/// Observable output type for upstream-generated reasoning.
///
/// `Unknown` means that wire evidence is insufficient to treat the output as readable text;
/// `Opaque` covers unreadable Provider-issued continuations such as Responses
/// `encrypted_content`. Only `PlainText` and `Summary` can enter a cross-protocol reasoning
/// channel, and the convertible direction remains protocol-specific.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningOutput {
    /// Upstream wire evidence is insufficient to determine the output format.
    #[default]
    Unknown,
    /// The upstream explicitly returns no reasoning output.
    Unsupported,
    /// The upstream returns readable complete reasoning text.
    PlainText,
    /// The upstream returns only a readable reasoning summary.
    Summary,
    /// The upstream returns an unreadable opaque or encrypted continuation.
    Opaque,
}

impl ReasoningOutput {
    /// Returns whether this output contains readable reasoning text or a summary.
    pub const fn is_readable(self) -> bool {
        matches!(self, Self::PlainText | Self::Summary)
    }

    /// Returns whether this configuration claims no additional reasoning output capability over the Provider contract.
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        matches!(
            (self, upper),
            (Self::Unknown | Self::Unsupported, _)
                | (Self::PlainText, Self::PlainText)
                | (Self::Summary, Self::Summary)
                | (Self::Opaque, Self::Opaque)
        )
    }
}

/// OpenAI-hosted tool kinds that a Responses Create request can reference.
///
/// These variants reserve standard protocol positions; the current pipeline, adapters, and Provider
/// registrations do not implement these tools.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedToolKind {
    /// Web search tool.
    WebSearch,
    /// File search tool.
    FileSearch,
    /// Code Interpreter tool.
    CodeInterpreter,
    /// Computer Use tool.
    ComputerUse,
    /// Image generation tool.
    ImageGeneration,
    /// Remote MCP tool.
    Mcp,
    /// Hosted shell tool.
    Shell,
    /// Apply patch tool.
    ApplyPatch,
    /// Tool search tool.
    ToolSearch,
    /// Skills tool.
    Skills,
    /// Programmatic Tool Calling tool.
    ProgrammaticToolCalling,
}

/// Standard additional output kinds for the Responses Create `include` field.
///
/// Variants use descriptive Rust names and Rustdoc identifies their wire paths; they currently
/// serve as reserved interface positions.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseInclude {
    /// `web_search_call.action.sources`.
    WebSearchCallSources,
    /// `code_interpreter_call.outputs`.
    CodeInterpreterCallOutputs,
    /// `computer_call_output.output.image_url`.
    ComputerCallOutputImageUrl,
    /// `file_search_call.results`.
    FileSearchCallResults,
    /// `message.input_image.image_url`.
    InputImageImageUrl,
    /// `message.output_text.logprobs`.
    OutputTextLogprobs,
    /// `reasoning.encrypted_content`.
    ReasoningEncryptedContent,
}

/// Standard image source kinds accepted by one protocol-native input profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageInputSource {
    /// An absolute HTTPS URL fetched by the upstream Provider.
    RemoteUrl,
    /// An inline RFC 2397-style Base64 data URL.
    DataUrl,
    /// An opaque Provider-issued file identifier.
    FileId,
}

/// Image media types that OpenBridge can validate without inspecting image content.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ImageMediaType {
    /// JPEG image data.
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// PNG image data.
    #[serde(rename = "image/png")]
    Png,
    /// GIF image data.
    #[serde(rename = "image/gif")]
    Gif,
    /// WebP image data.
    #[serde(rename = "image/webp")]
    Webp,
    /// BMP image data.
    #[serde(rename = "image/bmp")]
    Bmp,
}

impl ImageMediaType {
    /// Parses one canonical image media type without accepting aliases or parameters.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "image/jpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            "image/bmp" => Some(Self::Bmp),
            _ => None,
        }
    }
}

/// Standard image-detail values carried by Chat or Responses image parts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    /// Lets the upstream choose the effective image detail.
    Auto,
    /// Requests a low-detail representation.
    Low,
    /// Requests a high-detail representation.
    High,
    /// Requests the original image resolution when supported.
    Original,
}

impl ImageDetail {
    /// Parses one standard image-detail wire value.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "low" => Some(Self::Low),
            "high" => Some(Self::High),
            "original" => Some(Self::Original),
            _ => None,
        }
    }
}

/// Provider or Upstream API ceiling for protocol-native image inputs.
///
/// Byte limits apply to the Base64 payload after the data-URL prefix. The gateway request-body
/// limit remains an independent deployment-wide ceiling and may be smaller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageInputCapabilities {
    /// Allowed source kinds.
    pub sources: &'static [ImageInputSource],
    /// Allowed inline data-URL media types.
    pub media_types: &'static [ImageMediaType],
    /// Effective detail when the part omits `detail`, if the Provider documents one.
    pub detail_default: Option<ImageDetail>,
    /// Explicit detail values accepted on the wire.
    pub allowed_details: &'static [ImageDetail],
    /// Maximum image parts in one request.
    pub max_parts: u32,
    /// Maximum UTF-8 bytes in one remote URL.
    pub max_url_length: u32,
    /// Maximum Base64 characters in one inline image.
    pub max_inline_encoded_bytes: u32,
    /// Maximum decoded bytes in one inline image.
    pub max_inline_decoded_bytes: u32,
    /// Maximum cumulative Base64 characters across inline images.
    pub max_total_inline_encoded_bytes: u32,
    /// Maximum cumulative decoded bytes across inline images.
    pub max_total_inline_decoded_bytes: u32,
}

impl ImageInputCapabilities {
    /// Returns whether this profile stays within another Provider or API ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.sources
            .iter()
            .all(|source| upper.sources.contains(source))
            && self
                .media_types
                .iter()
                .all(|media_type| upper.media_types.contains(media_type))
            && self.detail_default == upper.detail_default
            && self
                .allowed_details
                .iter()
                .all(|detail| upper.allowed_details.contains(detail))
            && self.max_parts <= upper.max_parts
            && self.max_url_length <= upper.max_url_length
            && self.max_inline_encoded_bytes <= upper.max_inline_encoded_bytes
            && self.max_inline_decoded_bytes <= upper.max_inline_decoded_bytes
            && self.max_total_inline_encoded_bytes <= upper.max_total_inline_encoded_bytes
            && self.max_total_inline_decoded_bytes <= upper.max_total_inline_decoded_bytes
    }
}

/// Returns whether one optional image profile is conservatively bounded by another.
fn image_input_is_subset_of(
    value: Option<ImageInputCapabilities>,
    upper: Option<ImageInputCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
        (Some(_), None) => false,
    }
}

/// Shared generation-capability projection for Chat Completions and Responses.
///
/// This value is used only for common-protocol subset checks; static registrations must use
/// [`ChatCompletionsCapabilities`] or [`ResponsesCapabilities`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenerationCapabilities {
    /// Whether the endpoint is enabled.
    pub(crate) enabled: bool,
    /// Whether incremental results can be returned over SSE.
    pub(crate) streaming: bool,
    /// Whether JSON Schema function-tool calls are supported.
    pub(crate) function_calling: bool,
    /// Whether the request wire field `parallel_tool_calls: true` is supported.
    pub(crate) parallel_tool_calls: bool,
    /// Typed image input profile, or `None` when images are unsupported.
    pub(crate) image_input: Option<ImageInputCapabilities>,
    /// Whether structured-output constraints are supported.
    pub(crate) structured_outputs: bool,
    /// Whether the request wire field `store: true` is supported.
    pub(crate) store: bool,
    /// Observable type of upstream reasoning output.
    pub(crate) reasoning_output: ReasoningOutput,
}

impl GenerationCapabilities {
    /// Returns whether the current capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        (!self.enabled || upper.enabled)
            && (!self.streaming || upper.streaming)
            && (!self.function_calling || upper.function_calling)
            && (!self.parallel_tool_calls || upper.parallel_tool_calls)
            && image_input_is_subset_of(self.image_input, upper.image_input)
            && (!self.structured_outputs || upper.structured_outputs)
            && (!self.store || upper.store)
            && self.reasoning_output.is_subset_of(upper.reasoning_output)
    }
}

/// Capability ceiling for the Chat Completions Create endpoint.
///
/// Implemented fields retain current routing semantics. New fields such as audio, file, custom
/// tools, and predicted outputs only reserve definition positions and trigger `unimplemented!`
/// during registry compilation if enabled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChatCompletionsCapabilities {
    /// Whether the Chat Completions endpoint is enabled.
    pub enabled: bool,
    /// Whether Chat Completions streaming is supported.
    pub streaming: bool,
    /// Whether JSON Schema function-tool calls are supported.
    pub function_calling: bool,
    /// Whether the request wire field `parallel_tool_calls: true` is supported.
    pub parallel_tool_calls: bool,
    /// Typed `image_url` input profile, or `None` when images are unsupported.
    pub image_input: Option<ImageInputCapabilities>,
    /// Whether `response_format` or strict-function structured-output constraints are supported.
    pub structured_outputs: bool,
    /// Whether the request wire field `store: true` is supported.
    pub store: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,
    /// Whether `input_audio` input content parts are supported.
    pub audio_input: bool,
    /// Whether `file` input content parts are supported.
    pub file_input: bool,
    /// Whether audio output in `modalities` is supported.
    pub audio_output: bool,
    /// Whether `prediction` predicted outputs are supported.
    pub predicted_outputs: bool,
    /// Whether `web_search_options` is supported.
    pub web_search: bool,
    /// Whether prompt cache key/options/breakpoint semantics are supported.
    pub prompt_caching: bool,
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether token log probabilities are supported.
    pub logprobs: bool,
    /// Whether multiple choices with `n > 1` are supported.
    pub multiple_choices: bool,
}

impl ChatCompletionsCapabilities {
    /// Extracts generation capabilities shared by Chat Completions and Responses.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// Returns whether the current Chat Completions capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        // Prevent reserved fields from entering the static capability contract before request handling exists.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare currently implemented common-protocol capabilities.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
    }

    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || self.audio_input
            || self.file_input
            || self.audio_output
            || self.predicted_outputs
            || self.web_search
            || self.prompt_caching
            || self.moderation
            || self.logprobs
            || self.multiple_choices
        {
            unimplemented!("reserved Chat Completions capabilities are not implemented");
        }
    }
}

/// Capability ceiling for the Responses Create endpoint.
///
/// Other endpoints such as resource retrieve/cancel/delete are outside this structure. New Create
/// fields currently reserve type positions and trigger `unimplemented!` during registry compilation if enabled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResponsesCapabilities {
    /// Whether the Responses endpoint is enabled.
    pub enabled: bool,
    /// Whether Responses streaming is supported.
    pub streaming: bool,
    /// Whether function-tool calls are supported.
    pub function_calling: bool,
    /// Whether parallel tool calls are supported.
    pub parallel_tool_calls: bool,
    /// Typed `input_image` profile, or `None` when images are unsupported.
    pub image_input: Option<ImageInputCapabilities>,
    /// Whether structured output is supported.
    pub structured_outputs: bool,
    /// Whether persistent responses are supported.
    pub store: bool,
    /// Whether conversation state can continue with `previous_response_id`.
    pub previous_response_id: bool,
    /// Whether background responses are supported.
    pub background: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,
    /// Declared OpenAI-hosted tool kinds.
    pub hosted_tools: &'static [HostedToolKind],
    /// Whether file input items/content parts are supported.
    pub file_input: bool,
    /// Whether persistent `conversation` state is supported.
    pub conversation: bool,
    /// Whether `prompt` template references are supported.
    pub prompt_templates: bool,
    /// Whether prompt cache key/options/breakpoint semantics are supported.
    pub prompt_caching: bool,
    /// Whether `context_management` is supported.
    pub context_management: bool,
    /// Declared additional output kinds supported by `include`.
    pub include: &'static [ResponseInclude],
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether message output-text log probabilities are supported.
    pub logprobs: bool,
}

impl ResponsesCapabilities {
    /// Extracts endpoint capabilities shared by Responses and Chat.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// Returns whether the current Responses capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        // Prevent reserved fields from entering the static capability contract before request handling exists.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare implemented common capabilities and Responses state capabilities.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && (!self.previous_response_id || upper.previous_response_id)
            && (!self.background || upper.background)
    }

    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || !self.hosted_tools.is_empty()
            || self.file_input
            || self.conversation
            || self.prompt_templates
            || self.prompt_caching
            || self.context_management
            || !self.include.is_empty()
            || self.moderation
            || self.logprobs
        {
            unimplemented!("reserved Responses capabilities are not implemented");
        }
    }
}
