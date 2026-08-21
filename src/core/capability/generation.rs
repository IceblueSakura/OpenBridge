//! Chat Completions and Responses capability ceilings.
//!
//! This module owns generation-only fields and the common projection used for subset checks.
//! Reserved protocol positions fail closed until their runtime semantics exist.

use serde::Serialize;

mod media;
pub use media::*;

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
/// Variants use descriptive Rust names while serialization preserves the exact Responses wire
/// path. Capability profiles carry sets of these values independently so one projection never
/// implies support for another.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ResponseInclude {
    /// `web_search_call.action.sources`.
    #[serde(rename = "web_search_call.action.sources")]
    WebSearchCallSources,
    /// `code_interpreter_call.outputs`.
    #[serde(rename = "code_interpreter_call.outputs")]
    CodeInterpreterCallOutputs,
    /// `computer_call_output.output.image_url`.
    #[serde(rename = "computer_call_output.output.image_url")]
    ComputerCallOutputImageUrl,
    /// `file_search_call.results`.
    #[serde(rename = "file_search_call.results")]
    FileSearchCallResults,
    /// `message.input_image.image_url`.
    #[serde(rename = "message.input_image.image_url")]
    InputImageImageUrl,
    /// `message.output_text.logprobs`.
    #[serde(rename = "message.output_text.logprobs")]
    OutputTextLogprobs,
    /// `reasoning.encrypted_content`.
    #[serde(rename = "reasoning.encrypted_content")]
    ReasoningEncryptedContent,
}

impl ResponseInclude {
    /// Parses one exact Responses `include` wire value.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "web_search_call.action.sources" => Some(Self::WebSearchCallSources),
            "code_interpreter_call.outputs" => Some(Self::CodeInterpreterCallOutputs),
            "computer_call_output.output.image_url" => Some(Self::ComputerCallOutputImageUrl),
            "file_search_call.results" => Some(Self::FileSearchCallResults),
            "message.input_image.image_url" => Some(Self::InputImageImageUrl),
            "message.output_text.logprobs" => Some(Self::OutputTextLogprobs),
            "reasoning.encrypted_content" => Some(Self::ReasoningEncryptedContent),
            _ => None,
        }
    }

    /// Returns the exact Responses `include` wire value.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::WebSearchCallSources => "web_search_call.action.sources",
            Self::CodeInterpreterCallOutputs => "code_interpreter_call.outputs",
            Self::ComputerCallOutputImageUrl => "computer_call_output.output.image_url",
            Self::FileSearchCallResults => "file_search_call.results",
            Self::InputImageImageUrl => "message.input_image.image_url",
            Self::OutputTextLogprobs => "message.output_text.logprobs",
            Self::ReasoningEncryptedContent => "reasoning.encrypted_content",
        }
    }
}

/// Function-tool selection modes accepted by a generation operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
    /// Prevents the model from calling tools.
    None,
    /// Lets the model decide whether to call a tool.
    Auto,
    /// Requires the model to call at least one tool.
    Required,
    /// Selects a named function.
    Named,
}

/// All function-tool choice modes currently represented by the gateway contract.
pub const ALL_TOOL_CHOICE_MODES: &[ToolChoiceMode] = &[
    ToolChoiceMode::None,
    ToolChoiceMode::Auto,
    ToolChoiceMode::Required,
    ToolChoiceMode::Named,
];

/// Structured-output modes accepted by a generation operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputMode {
    /// JSON object output constraint.
    JsonObject,
    /// JSON Schema output constraint.
    JsonSchema,
}

const JSON_OBJECT_MODE: &[StructuredOutputMode] = &[StructuredOutputMode::JsonObject];
const JSON_SCHEMA_MODE: &[StructuredOutputMode] = &[StructuredOutputMode::JsonSchema];
const JSON_OBJECT_AND_SCHEMA_MODES: &[StructuredOutputMode] = &[
    StructuredOutputMode::JsonObject,
    StructuredOutputMode::JsonSchema,
];

/// Strictness accepted by a JSON Schema structured-output capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonSchemaSupport {
    /// Accepts non-strict JSON Schema constraints only.
    NonStrictOnly,
    /// Accepts both non-strict JSON Schema and `strict: true` constraints.
    StrictSupported,
}

