//! Fixed downstream Public Model contracts and safe serialization models.
//!
//! This module compiles client-visible model facts together with private operation execution
//! interfaces. Each execution interface pairs one conservative capability contract with its fixed
//! static Route candidates, while serialized responses retain no Provider, Target, Route,
//! upstream-model, or credential boundary.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::core::{
    ApiProtocol, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
    EmbeddingsCapabilities, OperationKind, ReasoningOutput,
};

use super::{
    InputModality, ModelContextLength, ModelLifecycle, ModelLifecycleStatus, OutputModality,
    PublicModelConfig, ReasoningLevel, ReasoningSupport, RegistryError, Route, RouteMode,
    UpstreamApi, UpstreamApiCapabilities,
};

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

    /// Takes the minimum known limit across all Routes; any unknown value remains unknown.
    fn intersection<'a>(values: impl Iterator<Item = &'a Self> + Clone) -> Self {
        Self {
            max_context_tokens: intersect_optional_limit(
                values.clone().map(|value| value.max_context_tokens),
            ),
            max_input_tokens: intersect_optional_limit(
                values.clone().map(|value| value.max_input_tokens),
            ),
            max_output_tokens: intersect_optional_limit(
                values.map(|value| value.max_output_tokens),
            ),
        }
    }
}

/// Confirmed Public Model input and output modalities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelModalities {
    input: Vec<InputModality>,
    output: Vec<OutputModality>,
}

impl ModelModalities {
    /// Computes the stable set intersection of multiple Route contracts.
    fn intersection<'a>(values: impl Iterator<Item = &'a Self> + Clone) -> Self {
        Self {
            input: intersect_sets(values.clone().map(|value| value.input.as_slice())),
            output: intersect_sets(values.map(|value| value.output.as_slice())),
        }
    }
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
    /// JSON-schema function tool。
    Function,
}

/// Function-tool selection mode.
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

/// Structured-output capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuredOutputCapabilities {
    support: SupportState,
    modes: Vec<StructuredOutputMode>,
    strict_schema: SupportState,
}

/// Structured-output modes currently modeled by OpenBridge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputMode {
    /// JSON object output constraint.
    JsonObject,
    /// JSON Schema output constraint.
    JsonSchema,
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

/// Unique, fixed capability contract for one protocol interface, used directly by request preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaceCapabilities {
    context_window: ContextWindow,
    modalities: ModelModalities,
    supported_parameters: Vec<String>,
    streaming: SupportState,
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
    /// Builds the public contract from one validated static API profile.
    fn from_capabilities(capabilities: EmbeddingsCapabilities) -> Self {
        Self {
            input_forms: capabilities.input_forms.to_vec(),
            encoding: EmbeddingEncodingCapabilities {
                default: capabilities.default_encoding,
                allowed: capabilities.allowed_encodings.map(<[_]>::to_vec),
            },
            dimensions: EmbeddingDimensionCapabilities {
                default: capabilities.default_dimensions,
                allowed: capabilities.allowed_dimensions,
            },
            limits: EmbeddingLimits {
                max_inputs: capabilities.max_inputs,
                max_tokens_per_input: capabilities.max_tokens_per_input,
                max_total_tokens: capabilities.max_total_tokens,
                locally_counted_input_forms: capabilities.locally_counted_input_forms.to_vec(),
            },
            supported_parameters: capabilities
                .supported_parameters
                .iter()
                .map(|parameter| (*parameter).to_owned())
                .collect(),
        }
    }

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
    /// Returns whether the interface guarantees streaming support.
    pub(crate) const fn supports_streaming(&self) -> bool {
        self.streaming.is_supported()
    }

    /// Returns whether the interface guarantees function-tool support.
    pub(crate) const fn supports_function_calling(&self) -> bool {
        self.tools.support.is_supported()
    }

    /// Returns whether the interface guarantees parallel function calls.
    pub(crate) const fn supports_parallel_tool_calls(&self) -> bool {
        self.tools.parallel_calls.is_supported()
    }

    /// Returns whether the interface guarantees image input.
    pub(crate) fn supports_image_input(&self) -> bool {
        self.modalities.input.contains(&InputModality::Image)
    }

    /// Returns whether the interface guarantees structured output.
    pub(crate) const fn supports_structured_outputs(&self) -> bool {
        self.structured_outputs.support.is_supported()
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

/// Private execution candidate compiled from one statically executable Route.
///
/// This type is never serialized or exposed by a downstream API. It freezes the Route identity and
/// the planning facts needed to construct a Native request or `BridgePlan` without re-resolving the
/// Public Model's configured Route list during a request.
#[derive(Clone, Debug)]
pub(crate) struct RouteExecutionCandidate {
    route_id: String,
    upstream_target_id: String,
    downstream_operation: OperationKind,
    upstream_operation: OperationKind,
    mode: RouteMode,
    upstream_model: String,
    reasoning_output: ReasoningOutput,
}

impl RouteExecutionCandidate {
    /// Returns the configured Route ID retained for forwarding diagnostics and attempt attribution.
    pub(crate) fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the prevalidated Upstream Target ID used by forwarding.
    pub(crate) fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the downstream operation represented by this interface candidate.
    pub(crate) const fn downstream_operation(&self) -> OperationKind {
        self.downstream_operation
    }

    /// Returns the upstream operation represented by this interface candidate.
    pub(crate) const fn upstream_operation(&self) -> OperationKind {
        self.upstream_operation
    }

    /// Returns the downstream generation protocol guaranteed by a generation execution interface.
    pub(crate) fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_operation
            .api_protocol()
            .expect("generation candidates have a downstream API protocol")
    }

    /// Returns the upstream generation protocol guaranteed by a generation execution interface.
    pub(crate) fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_operation
            .api_protocol()
            .expect("generation candidates have an upstream API protocol")
    }

    /// Returns whether forwarding is Native or must use the restricted protocol bridge.
    pub(crate) const fn mode(&self) -> RouteMode {
        self.mode
    }

    /// Returns the trusted model ID used only while rendering a bridged upstream request.
    pub(crate) fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the upstream reasoning-output classification required by bridge preparation.
    pub(crate) const fn reasoning_output(&self) -> ReasoningOutput {
        self.reasoning_output
    }
}

