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

/// Input shapes accepted by the Embeddings Create request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInputForm {
    /// One non-empty string.
    String,
    /// A non-empty array of non-empty strings.
    StringArray,
    /// One non-empty token-ID array.
    TokenArray,
    /// A non-empty array of non-empty token-ID arrays.
    TokenArrayArray,
}

/// Embedding vector encodings preserved on the upstream and downstream wire.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncoding {
    /// A JSON array of floating-point components.
    #[default]
    Float,
    /// A Provider-produced base64 string preserved without local conversion.
    Base64,
}

/// Explicit domain accepted by the Embeddings `dimensions` request field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EmbeddingDimensionDomain {
    /// A closed inclusive integer range.
    Range {
        /// Smallest accepted positive dimension.
        minimum: u32,
        /// Largest accepted positive dimension.
        maximum: u32,
    },
    /// A closed ordered set of accepted positive dimensions.
    Values {
        /// Accepted dimension values in ascending order.
        values: &'static [u32],
    },
}

impl EmbeddingDimensionDomain {
    /// Returns whether the domain contains one dimension.
    pub(crate) fn contains(self, value: u32) -> bool {
        match self {
            Self::Range { minimum, maximum } => (minimum..=maximum).contains(&value),
            Self::Values { values } => values.contains(&value),
        }
    }

    /// Returns whether every value in this domain is also accepted by the upper domain.
    fn is_subset_of(self, upper: Self) -> bool {
        match (self, upper) {
            (
                Self::Range { minimum, maximum },
                Self::Range {
                    minimum: upper_minimum,
                    maximum: upper_maximum,
                },
            ) => minimum >= upper_minimum && maximum <= upper_maximum,
            (Self::Values { values }, upper) => values.iter().all(|value| upper.contains(*value)),
            (Self::Range { minimum, maximum }, Self::Values { values }) => {
                let expected_len = maximum
                    .checked_sub(minimum)
                    .and_then(|width| width.checked_add(1))
                    .and_then(|length| usize::try_from(length).ok());
                expected_len == Some(values.len())
                    && values.first() == Some(&minimum)
                    && values.last() == Some(&maximum)
                    && values.windows(2).all(|pair| pair[1] == pair[0] + 1)
            }
        }
    }
}

/// Complete Upstream API capability profile for Embeddings Create.
///
/// The profile contains only fixed request and response guarantees. It does not contain Provider,
/// endpoint, credential, or Route identity and is projected into an owned public interface by the
/// registry compiler.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmbeddingsCapabilities {
    /// Whether the Embeddings Create operation is enabled.
    pub enabled: bool,
    /// Accepted non-empty input shapes in deterministic enum order.
    pub input_forms: &'static [EmbeddingInputForm],
    /// Encoding produced when `encoding_format` is omitted.
    pub default_encoding: EmbeddingEncoding,
    /// Encodings clients may request explicitly; `None` forbids the request field.
    pub allowed_encodings: Option<&'static [EmbeddingEncoding]>,
    /// Positive vector dimension produced when `dimensions` is omitted.
    pub default_dimensions: u32,
    /// Dimension domain clients may request explicitly; `None` forbids the request field.
    pub allowed_dimensions: Option<EmbeddingDimensionDomain>,
    /// Maximum number of input items accepted by one request.
    pub max_inputs: u32,
    /// Optional maximum token count for each input item.
    pub max_tokens_per_input: Option<u32>,
    /// Optional maximum total token count for one request.
    pub max_total_tokens: Option<u32>,
    /// Input forms whose token counts can be computed locally without a tokenizer.
    pub locally_counted_input_forms: &'static [EmbeddingInputForm],
    /// Optional top-level request parameters accepted by this Native API.
    pub supported_parameters: &'static [&'static str],
}