impl JsonSchemaSupport {
    /// Returns whether this strictness stays within another JSON Schema ceiling.
    const fn is_subset_of(self, upper: Self) -> bool {
        matches!(self, Self::NonStrictOnly) || matches!(upper, Self::StrictSupported)
    }

    /// Returns the strictness guaranteed by both profiles.
    const fn intersection(self, other: Self) -> Self {
        if matches!(self, Self::StrictSupported) && matches!(other, Self::StrictSupported) {
            Self::StrictSupported
        } else {
            Self::NonStrictOnly
        }
    }
}

/// Fine-grained function-tool capability profile.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FunctionToolCapabilities {
    /// Function-tool selection modes accepted by the operation.
    pub choice_modes: &'static [ToolChoiceMode],
    /// Whether `parallel_tool_calls: true` is accepted with function tools.
    pub parallel_calls: bool,
    /// Whether strict JSON Schema function parameters are accepted.
    pub strict_schema: bool,
}

impl FunctionToolCapabilities {
    /// Returns whether this profile is no broader than another function-tool profile.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        self.choice_modes
            .iter()
            .all(|mode| upper.choice_modes.contains(mode))
            && (!self.parallel_calls || upper.parallel_calls)
            && (!self.strict_schema || upper.strict_schema)
    }
}

/// Closed non-empty structured-output capability profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredOutputProfile {
    /// Accepts the JSON Object response format only.
    JsonObject,
    /// Accepts JSON Schema with the declared strictness support.
    JsonSchema(JsonSchemaSupport),
    /// Accepts JSON Object and JSON Schema with the declared strictness support.
    JsonObjectAndJsonSchema(JsonSchemaSupport),
}

impl StructuredOutputProfile {
    /// Returns supported modes in stable JSON Object then JSON Schema order.
    pub const fn modes(self) -> &'static [StructuredOutputMode] {
        match self {
            Self::JsonObject => JSON_OBJECT_MODE,
            Self::JsonSchema(_) => JSON_SCHEMA_MODE,
            Self::JsonObjectAndJsonSchema(_) => JSON_OBJECT_AND_SCHEMA_MODES,
        }
    }

    /// Returns whether this profile supports the requested structured-output mode.
    pub const fn supports(self, mode: StructuredOutputMode) -> bool {
        matches!(
            (self, mode),
            (
                Self::JsonObject | Self::JsonObjectAndJsonSchema(_),
                StructuredOutputMode::JsonObject
            ) | (
                Self::JsonSchema(_) | Self::JsonObjectAndJsonSchema(_),
                StructuredOutputMode::JsonSchema
            )
        )
    }

    /// Returns whether this profile accepts `strict: true` JSON Schema constraints.
    pub const fn supports_strict_schema(self) -> bool {
        matches!(
            self,
            Self::JsonSchema(JsonSchemaSupport::StrictSupported)
                | Self::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported)
        )
    }

    /// Returns whether this profile is no broader than another structured-output profile.
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        match (self, upper) {
            (Self::JsonObject, Self::JsonObject | Self::JsonObjectAndJsonSchema(_)) => true,
            (Self::JsonSchema(value), Self::JsonSchema(upper))
            | (Self::JsonSchema(value), Self::JsonObjectAndJsonSchema(upper)) => {
                value.is_subset_of(upper)
            }
            (Self::JsonObjectAndJsonSchema(value), Self::JsonObjectAndJsonSchema(upper)) => {
                value.is_subset_of(upper)
            }
            _ => false,
        }
    }

    /// Returns the closed profile guaranteed by both operands, or `None` for disjoint modes.
    pub(crate) const fn intersection(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::JsonObject, Self::JsonObject)
            | (Self::JsonObject, Self::JsonObjectAndJsonSchema(_))
            | (Self::JsonObjectAndJsonSchema(_), Self::JsonObject) => Some(Self::JsonObject),
            (Self::JsonSchema(value), Self::JsonSchema(other))
            | (Self::JsonSchema(value), Self::JsonObjectAndJsonSchema(other))
            | (Self::JsonObjectAndJsonSchema(value), Self::JsonSchema(other)) => {
                Some(Self::JsonSchema(value.intersection(other)))
            }
            (Self::JsonObjectAndJsonSchema(value), Self::JsonObjectAndJsonSchema(other)) => {
                Some(Self::JsonObjectAndJsonSchema(value.intersection(other)))
            }
            (Self::JsonObject, Self::JsonSchema(_)) | (Self::JsonSchema(_), Self::JsonObject) => {
                None
            }
        }
    }
}