/// One immutable executable interface shared by request preflight and Route planning.
#[derive(Debug)]
pub(crate) struct ModelExecutionInterface {
    generation_capabilities: Option<ModelInterfaceCapabilities>,
    embedding_capabilities: Option<EmbeddingInterfaceCapabilities>,
    candidates: Vec<RouteExecutionCandidate>,
}

impl ModelExecutionInterface {
    /// Returns the fixed capability contract derived from exactly these static candidates.
    pub(crate) const fn capabilities(&self) -> &ModelInterfaceCapabilities {
        self.generation_capabilities
            .as_ref()
            .expect("generation preflight selected a generation execution interface")
    }

    /// Returns the fixed Embeddings contract derived from this interface's single Native candidate.
    pub(crate) const fn embedding_capabilities(&self) -> Option<&EmbeddingInterfaceCapabilities> {
        self.embedding_capabilities.as_ref()
    }

    /// Returns static candidates in their configured priority order.
    pub(crate) fn candidates(&self) -> &[RouteExecutionCandidate] {
        &self.candidates
    }
}

/// Operation execution interfaces compiled from one Public Model's static Route bindings.
#[derive(Debug)]
struct ModelExecutionInterfaces {
    chat_completions: Option<ModelExecutionInterface>,
    responses: Option<ModelExecutionInterface>,
    embeddings: Option<ModelExecutionInterface>,
}

impl ModelExecutionInterfaces {
    /// Returns the interface that owns both preflight capabilities and planning candidates.
    const fn for_operation(&self, operation: OperationKind) -> Option<&ModelExecutionInterface> {
        match operation {
            OperationKind::ChatCompletions => self.chat_completions.as_ref(),
            OperationKind::Responses => self.responses.as_ref(),
            OperationKind::EmbeddingsCreate => self.embeddings.as_ref(),
        }
    }

    /// Returns whether this Public Model has any statically executable downstream protocol.
    const fn is_available(&self) -> bool {
        self.chat_completions.is_some() || self.responses.is_some() || self.embeddings.is_some()
    }

    /// Projects capability copies into the safe Models response without candidate topology.
    fn public_projection(&self) -> ModelInterfaces {
        ModelInterfaces {
            chat_completions: self
                .chat_completions
                .as_ref()
                .and_then(|interface| interface.generation_capabilities.clone()),
            responses: self
                .responses
                .as_ref()
                .and_then(|interface| interface.generation_capabilities.clone()),
            embeddings: self
                .embeddings
                .as_ref()
                .and_then(|interface| interface.embedding_capabilities.clone()),
        }
    }
}

/// Resolved downstream Public Model, fixed information object, diagnostic Route IDs, and execution interfaces.
#[derive(Debug)]
pub struct PublicModel {
    pub(super) routes: Vec<String>,
    execution_interfaces: ModelExecutionInterfaces,
    pub(super) info: PublicModelInfo,
}

impl PublicModel {
    /// Returns configured Route IDs ordered by priority for diagnostics and tests.
    ///
    /// Request planning does not read this raw list; it consumes the protocol-specific static
    /// candidate set in [`Self::execution_interface`].
    pub fn routes(&self) -> &[String] {
        &self.routes
    }

    /// Returns complete safe model information for the extension interface.
    pub fn info(&self) -> &PublicModelInfo {
        &self.info
    }

    /// Returns the standard OpenAI Models resource projection.
    pub fn standard(&self) -> &StandardModel {
        self.info.standard()
    }

    /// Returns the precompiled interface used by both request preflight and Route planning.
    pub(crate) const fn execution_interface(
        &self,
        operation: OperationKind,
    ) -> Option<&ModelExecutionInterface> {
        self.execution_interfaces.for_operation(operation)
    }

    /// Returns whether the model remains visible to clients and has at least one executable interface.
    pub(crate) fn is_available(&self) -> bool {
        self.info.lifecycle.status != ModelLifecycleStatus::Retired
            && self.execution_interfaces.is_available()
    }
}

/// Validated Route binding used to compile one Public Model's static execution interfaces.
pub(super) struct PublicRouteBinding<'a> {
    pub(super) route_id: String,
    pub(super) route: &'a Route,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) target_enabled: bool,
}

