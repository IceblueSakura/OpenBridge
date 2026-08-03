//! Provider-independent capability ceilings and protocol-specific value objects.
//!
//! Capabilities may only be narrowed from the Provider contract during registry construction; subset
//! checks prevent static definitions from expanding. The request path uses the precompiled Public
//! Model contract instead of comparing capabilities Route by Route.

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
    /// Web search tool。
    WebSearch,
    /// File search tool。
    FileSearch,
    /// Code Interpreter tool。
    CodeInterpreter,
    /// Computer Use tool。
    ComputerUse,
    /// Image generation tool。
    ImageGeneration,
    /// Remote MCP tool。
    Mcp,
    /// Hosted shell tool。
    Shell,
    /// Apply patch tool。
    ApplyPatch,
    /// Tool search tool。
    ToolSearch,
    /// Skills tool。
    Skills,
    /// Programmatic Tool Calling tool。
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

/// Shared generation-capability projection for Chat Completions and Responses.
///
/// This value is used only for request analysis and common-protocol subset checks; static
/// registrations must use [`ChatCompletionsCapabilities`] or [`ResponsesCapabilities`].
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
    /// Whether image input content parts are supported.
    pub(crate) image_input: bool,
    /// Whether structured-output constraints are supported.
    pub(crate) structured_outputs: bool,
    /// Whether the request wire field `store: true` is supported.
    pub(crate) store: bool,
    /// Observable type of upstream reasoning output.
    pub(crate) reasoning_output: ReasoningOutput,
}

impl GenerationCapabilities {
    /// Returns whether the current capabilities stay within the given ceiling.
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        (!self.enabled || upper.enabled)
            && (!self.streaming || upper.streaming)
            && (!self.function_calling || upper.function_calling)
            && (!self.parallel_tool_calls || upper.parallel_tool_calls)
            && (!self.image_input || upper.image_input)
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
    /// Whether `image_url` input content parts are supported.
    pub image_input: bool,
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
    /// Whether image input is supported.
    pub image_input: bool,
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

/// Protocol-specific capability ceilings for a Provider contract.
///
/// An Upstream API may disable capabilities supported by the Provider contract but cannot enable
/// unimplemented capabilities. The request path uses a separately precompiled Public Model
/// contract. Chat Completions and Responses are modeled separately so observations from one
/// endpoint are not incorrectly applied to the other.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApiCapabilities {
    /// Capability ceiling for the Chat Completions endpoint.
    pub chat_completions: ChatCompletionsCapabilities,
    /// Capability ceiling for the Responses endpoint.
    pub responses: ResponsesCapabilities,
}