/// Shared generation-capability projection for Chat Completions and Responses.
///
/// This value is used only for common-protocol subset checks; static registrations must use
/// [`ChatCompletionsCapabilities`] or [`ResponsesCapabilities`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenerationCapabilities {
    /// Whether incremental results can be returned over SSE.
    pub(crate) streaming: bool,
    /// Fine-grained function-tool capability profile.
    pub(crate) function_tools: Option<FunctionToolCapabilities>,
    /// Typed image input profile, or `None` when images are unsupported.
    pub(crate) image_input: Option<ImageInputCapabilities>,
    /// Fine-grained structured-output capability profile.
    pub(crate) structured_outputs: Option<StructuredOutputProfile>,
    /// Whether the request wire field `store: true` is supported.
    pub(crate) store: bool,
    /// Observable type of upstream reasoning output.
    pub(crate) reasoning_output: ReasoningOutput,
}

impl GenerationCapabilities {
    /// Returns whether the current capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        (!self.streaming || upper.streaming)
            && optional_function_tool_capabilities_is_subset_of(
                self.function_tools,
                upper.function_tools,
            )
            && image_input_is_subset_of(self.image_input, upper.image_input)
            && optional_structured_output_profile_is_subset_of(
                self.structured_outputs,
                upper.structured_outputs,
            )
            && (!self.store || upper.store)
            && self.reasoning_output.is_subset_of(upper.reasoning_output)
    }
}

/// Shared Chat Completions common fields parameterized by the audio contract layer.
///
/// Provider definitions use [`ProviderChatCompletionsCapabilities`], while concrete Target APIs
/// use [`ChatCompletionsCapabilities`]. The generic envelope prevents common fields from drifting
/// while the type parameter prevents a Provider ceiling from entering executable Route state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChatCompletionsProfile<A> {
    /// Whether Chat Completions streaming is supported.
    pub streaming: bool,
    /// Whether streaming can provide the final usage-only Chat chunk requested by `stream_options`.
    pub stream_usage: bool,
    /// Fine-grained function-tool capability profile, or `None` when tools are unsupported.
    pub function_tools: Option<FunctionToolCapabilities>,
    /// Complete operation-specific media profile.
    pub media: ChatMediaProfile<A>,
    /// Fine-grained structured-output profile, or `None` when structured output is unsupported.
    pub structured_outputs: Option<StructuredOutputProfile>,
    /// Whether the request wire field `store: true` is supported.
    pub store: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,

    /// Whether `prediction` predicted outputs are supported.
    pub predicted_outputs: bool,
    /// Whether `web_search_options` is supported.
    pub web_search: bool,
    /// Whether the request wire field `prompt_cache_key` is forwarded exactly.
    pub prompt_cache_key: bool,
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether token log probabilities are supported.
    pub logprobs: bool,
    /// Whether multiple choices with `n > 1` are supported.
    pub multiple_choices: bool,
}

/// Provider-wide Chat Completions ceiling with an optional non-empty audio task set.
pub type ProviderChatCompletionsCapabilities = ChatCompletionsProfile<Option<ProviderAudioCeiling>>;

/// Concrete Target Chat Completions profile with at most one executable audio task.
pub type ChatCompletionsCapabilities = ChatCompletionsProfile<Option<ExecutableAudioProfile>>;