/// Compiles a fixed Public Model without deployment details from the complete Route set.
///
/// Returns an error when an Embeddings interface cannot fit one worst-case valid result within
/// the configured JSON response budget.
pub(super) fn compile_public_model(
    config: PublicModelConfig,
    bindings: &[PublicRouteBinding<'_>],
    max_json_response_body_bytes: usize,
) -> Result<PublicModel, RegistryError> {
    // Compile static eligibility once so every protocol contract and request plan shares the same candidates.
    let mut candidates = bindings
        .iter()
        .filter_map(PrecompiledRouteCandidate::from_binding)
        .collect::<Vec<_>>();

    // Narrow an Embeddings batch contract to what one bounded validated response can always contain.
    constrain_embedding_response_budget(&config.id, max_json_response_body_bytes, &mut candidates)?;

    // Derive protocol contracts and model facts exclusively from the compiled static candidates.
    let contributions = candidates
        .iter()
        .map(|candidate| candidate.contribution.clone())
        .collect::<Vec<_>>();
    let execution_interfaces = compile_execution_interfaces(&candidates);
    let capabilities = aggregate_model_capabilities(&contributions);
    let description = config.description.or_else(|| {
        intersect_optional_string(
            contributions
                .iter()
                .map(|contribution| contribution.model_description.as_deref()),
        )
    });

    // Freeze the standard projection and safe extension object without exposing execution topology.
    let info = PublicModelInfo {
        schema_version: MODEL_INFO_SCHEMA_VERSION,
        standard: StandardModel {
            id: config.id,
            object: "model",
            created: config.created,
            owned_by: "openbridge",
        },
        name: config.display_name,
        description,
        lifecycle: config.lifecycle,
        capabilities,
        interfaces: execution_interfaces.public_projection(),
    };
    Ok(PublicModel {
        routes: config.routes,
        execution_interfaces,
        info,
    })
}

/// Narrows the single Embeddings candidate using checked worst-case JSON serialization bounds.
fn constrain_embedding_response_budget(
    public_model: &str,
    response_budget: usize,
    candidates: &mut [PrecompiledRouteCandidate],
) -> Result<(), RegistryError> {
    // Locate the one compiler-approved Embeddings candidate without affecting generation interfaces.
    let Some(candidate) = candidates.iter_mut().find(|candidate| {
        candidate.execution.downstream_operation() == OperationKind::EmbeddingsCreate
    }) else {
        return Ok(());
    };
    let Some(mut capabilities) = candidate.contribution.embedding_capabilities else {
        return Err(RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        });
    };

    // Compute the largest public dimension and the worst permitted vector encoding.
    let maximum_dimensions = maximum_embedding_dimensions(capabilities)
        .and_then(|dimensions| usize::try_from(dimensions).ok())
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let vector_bytes = worst_case_embedding_vector_bytes(capabilities, maximum_dimensions)
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;

    // Bound the raw upstream and projected downstream envelopes, then derive the safe batch count.
    let envelope_bytes = embedding_response_envelope_bytes(candidate.execution.upstream_model())
        .and_then(|upstream| {
            embedding_response_envelope_bytes(public_model)
                .map(|downstream| upstream.max(downstream))
        })
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let item_bytes = embedding_response_item_bytes(vector_bytes).ok_or_else(|| {
        RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        }
    })?;
    let available_with_first_separator = response_budget
        .checked_sub(envelope_bytes)
        .and_then(|available| available.checked_add(1))
        .ok_or_else(|| RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        })?;
    let bytes_per_additional_item = item_bytes.checked_add(1).ok_or_else(|| {
        RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        }
    })?;
    let budget_max_inputs = available_with_first_separator / bytes_per_additional_item;
    let budget_max_inputs = u32::try_from(budget_max_inputs).unwrap_or(u32::MAX);
    if budget_max_inputs == 0 {
        return Err(RegistryError::EmbeddingResponseBudgetTooSmall {
            public_model: public_model.to_owned(),
        });
    }

    // Publish and preflight only the minimum of Provider and runtime response-budget limits.
    capabilities.max_inputs = capabilities.max_inputs.min(budget_max_inputs);
    candidate.contribution.embedding_capabilities = Some(capabilities);
    Ok(())
}

/// Returns the largest dimension a client can receive from an Embeddings interface.
fn maximum_embedding_dimensions(capabilities: EmbeddingsCapabilities) -> Option<u32> {
    match capabilities.allowed_dimensions {
        Some(EmbeddingDimensionDomain::Range { maximum, .. }) => Some(maximum),
        Some(EmbeddingDimensionDomain::Values { values }) => values.last().copied(),
        None => Some(capabilities.default_dimensions),
    }
}

/// Returns the worst JSON byte length of one vector among all permitted encodings.
fn worst_case_embedding_vector_bytes(
    capabilities: EmbeddingsCapabilities,
    dimensions: usize,
) -> Option<usize> {
    // Include the default plus every explicitly requestable encoding without double-counting.
    let mut encodings = vec![capabilities.default_encoding];
    if let Some(allowed) = capabilities.allowed_encodings {
        for encoding in allowed {
            if !encodings.contains(encoding) {
                encodings.push(*encoding);
            }
        }
    }
    encodings
        .into_iter()
        .map(|encoding| embedding_vector_bytes(encoding, dimensions))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()
}