impl EmbeddingsCapabilities {
    /// Creates a disabled Provider or Upstream API capability profile.
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            input_forms: &[],
            default_encoding: EmbeddingEncoding::Float,
            allowed_encodings: None,
            default_dimensions: 0,
            allowed_dimensions: None,
            max_inputs: 0,
            max_tokens_per_input: None,
            max_total_tokens: None,
            locally_counted_input_forms: &[],
            supported_parameters: &[],
        }
    }

    /// Validates closed sets, defaults, domains, limits, and parameter ownership.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        if !self.enabled {
            return Ok(());
        }

        // Validate non-empty deterministic input and explicit-encoding domains.
        if !is_strictly_sorted(self.input_forms) {
            return Err("input_forms must be non-empty, unique, and ordered");
        }
        if let Some(encodings) = self.allowed_encodings
            && (!is_strictly_sorted(encodings) || !encodings.contains(&self.default_encoding))
        {
            return Err(
                "allowed_encodings must be non-empty, unique, ordered, and contain the default",
            );
        }

        // Validate the positive default and any explicit dimension domain.
        if self.default_dimensions == 0 {
            return Err("default_dimensions must be greater than zero");
        }
        if let Some(domain) = self.allowed_dimensions {
            match domain {
                EmbeddingDimensionDomain::Range { minimum, maximum }
                    if minimum == 0 || minimum > maximum =>
                {
                    return Err("allowed dimension range must be positive and ordered");
                }
                EmbeddingDimensionDomain::Values { values }
                    if !is_strictly_sorted(values) || values.first() == Some(&0) =>
                {
                    return Err("allowed dimension values must be positive, unique, and ordered");
                }
                _ => {}
            }
            if !domain.contains(self.default_dimensions) {
                return Err("allowed dimensions must contain the default");
            }
        }

        // Validate positive request limits and their internal ordering.
        if self.max_inputs == 0
            || self.max_tokens_per_input == Some(0)
            || self.max_total_tokens == Some(0)
        {
            return Err("embedding limits must be greater than zero");
        }
        if self
            .max_tokens_per_input
            .is_some_and(|per_input| self.max_total_tokens.is_some_and(|total| per_input > total))
        {
            return Err("max_tokens_per_input must not exceed max_total_tokens");
        }

        // Restrict local counting to the ordered token-array subset of accepted input forms.
        if !is_sorted_unique_or_empty(self.locally_counted_input_forms)
            || self.locally_counted_input_forms.iter().any(|form| {
                !self.input_forms.contains(form)
                    || !matches!(
                        form,
                        EmbeddingInputForm::TokenArray | EmbeddingInputForm::TokenArrayArray
                    )
            })
        {
            return Err("locally counted forms must be an ordered accepted token-array subset");
        }

        // Keep the optional parameter set closed and consistent with explicit domains.
        if !is_sorted_unique_or_empty(self.supported_parameters)
            || self
                .supported_parameters
                .iter()
                .any(|parameter| !matches!(*parameter, "dimensions" | "encoding_format" | "user"))
        {
            return Err(
                "supported_parameters must be an ordered subset of the Embeddings allowlist",
            );
        }
        if self.supported_parameters.contains(&"encoding_format")
            != self.allowed_encodings.is_some()
            || self.supported_parameters.contains(&"dimensions")
                != self.allowed_dimensions.is_some()
        {
            return Err(
                "supported parameters must match the explicit encoding and dimension domains",
            );
        }
        Ok(())
    }

    /// Returns whether this API profile stays within a Provider capability ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        if !self.enabled {
            return true;
        }
        if !upper.enabled
            || self
                .input_forms
                .iter()
                .any(|form| !upper.input_forms.contains(form))
            || !encoding_supported_by(upper, self.default_encoding)
            || !dimension_supported_by(upper, self.default_dimensions)
            || !limit_is_subset(self.max_inputs, upper.max_inputs)
            || !optional_limit_is_subset(self.max_tokens_per_input, upper.max_tokens_per_input)
            || !optional_limit_is_subset(self.max_total_tokens, upper.max_total_tokens)
            || self
                .locally_counted_input_forms
                .iter()
                .any(|form| !upper.locally_counted_input_forms.contains(form))
            || self
                .supported_parameters
                .iter()
                .any(|parameter| !upper.supported_parameters.contains(parameter))
        {
            return false;
        }
        if self.allowed_encodings.is_some_and(|encodings| {
            upper.allowed_encodings.is_none_or(|upper_encodings| {
                encodings
                    .iter()
                    .any(|encoding| !upper_encodings.contains(encoding))
            })
        }) {
            return false;
        }
        match (self.allowed_dimensions, upper.allowed_dimensions) {
            (Some(domain), Some(upper_domain)) => domain.is_subset_of(upper_domain),
            (Some(_), None) => false,
            _ => true,
        }
    }
}

/// Returns whether one non-empty slice is strictly ordered and therefore duplicate-free.
fn is_strictly_sorted<T: Ord>(values: &[T]) -> bool {
    !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether a slice is empty or strictly ordered and duplicate-free.
fn is_sorted_unique_or_empty<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether the Provider ceiling can produce or explicitly accept one encoding.
fn encoding_supported_by(upper: EmbeddingsCapabilities, value: EmbeddingEncoding) -> bool {
    upper.default_encoding == value
        || upper
            .allowed_encodings
            .is_some_and(|encodings| encodings.contains(&value))
}

/// Returns whether the Provider ceiling can produce or explicitly accept one dimension.
fn dimension_supported_by(upper: EmbeddingsCapabilities, value: u32) -> bool {
    upper.default_dimensions == value
        || upper
            .allowed_dimensions
            .is_some_and(|domain| domain.contains(value))
}

/// Returns whether a required positive limit is no wider than the Provider ceiling.
fn limit_is_subset(value: u32, upper: u32) -> bool {
    upper == 0 || value <= upper
}

/// Returns whether an optional limit is no wider than a known Provider ceiling.
fn optional_limit_is_subset(value: Option<u32>, upper: Option<u32>) -> bool {
    match (value, upper) {
        (Some(value), Some(upper)) => value <= upper,
        (None, Some(_)) => false,
        (_, None) => true,
    }
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
    /// Capability ceiling for the Embeddings Create operation.
    pub embeddings: EmbeddingsCapabilities,
}