impl<A: Copy> ChatCompletionsProfile<A> {
    /// Extracts generation capabilities shared by Chat Completions and Responses.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.media.image,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || self.predicted_outputs
            || self.web_search
            || self.moderation
            || self.logprobs
            || self.multiple_choices
        {
            unimplemented!("reserved Chat Completions capabilities are not implemented");
        }
        if self
            .function_tools
            .is_some_and(|profile| profile.choice_modes.is_empty())
        {
            panic!("invalid Chat Completions function-tool capability profile");
        }
    }
}

impl ChatCompletionsProfile<Option<ProviderAudioCeiling>> {
    /// Projects non-media fields and requires one complete executable Target media profile.
    pub const fn to_executable(
        self,
        media: ChatMediaProfile<Option<ExecutableAudioProfile>>,
    ) -> ChatCompletionsCapabilities {
        ChatCompletionsProfile {
            streaming: self.streaming,
            stream_usage: self.stream_usage,
            function_tools: self.function_tools,
            media,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
            custom_tool_calling: self.custom_tool_calling,
            predicted_outputs: self.predicted_outputs,
            web_search: self.web_search,
            prompt_cache_key: self.prompt_cache_key,
            moderation: self.moderation,
            logprobs: self.logprobs,
            multiple_choices: self.multiple_choices,
        }
    }
}

impl ChatCompletionsProfile<Option<ExecutableAudioProfile>> {
    /// Returns whether this concrete Target profile stays within the Provider Chat ceiling.
    pub(crate) fn is_subset_of(self, upper: ProviderChatCompletionsCapabilities) -> bool {
        // Reject reserved fields before comparing trusted static contracts.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare common fields and require the complete executable media profile to fit the ceiling.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && (!self.stream_usage || upper.stream_usage)
            && self.media.is_subset_of(upper.media)
            && (!self.prompt_cache_key || upper.prompt_cache_key)
    }

    /// Returns whether the typed audio profile contains any input capability.
    pub const fn has_audio_input(self) -> bool {
        match self.media.audio {
            Some(audio) => audio.has_input(),
            None => false,
        }
    }

    /// Returns whether the typed audio profile contains generated-audio output.
    pub const fn has_audio_output(self) -> bool {
        match self.media.audio {
            Some(audio) => audio.has_output(),
            None => false,
        }
    }
}

fn optional_function_tool_capabilities_is_subset_of(
    value: Option<FunctionToolCapabilities>,
    upper: Option<FunctionToolCapabilities>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

fn optional_structured_output_profile_is_subset_of(
    value: Option<StructuredOutputProfile>,
    upper: Option<StructuredOutputProfile>,
) -> bool {
    match (value, upper) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(value), Some(upper)) => value.is_subset_of(upper),
    }
}

/// Whether a Responses API accepts persistent response storage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StorageSupport {
    /// The executable API does not accept `store: true`.
    #[default]
    Unsupported,
    /// The executable API accepts `store: true`.
    Supported,
}

impl StorageSupport {
    /// Returns whether persistent response storage is supported.
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// Closed Target-affinity contract for one executable Responses API.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResponsesAffinity {
    /// Requests carry no Provider state that binds execution to this Target.
    #[default]
    Unbound,
    /// Provider state is Target-bound, but continuation by response ID is unsupported.
    TargetBound,
    /// Provider-issued response IDs bind continuation to this Target and credential context.
    TargetBoundContinuation,
}

/// Executable storage and affinity state owned by one concrete Responses Target.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutableResponsesState {
    storage: StorageSupport,
    affinity: ResponsesAffinity,
}

impl ExecutableResponsesState {
    /// Creates one executable Responses state from independent storage and closed affinity facts.
    pub const fn new(storage: StorageSupport, affinity: ResponsesAffinity) -> Self {
        Self { storage, affinity }
    }

    /// Returns the persistent-storage support carried by this executable state.
    pub const fn storage(self) -> StorageSupport {
        self.storage
    }

    /// Returns the closed Target-affinity variant carried by this executable state.
    pub const fn affinity(self) -> ResponsesAffinity {
        self.affinity
    }

    /// Returns whether this executable Responses API accepts `store: true`.
    pub const fn supports_store(self) -> bool {
        self.storage.is_supported()
    }

    /// Returns whether this executable Responses API accepts `previous_response_id`.
    pub const fn supports_previous_response_id(self) -> bool {
        matches!(self.affinity, ResponsesAffinity::TargetBoundContinuation)
    }