/// Returns a checked upper bound for one normalized vector JSON value.
fn embedding_vector_bytes(encoding: EmbeddingEncoding, dimensions: usize) -> Option<usize> {
    const MAX_NORMALIZED_JSON_NUMBER_BYTES: usize = 32;
    match encoding {
        EmbeddingEncoding::Float => dimensions
            .checked_mul(MAX_NORMALIZED_JSON_NUMBER_BYTES)
            .and_then(|numbers| numbers.checked_add(dimensions.saturating_sub(1)))
            .and_then(|contents| contents.checked_add(2)),
        EmbeddingEncoding::Base64 => dimensions
            .checked_mul(std::mem::size_of::<f32>())
            .and_then(|bytes| bytes.checked_add(2))
            .map(|rounded| rounded / 3)
            .and_then(|groups| groups.checked_mul(4))
            .and_then(|characters| characters.checked_add(2)),
    }
}

/// Returns the fixed JSON bytes for one data item around its bounded vector and index.
fn embedding_response_item_bytes(vector_bytes: usize) -> Option<usize> {
    const ITEM_PREFIX: &str = r#"{"object":"embedding","embedding":"#;
    const ITEM_SUFFIX_AND_MAX_INDEX: &str = r#", "index":4294967295}"#;
    ITEM_PREFIX
        .len()
        .checked_add(vector_bytes)
        .and_then(|bytes| bytes.checked_add(ITEM_SUFFIX_AND_MAX_INDEX.len()))
}

/// Returns a fixed top-level response envelope using worst-case usage counters and model escaping.
fn embedding_response_envelope_bytes(model: &str) -> Option<usize> {
    // Serialize the trusted model string so quotes and non-ASCII content use the real JSON bound.
    let model = serde_json::to_string(model).ok()?;
    let envelope = format!(
        r#"{{"object":"list","data":[],"model":{model},"usage":{{"prompt_tokens":18446744073709551615,"total_tokens":18446744073709551615}}}}"#
    );
    Some(envelope.len())
}

/// Compiles the unique operation execution interfaces from one Public Model's candidates.
fn compile_execution_interfaces(
    candidates: &[PrecompiledRouteCandidate],
) -> ModelExecutionInterfaces {
    // Partition the already ordered candidates by their fixed downstream operation.
    ModelExecutionInterfaces {
        chat_completions: compile_execution_interface(
            OperationKind::ChatCompletions,
            candidates.iter().filter(|candidate| {
                candidate.execution.downstream_operation() == OperationKind::ChatCompletions
            }),
        ),
        responses: compile_execution_interface(
            OperationKind::Responses,
            candidates.iter().filter(|candidate| {
                candidate.execution.downstream_operation() == OperationKind::Responses
            }),
        ),
        embeddings: compile_execution_interface(
            OperationKind::EmbeddingsCreate,
            candidates.iter().filter(|candidate| {
                candidate.execution.downstream_operation() == OperationKind::EmbeddingsCreate
            }),
        ),
    }
}

/// Pairs one operation's conservative capability contract with its fixed static candidates.
fn compile_execution_interface<'a>(
    operation: OperationKind,
    candidates: impl Iterator<Item = &'a PrecompiledRouteCandidate>,
) -> Option<ModelExecutionInterface> {
    // Materialize one operation's static candidates without changing their configuration order.
    let candidates = candidates.collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let contributions = candidates
        .iter()
        .map(|candidate| candidate.contribution.clone())
        .collect::<Vec<_>>();
    let (generation_capabilities, embedding_capabilities) = match operation {
        OperationKind::ChatCompletions | OperationKind::Responses => {
            (aggregate_interface(contributions.iter()), None)
        }
        OperationKind::EmbeddingsCreate => {
            (None, aggregate_embedding_interface(contributions.iter()))
        }
    };

    // Freeze the matching planning data beside the contract that was derived from it.
    Some(ModelExecutionInterface {
        generation_capabilities,
        embedding_capabilities,
        candidates: candidates
            .into_iter()
            .map(|candidate| candidate.execution.clone())
            .collect(),
    })
}

/// Static candidate and capability input compiled together from one resolved Route binding.
struct PrecompiledRouteCandidate {
    execution: RouteExecutionCandidate,
    contribution: RouteContractContribution,
}

impl PrecompiledRouteCandidate {
    /// Includes only a statically enabled Target/API and preserves its validated Route facts.
    fn from_binding(binding: &PublicRouteBinding<'_>) -> Option<Self> {
        // Reject disabled Targets and APIs before either capability aggregation or request planning can see them.
        if !binding.target_enabled || !binding.upstream_api.capabilities().enabled() {
            return None;
        }

        // Freeze every planning fact that otherwise required a request-time Route/API lookup.
        Some(Self {
            execution: RouteExecutionCandidate {
                route_id: binding.route_id.clone(),
                upstream_target_id: binding.route.upstream_target().to_owned(),
                downstream_operation: binding.route.downstream_operation(),
                upstream_operation: binding.route.upstream_operation(),
                mode: binding.route.mode(),
                upstream_model: binding.upstream_api.upstream_model().to_owned(),
                reasoning_output: binding.upstream_api.reasoning_output(),
            },
            contribution: RouteContractContribution::from_binding(binding),
        })
    }
}

#[derive(Clone)]
struct RouteContractContribution {
    model_tasks: Vec<ModelTask>,
    embedding_capabilities: Option<EmbeddingsCapabilities>,
    continuation_issuer: ContinuationIssuer,
    context_window: ContextWindow,
    modalities: ModelModalities,
    model_modalities: Option<ModelModalities>,
    model_description: Option<String>,
    model_tokenizer: Option<String>,
    model_knowledge_cutoff: Option<String>,
    model_reasoning: SupportState,
    model_reasoning_levels: Vec<ReasoningLevel>,
    interface_parameters: Vec<String>,
    streaming: SupportState,
    system_messages: SupportState,
    function_calling: SupportState,
    parallel_tool_calls: SupportState,
    structured_outputs: SupportState,
    reasoning: SupportState,
    reasoning_levels: Vec<ReasoningLevel>,
    reasoning_output: ReasoningOutputMode,
    prompt_caching: SupportState,
    store: SupportState,
    previous_response_id: SupportState,
    background: SupportState,
}

/// Internal target/operation identity used only to prove continuation issuer uniqueness.
#[derive(Clone, Eq, PartialEq)]
struct ContinuationIssuer {
    upstream_target: String,
    upstream_operation: OperationKind,
}

impl RouteContractContribution {
    /// Converts a Native or Bridged Route into one fixed public-contract input.
    fn from_binding(binding: &PublicRouteBinding<'_>) -> Self {
        let route = binding.route;
        let upstream_api = binding.upstream_api;
        let capabilities = upstream_api.capabilities();
        if let Some(embeddings) = capabilities.embeddings() {
            return Self::from_embedding_binding(binding, embeddings);
        }
        let generation = capabilities
            .generation_capabilities()
            .expect("generation operation has generation capabilities");

        // The Bridge exposes only the public subset fully supported by the current converter.
        let bridged = route.mode() == RouteMode::Bridged;
        let structured_outputs = generation.structured_outputs && !bridged;
        let image_input = generation.image_input && !bridged;
        let store = generation.store && !bridged;
        let reasoning = route_reasoning_support(upstream_api, bridged);
        let reasoning_levels = if reasoning == SupportState::Supported {
            upstream_api.model().reasoning_levels().to_vec()
        } else {
            Vec::new()
        };
        let (
            previous_response_id,
            background,
            prompt_caching,
            audio_input,
            file_input,
            audio_output,
        ) = protocol_specific_capabilities(route, upstream_api, bridged);

        // Narrow model parameters and protocol-control fields to those fully accepted by this Route.
        let model_parameters =
            sorted_unique(upstream_api.model().supported_parameters().iter().cloned());
        let interface_parameters = interface_parameters(
            route
                .downstream_protocol()
                .expect("generation Route has a downstream API protocol"),
            route.mode(),
            &model_parameters,
            generation.streaming,
            generation.function_calling,
            generation.parallel_tool_calls,
            structured_outputs,
            reasoning,
            store,
            previous_response_id,
            background,
        );
        let mut input = vec![InputModality::Text];
        if image_input {
            input.push(InputModality::Image);
        }
        if audio_input {
            input.push(InputModality::Audio);
        }
        if file_input {
            input.push(InputModality::File);
        }
        let mut output = vec![OutputModality::Text];
        if audio_output {
            output.push(OutputModality::Audio);
        }
        if let Some(model_input) = upstream_api.model().input_modalities() {
            input.retain(|modality| model_input.contains(modality));
        }
        if let Some(model_output) = upstream_api.model().output_modalities() {
            output.retain(|modality| model_output.contains(modality));
        }
        let model_modalities = upstream_api
            .model()
            .input_modalities()
            .zip(upstream_api.model().output_modalities())
            .map(|(input, output)| ModelModalities {
                input: sorted_values(input),
                output: sorted_values(output),
            });
        let model_reasoning = SupportState::from(upstream_api.model().reasoning());
        let model_reasoning_levels = if model_reasoning.is_supported() {
            upstream_api.model().reasoning_levels().to_vec()
        } else {
            Vec::new()
        };

        Self {
            model_tasks: vec![ModelTask::Chat, ModelTask::TextGeneration],
            embedding_capabilities: None,
            continuation_issuer: ContinuationIssuer {
                upstream_target: route.upstream_target().to_owned(),
                upstream_operation: route.upstream_operation(),
            },
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            model_modalities,
            model_description: upstream_api.model().description().map(str::to_owned),
            model_tokenizer: upstream_api.model().tokenizer().map(str::to_owned),
            model_knowledge_cutoff: upstream_api.model().knowledge_cutoff().map(str::to_owned),
            model_reasoning,
            model_reasoning_levels,
            interface_parameters,
            streaming: SupportState::from_bool(generation.streaming),
            system_messages: SupportState::Unknown,
            function_calling: SupportState::from_bool(generation.function_calling),
            parallel_tool_calls: SupportState::from_bool(generation.parallel_tool_calls),
            structured_outputs: SupportState::from_bool(structured_outputs),
            reasoning,
            reasoning_levels,
            reasoning_output: route_reasoning_output(upstream_api, bridged, reasoning),
            prompt_caching: SupportState::from_bool(prompt_caching),
            store: SupportState::from_bool(store),
            previous_response_id: SupportState::from_bool(previous_response_id),
            background: SupportState::from_bool(background),
        }
    }