    /// Returns whether Provider state is bound to the concrete Upstream Target.
    pub const fn is_target_bound(self) -> bool {
        matches!(
            self.affinity,
            ResponsesAffinity::TargetBound | ResponsesAffinity::TargetBoundContinuation
        )
    }

    /// Returns whether continuation safety requires one enabled credential-pool member.
    pub const fn requires_single_credential_member(self) -> bool {
        self.supports_previous_response_id()
    }

    /// Returns whether this executable state stays within one Provider state ceiling.
    const fn is_subset_of(self, upper: ProviderResponsesStateCeiling) -> bool {
        (!self.supports_store() || upper.supports_store())
            && (!self.supports_previous_response_id() || upper.supports_previous_response_id())
    }
}

/// Provider-wide upper bound for the two independent Responses state capabilities.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProviderResponsesStateCeiling {
    /// Neither storage nor continuation is supported.
    #[default]
    Stateless,
    /// Persistent response storage is supported without continuation.
    Storage,
    /// Response-ID continuation is supported without persistent storage.
    Continuation,
    /// Both persistent storage and response-ID continuation are supported.
    StorageAndContinuation,
}

impl ProviderResponsesStateCeiling {
    /// Returns whether the Provider ceiling permits `store: true`.
    pub const fn supports_store(self) -> bool {
        matches!(self, Self::Storage | Self::StorageAndContinuation)
    }

    /// Returns whether the Provider ceiling permits response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        matches!(self, Self::Continuation | Self::StorageAndContinuation)
    }
}

/// Shared Responses Create fields parameterized by the state contract layer.
///
/// Provider definitions use [`ProviderResponsesCapabilities`], while concrete Target APIs use
/// [`ResponsesCapabilities`]. Other endpoints such as resource retrieve/cancel/delete remain
/// outside this structure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResponsesProfile<S> {
    /// Whether Responses streaming is supported.
    pub streaming: bool,
    /// Whether a successful streaming terminal carries complete token usage.
    pub terminal_usage: bool,
    /// Fine-grained function-tool capability profile, or `None` when tools are unsupported.
    pub function_tools: Option<FunctionToolCapabilities>,
    /// Complete operation-specific media profile.
    pub media: ResponsesMediaProfile,
    /// Fine-grained structured-output profile, or `None` when structured output is unsupported.
    pub structured_outputs: Option<StructuredOutputProfile>,
    /// Layer-specific Provider ceiling or executable Responses state.
    pub state: S,
    /// Whether background responses are supported.
    pub background: bool,
    /// Observable type of upstream reasoning output.
    pub reasoning_output: ReasoningOutput,
    /// Whether `type: "custom"` tools are supported.
    pub custom_tool_calling: bool,
    /// Declared OpenAI-hosted tool kinds.
    pub hosted_tools: &'static [HostedToolKind],

    /// Whether persistent `conversation` state is supported.
    pub conversation: bool,
    /// Whether `prompt` template references are supported.
    pub prompt_templates: bool,
    /// Whether the request wire field `prompt_cache_key` is forwarded exactly.
    pub prompt_cache_key: bool,
    /// Whether `context_management` is supported.
    pub context_management: bool,
    /// Declared additional output kinds supported by `include`.
    pub include: &'static [ResponseInclude],
    /// Whether request-level moderation configuration is supported.
    pub moderation: bool,
    /// Whether message output-text log probabilities are supported.
    pub logprobs: bool,
}

/// Provider-wide Responses ceiling without Target ownership state.
pub type ProviderResponsesCapabilities = ResponsesProfile<ProviderResponsesStateCeiling>;

/// Concrete executable Responses profile with one closed Target state.
pub type ResponsesCapabilities = ResponsesProfile<ExecutableResponsesState>;

impl ProviderResponsesCapabilities {
    /// Returns the complete Provider Responses state ceiling.
    pub const fn state_ceiling(self) -> ProviderResponsesStateCeiling {
        self.state
    }

    /// Returns whether the Provider ceiling permits persistent response storage.
    pub const fn supports_store(self) -> bool {
        self.state.supports_store()
    }