    /// Converts one Native Embeddings Route into public model facts and its typed interface profile.
    fn from_embedding_binding(
        binding: &PublicRouteBinding<'_>,
        capabilities: EmbeddingsCapabilities,
    ) -> Self {
        let route = binding.route;
        let upstream_api = binding.upstream_api;

        // Derive safe model facts without projecting target, API, or upstream-model identity.
        let mut input = vec![InputModality::Text];
        let mut output = vec![OutputModality::Embedding];
        if let Some(model_input) = upstream_api.model().input_modalities() {
            input.retain(|modality| model_input.contains(modality));
        }
        if let Some(model_output) = upstream_api.model().output_modalities() {
            output.retain(|modality| model_output.contains(modality));
        }
        let model_modalities = upstream_api
            .model()
            .input_modalities()
            .zip(upstream_api.model().output_modalities())
            .map(|(input, output)| ModelModalities {
                input: sorted_values(input),
                output: sorted_values(output),
            });
        let model_reasoning = SupportState::from(upstream_api.model().reasoning());

        // Populate generation-only fields with explicit unsupported values; they are never projected into this operation.
        Self {
            model_tasks: vec![ModelTask::Embedding],
            embedding_capabilities: Some(capabilities),
            continuation_issuer: ContinuationIssuer {
                upstream_target: route.upstream_target().to_owned(),
                upstream_operation: route.upstream_operation(),
            },
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            model_modalities,
            model_description: upstream_api.model().description().map(str::to_owned),
            model_tokenizer: upstream_api.model().tokenizer().map(str::to_owned),
            model_knowledge_cutoff: upstream_api.model().knowledge_cutoff().map(str::to_owned),
            model_reasoning,
            model_reasoning_levels: if model_reasoning.is_supported() {
                upstream_api.model().reasoning_levels().to_vec()
            } else {
                Vec::new()
            },
            interface_parameters: Vec::new(),
            streaming: SupportState::Unsupported,
            system_messages: SupportState::Unsupported,
            function_calling: SupportState::Unsupported,
            parallel_tool_calls: SupportState::Unsupported,
            structured_outputs: SupportState::Unsupported,
            reasoning: SupportState::Unsupported,
            reasoning_levels: Vec::new(),
            reasoning_output: ReasoningOutputMode::Unsupported,
            prompt_caching: SupportState::Unsupported,
            store: SupportState::Unsupported,
            previous_response_id: SupportState::Unsupported,
            background: SupportState::Unsupported,
        }
    }
}

/// Returns the reasoning output form actually observable through the downstream interface.
fn route_reasoning_output(
    upstream_api: &UpstreamApi,
    bridged: bool,
    reasoning: SupportState,
) -> ReasoningOutputMode {
    if !bridged {
        return upstream_api.reasoning_output().into();
    }
    match reasoning {
        SupportState::Supported => upstream_api.reasoning_output().into(),
        SupportState::Unsupported => ReasoningOutputMode::Unsupported,
        SupportState::Unknown => ReasoningOutputMode::Unknown,
    }
}

/// Reads protocol-specific Native endpoint capabilities; the Bridge always narrows state and extra modalities.
fn protocol_specific_capabilities(
    route: &Route,
    upstream_api: &UpstreamApi,
    bridged: bool,
) -> (bool, bool, bool, bool, bool, bool) {
    if bridged {
        return (false, false, false, false, false, false);
    }
    match upstream_api.capabilities() {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => (
            false,
            false,
            capabilities.prompt_caching,
            capabilities.audio_input,
            capabilities.file_input,
            capabilities.audio_output,
        ),
        UpstreamApiCapabilities::Responses(capabilities) => (
            route.downstream_operation() == OperationKind::Responses
                && capabilities.previous_response_id,
            route.downstream_operation() == OperationKind::Responses && capabilities.background,
            capabilities.prompt_caching,
            false,
            capabilities.file_input,
            false,
        ),
        UpstreamApiCapabilities::Embeddings(_) => {
            unreachable!("Embeddings does not use generation protocol capabilities")
        }
    }
}

/// Determines whether model reasoning remains publishable as a downstream request capability after this Route.
fn route_reasoning_support(upstream_api: &UpstreamApi, bridged: bool) -> SupportState {
    let model_support = SupportState::from(upstream_api.model().reasoning());
    if !bridged || model_support != SupportState::Supported {
        return model_support;
    }
    match (
        upstream_api
            .api_protocol()
            .expect("reasoning support is generation-only"),
        upstream_api.reasoning_output(),
    ) {
        (ApiProtocol::ChatCompletions, ReasoningOutput::PlainText)
        | (ApiProtocol::Responses, ReasoningOutput::PlainText | ReasoningOutput::Summary) => {
            SupportState::Supported
        }
        (_, ReasoningOutput::Unknown) => SupportState::Unknown,
        _ => SupportState::Unsupported,
    }
}

/// Produces the parameter names that one Route guarantees to accept downstream.
#[allow(clippy::too_many_arguments)]
fn interface_parameters(
    protocol: ApiProtocol,
    mode: RouteMode,
    model_parameters: &[String],
    streaming: bool,
    function_calling: bool,
    parallel_tool_calls: bool,
    structured_outputs: bool,
    reasoning: SupportState,
    store: bool,
    previous_response_id: bool,
    background: bool,
) -> Vec<String> {
    // Native retains confirmed model parameters; Bridge keeps only the converter's explicit allowlist.
    let mut parameters = model_parameters
        .iter()
        .filter(|parameter| {
            mode == RouteMode::Native || bridge_parameter_allowed(protocol, parameter)
        })
        .cloned()
        .collect::<BTreeSet<_>>();

    // Add and narrow protocol-control fields actually gated by OpenBridge.
    if streaming {
        parameters.insert("stream".to_owned());
    }
    if function_calling {
        parameters.insert("tools".to_owned());
        parameters.insert("tool_choice".to_owned());
    } else {
        parameters.remove("tools");
        parameters.remove("tool_choice");
    }
    if parallel_tool_calls {
        parameters.insert("parallel_tool_calls".to_owned());
    } else {
        parameters.remove("parallel_tool_calls");
    }
    if structured_outputs {
        match protocol {
            ApiProtocol::ChatCompletions => {
                parameters.insert("response_format".to_owned());
            }
            ApiProtocol::Responses => {
                parameters.insert("text".to_owned());
            }
        }
    } else {
        parameters.remove("response_format");
        parameters.remove("structured_outputs");
        parameters.remove("text");
    }
    if reasoning.is_supported() {
        parameters.insert(match protocol {
            ApiProtocol::ChatCompletions => "reasoning_effort".to_owned(),
            ApiProtocol::Responses => "reasoning".to_owned(),
        });
    } else {
        parameters.remove("reasoning");
        parameters.remove("reasoning_effort");
    }
    if store {
        parameters.insert("store".to_owned());
    } else {
        parameters.remove("store");
    }
    if previous_response_id {
        parameters.insert("previous_response_id".to_owned());
    }
    if background {
        parameters.insert("background".to_owned());
    }
    parameters.into_iter().collect()
}

/// Returns whether the current Bridge request converter can represent a parameter completely.
fn bridge_parameter_allowed(protocol: ApiProtocol, parameter: &str) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => matches!(
            parameter,
            "max_completion_tokens"
                | "max_tokens"
                | "parallel_tool_calls"
                | "reasoning_effort"
                | "stream"
                | "temperature"
                | "tool_choice"
                | "tools"
                | "top_p"
        ),
        ApiProtocol::Responses => matches!(
            parameter,
            "max_output_tokens"
                | "parallel_tool_calls"
                | "reasoning"
                | "stream"
                | "temperature"
                | "tool_choice"
                | "tools"
                | "top_p"
        ),
    }
}

/// Projects the single validated Native Embeddings candidate into its typed public contract.
fn aggregate_embedding_interface<'a>(
    contributions: impl Iterator<Item = &'a RouteContractContribution>,
) -> Option<EmbeddingInterfaceCapabilities> {
    // Select only the Embeddings profile; the registry compiler rejects more than one executable candidate.
    let capabilities = contributions
        .filter_map(|contribution| contribution.embedding_capabilities)
        .collect::<Vec<_>>();
    debug_assert!(capabilities.len() <= 1);
    capabilities
        .first()
        .copied()
        .map(EmbeddingInterfaceCapabilities::from_capabilities)
}

/// Reduces all Route contract inputs for one protocol to a unique interface contract.
fn aggregate_interface<'a>(
    contributions: impl Iterator<Item = &'a RouteContractContribution> + Clone,
) -> Option<ModelInterfaceCapabilities> {
    let contributions = contributions.collect::<Vec<_>>();
    if contributions.is_empty() {
        return None;
    }

    // Compute conservative intersections for scalars, sets, and reasoning output separately.
    let context_window =
        ContextWindow::intersection(contributions.iter().map(|value| &value.context_window));
    let modalities =
        ModelModalities::intersection(contributions.iter().map(|value| &value.modalities));
    let previous_response_id = aggregate_previous_response_id(&contributions);
    let mut supported_parameters = intersect_sets(
        contributions
            .iter()
            .map(|value| value.interface_parameters.as_slice()),
    );
    if !previous_response_id.is_supported() {
        supported_parameters.retain(|parameter| parameter != "previous_response_id");
    }
    let streaming = SupportState::intersection(contributions.iter().map(|value| value.streaming));
    let function_calling =
        SupportState::intersection(contributions.iter().map(|value| value.function_calling));
    let parallel_tool_calls =
        SupportState::intersection(contributions.iter().map(|value| value.parallel_tool_calls));
    let structured_outputs =
        SupportState::intersection(contributions.iter().map(|value| value.structured_outputs));
    let reasoning = SupportState::intersection(contributions.iter().map(|value| value.reasoning));
    let reasoning_levels = if reasoning.is_supported() {
        intersect_sets(
            contributions
                .iter()
                .map(|value| value.reasoning_levels.as_slice()),
        )
    } else {
        Vec::new()
    };
    let reasoning_output =
        intersect_reasoning_output(contributions.iter().map(|value| value.reasoning_output));

    // Build stable tool, structured-output, and state subobjects from the aggregate state.
    Some(ModelInterfaceCapabilities {
        context_window,
        modalities,
        supported_parameters,
        streaming,
        system_messages: SupportState::intersection(
            contributions.iter().map(|value| value.system_messages),
        ),
        tools: ToolCapabilities {
            support: function_calling,
            types: function_calling
                .is_supported()
                .then_some(ToolType::Function)
                .into_iter()
                .collect(),
            tool_choice_modes: if function_calling.is_supported() {
                vec![
                    ToolChoiceMode::None,
                    ToolChoiceMode::Auto,
                    ToolChoiceMode::Required,
                    ToolChoiceMode::Named,
                ]
            } else {
                Vec::new()
            },
            parallel_calls: parallel_tool_calls,
            strict_schema: if function_calling.is_supported() && structured_outputs.is_supported() {
                SupportState::Supported
            } else {
                SupportState::Unsupported
            },
        },
        structured_outputs: StructuredOutputCapabilities {
            support: structured_outputs,
            modes: if structured_outputs.is_supported() {
                vec![
                    StructuredOutputMode::JsonObject,
                    StructuredOutputMode::JsonSchema,
                ]
            } else {
                Vec::new()
            },
            strict_schema: if structured_outputs.is_supported() {
                SupportState::Supported
            } else {
                SupportState::Unsupported
            },
        },
        reasoning: InterfaceReasoningCapabilities {
            support: reasoning,
            levels: reasoning_levels,
            output: reasoning_output,
        },
        prompt_caching: SupportState::intersection(
            contributions.iter().map(|value| value.prompt_caching),
        ),
        state: StateCapabilities {
            store: SupportState::intersection(contributions.iter().map(|value| value.store)),
            previous_response_id,
            background: SupportState::intersection(
                contributions.iter().map(|value| value.background),
            ),
        },
    })
}