    /// Returns whether the Provider ceiling permits response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        self.state.supports_previous_response_id()
    }

    /// Projects non-media fields and requires one complete executable Target media profile.
    pub const fn to_executable(
        self,
        state: ExecutableResponsesState,
        media: ResponsesMediaProfile,
    ) -> ResponsesCapabilities {
        ResponsesProfile {
            streaming: self.streaming,
            terminal_usage: self.terminal_usage,
            function_tools: self.function_tools,
            media,
            structured_outputs: self.structured_outputs,
            state,
            background: self.background,
            reasoning_output: self.reasoning_output,
            custom_tool_calling: self.custom_tool_calling,
            hosted_tools: self.hosted_tools,
            conversation: self.conversation,
            prompt_templates: self.prompt_templates,
            prompt_cache_key: self.prompt_cache_key,
            context_management: self.context_management,
            include: self.include,
            moderation: self.moderation,
            logprobs: self.logprobs,
        }
    }

    /// Extracts common generation capabilities from the Provider ceiling.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.media.image,
            structured_outputs: self.structured_outputs,
            store: self.state.supports_store(),
            reasoning_output: self.reasoning_output,
        }
    }
}

impl ResponsesCapabilities {
    /// Extracts endpoint capabilities shared by Responses and Chat.
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            streaming: self.streaming,
            function_tools: self.function_tools,
            image_input: self.media.image,
            structured_outputs: self.structured_outputs,
            store: self.state.supports_store(),
            reasoning_output: self.reasoning_output,
        }
    }

    /// Returns the complete executable Responses state.
    pub const fn state(self) -> ExecutableResponsesState {
        self.state
    }

    /// Returns whether this executable API accepts persistent response storage.
    pub const fn supports_store(self) -> bool {
        self.state.supports_store()
    }

    /// Returns whether this executable API accepts response-ID continuation.
    pub const fn supports_previous_response_id(self) -> bool {
        self.state.supports_previous_response_id()
    }

    /// Returns whether Provider state is bound to the concrete Upstream Target.
    pub const fn is_target_bound(self) -> bool {
        self.state.is_target_bound()
    }

    /// Returns whether continuation safety requires one enabled credential-pool member.
    pub const fn requires_single_credential_member(self) -> bool {
        self.state.requires_single_credential_member()
    }

    /// Returns whether the current Responses capabilities stay within the given ceiling.
    pub(crate) fn is_subset_of(self, upper: ProviderResponsesCapabilities) -> bool {
        // Prevent reserved fields from entering the static capability contract before request handling exists.
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // Compare implemented common capabilities and Responses state capabilities.
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && (!self.terminal_usage || upper.terminal_usage)
            && self.state.is_subset_of(upper.state)
            && (!self.background || upper.background)
            && self.media.is_subset_of(upper.media)
            && (!self.prompt_cache_key || upper.prompt_cache_key)
            && response_includes_are_subset_of(self.include, upper.include)
    }
}

impl<S: Copy> ResponsesProfile<S> {
    /// Stops compilation when reserved fields are registered so they cannot become false runtime capabilities.
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || !self.hosted_tools.is_empty()
            || self.conversation
            || self.prompt_templates
            || self.context_management
            || self.moderation
            || self.logprobs
        {
            unimplemented!("reserved Responses capabilities are not implemented");
        }
        if self
            .function_tools
            .is_some_and(|profile| profile.choice_modes.is_empty())
        {
            panic!("invalid Responses function-tool capability profile");
        }
    }
}

/// Validates duplicate-free `include` sets and checks every concrete value against the ceiling.
fn response_includes_are_subset_of(values: &[ResponseInclude], upper: &[ResponseInclude]) -> bool {
    // Reject duplicate values in either trusted static capability set.
    let unique = |items: &[ResponseInclude]| {
        items
            .iter()
            .enumerate()
            .all(|(index, item)| !items[index + 1..].contains(item))
    };

    // Require every executable projection to be explicitly present in the Provider ceiling.
    unique(values) && unique(upper) && values.iter().all(|value| upper.contains(value))
}