/// Exposes continuation only when every Route supports it and one target/API is the unique issuer.
fn aggregate_previous_response_id(contributions: &[&RouteContractContribution]) -> SupportState {
    // Intersect Route capabilities before applying the stricter issuer-affinity boundary.
    let support =
        SupportState::intersection(contributions.iter().map(|value| value.previous_response_id));
    if !support.is_supported() {
        return support;
    }

    // Reject an otherwise supported contract when a response ID could belong to multiple issuers.
    let Some(first) = contributions.first() else {
        return SupportState::Unsupported;
    };
    if contributions
        .iter()
        .all(|value| value.continuation_issuer == first.continuation_issuer)
    {
        support
    } else {
        SupportState::Unsupported
    }
}

/// Aggregates Public Model model capabilities without mixing in Provider or Route identity.
fn aggregate_model_capabilities(contributions: &[RouteContractContribution]) -> ModelCapabilities {
    if contributions.is_empty() {
        return ModelCapabilities {
            tasks: Vec::new(),
            context_window: ContextWindow::from_model(ModelContextLength::default()),
            modalities: ModelModalities {
                input: Vec::new(),
                output: Vec::new(),
            },
            tokenizer: None,
            knowledge_cutoff: None,
            reasoning: ModelReasoningCapabilities {
                support: SupportState::Unknown,
                levels: Vec::new(),
            },
        };
    }

    // Intersect model facts across executable Routes so fallback cannot expand public capability.
    let reasoning =
        SupportState::intersection(contributions.iter().map(|value| value.model_reasoning));
    let declared_modalities = contributions
        .iter()
        .map(|value| value.model_modalities.as_ref())
        .collect::<Option<Vec<_>>>();
    ModelCapabilities {
        tasks: intersect_sets(
            contributions
                .iter()
                .map(|contribution| contribution.model_tasks.as_slice()),
        ),
        context_window: ContextWindow::intersection(
            contributions.iter().map(|value| &value.context_window),
        ),
        modalities: declared_modalities.map_or_else(
            || ModelModalities::intersection(contributions.iter().map(|value| &value.modalities)),
            |modalities| ModelModalities::intersection(modalities.into_iter()),
        ),
        tokenizer: intersect_optional_string(
            contributions
                .iter()
                .map(|value| value.model_tokenizer.as_deref()),
        ),
        knowledge_cutoff: intersect_optional_string(
            contributions
                .iter()
                .map(|value| value.model_knowledge_cutoff.as_deref()),
        ),
        reasoning: ModelReasoningCapabilities {
            support: reasoning,
            levels: if reasoning.is_supported() {
                intersect_sets(
                    contributions
                        .iter()
                        .map(|value| value.model_reasoning_levels.as_slice()),
                )
            } else {
                Vec::new()
            },
        },
    }
}

/// Returns a safe minimum only when every Route provides a value.
fn intersect_optional_limit(values: impl Iterator<Item = Option<u32>>) -> Option<u32> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() || values.iter().any(Option::is_none) {
        None
    } else {
        values.into_iter().flatten().min()
    }
}

/// Returns one optional catalog string only when every Route confirms the same value.
fn intersect_optional_string<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let mut values = values;
    let first = values.next().flatten()?;
    values
        .all(|value| value == Some(first))
        .then(|| first.to_owned())
}

/// Computes a stable intersection for ordered comparable sets.
fn intersect_sets<'a, T>(values: impl Iterator<Item = &'a [T]>) -> Vec<T>
where
    T: Clone + Ord + 'a,
{
    let mut values = values.map(|value| value.iter().cloned().collect::<BTreeSet<_>>());
    let Some(mut intersection) = values.next() else {
        return Vec::new();
    };
    for value in values {
        intersection.retain(|item| value.contains(item));
    }
    intersection.into_iter().collect()
}

/// Deduplicates a parameter iterator and sorts it by wire name.
fn sorted_unique(values: impl Iterator<Item = String>) -> Vec<String> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

/// Copies and stably sorts an already validated, duplicate-free enum set.
fn sorted_values<T: Clone + Ord>(values: &[T]) -> Vec<T> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Publishes a reasoning output form only when every Route returns the same form.
fn intersect_reasoning_output(
    mut values: impl Iterator<Item = ReasoningOutputMode>,
) -> ReasoningOutputMode {
    let Some(first) = values.next() else {
        return ReasoningOutputMode::Unknown;
    };
    if values.all(|value| value == first) {
        first
    } else {
        ReasoningOutputMode::Unknown
    }
}
