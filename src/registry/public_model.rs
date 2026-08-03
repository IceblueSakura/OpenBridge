//! Fixed downstream Public Model contracts and safe serialization models.
//!
//! This module compiles only client-visible model facts and Chat/Responses interface capabilities.
//! Capabilities use the conservative intersection of all executable Routes, while responses retain
//! no Provider, Target, Route, upstream-model, or credential boundary.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::core::{ApiProtocol, ReasoningOutput};

use super::{
    InputModality, ModelContextLength, ModelLifecycle, ModelLifecycleStatus, OutputModality,
    PublicModelConfig, ReasoningLevel, ReasoningSupport, Route, RouteMode, UpstreamApi,
    UpstreamApiCapabilities,
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
    supported_parameters: Vec<String>,
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

/// The two OpenAI-compatible interface contracts of a Public Model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaces {
    chat_completions: Option<ModelInterfaceCapabilities>,
    responses: Option<ModelInterfaceCapabilities>,
}

impl ModelInterfaces {
    /// Returns the fixed interface contract for the downstream protocol.
    pub(crate) const fn for_protocol(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        match protocol {
            ApiProtocol::ChatCompletions => self.chat_completions.as_ref(),
            ApiProtocol::Responses => self.responses.as_ref(),
        }
    }

    /// Returns whether at least one executable interface exists.
    const fn is_available(&self) -> bool {
        self.chat_completions.is_some() || self.responses.is_some()
    }
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

    /// Returns the fixed capability contract shared with request preflight for the protocol.
    pub(crate) const fn interface(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        self.interfaces.for_protocol(protocol)
    }
}

/// Resolved downstream Public Model, fixed information object, and ordered Route list.
#[derive(Debug)]
pub struct PublicModel {
    pub(super) routes: Vec<String>,
    pub(super) info: PublicModelInfo,
}

impl PublicModel {
    /// Returns Route IDs ordered by priority; capabilities do not change this order.
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

    /// Returns the unique capability contract used by request preflight for the downstream protocol.
    pub(crate) const fn interface(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        self.info.interface(protocol)
    }

    /// Returns whether the model remains visible to clients and has at least one executable interface.
    pub(crate) fn is_available(&self) -> bool {
        self.info.lifecycle.status != ModelLifecycleStatus::Retired
            && self.info.interfaces.is_available()
    }
}

/// View of one executable Route used while compiling a Public Model.
pub(super) struct PublicRouteBinding<'a> {
    pub(super) route: &'a Route,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) target_enabled: bool,
}

/// Compiles a fixed Public Model without deployment details from the complete Route set.
pub(super) fn compile_public_model(
    config: PublicModelConfig,
    bindings: &[PublicRouteBinding<'_>],
) -> PublicModel {
    // Include only statically enabled Routes whose endpoint capability is enabled.
    let contributions = bindings
        .iter()
        .filter(|binding| binding.target_enabled)
        .filter_map(RouteContractContribution::from_binding)
        .collect::<Vec<_>>();

    // Compute the unique conservative intersection per protocol and aggregate model facts across executable Routes.
    let chat_completions = aggregate_interface(
        contributions
            .iter()
            .filter(|contribution| contribution.protocol == ApiProtocol::ChatCompletions),
    );
    let responses = aggregate_interface(
        contributions
            .iter()
            .filter(|contribution| contribution.protocol == ApiProtocol::Responses),
    );
    let capabilities = aggregate_model_capabilities(&contributions);
    let description = config.description.or_else(|| {
        intersect_optional_string(
            contributions
                .iter()
                .map(|contribution| contribution.model_description.as_deref()),
        )
    });

    // Freeze the standard projection and extension object; retain Route IDs only in private execution data.
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
        interfaces: ModelInterfaces {
            chat_completions,
            responses,
        },
    };
    PublicModel {
        routes: config.routes,
        info,
    }
}

#[derive(Clone)]
struct RouteContractContribution {
    protocol: ApiProtocol,
    context_window: ContextWindow,
    modalities: ModelModalities,
    model_modalities: Option<ModelModalities>,
    model_description: Option<String>,
    model_tokenizer: Option<String>,
    model_knowledge_cutoff: Option<String>,
    model_parameters: Vec<String>,
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

impl RouteContractContribution {
    /// Converts a Native or Bridged Route into one fixed public-contract input.
    fn from_binding(binding: &PublicRouteBinding<'_>) -> Option<Self> {
        let route = binding.route;
        let upstream_api = binding.upstream_api;
        let generation = upstream_api.capabilities().generation_capabilities();
        if !generation.enabled {
            return None;
        }

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
            route.downstream_protocol(),
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

        Some(Self {
            protocol: route.downstream_protocol(),
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            model_modalities,
            model_description: upstream_api.model().description().map(str::to_owned),
            model_tokenizer: upstream_api.model().tokenizer().map(str::to_owned),
            model_knowledge_cutoff: upstream_api.model().knowledge_cutoff().map(str::to_owned),
            model_parameters,
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
        })
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
            route.downstream_protocol() == ApiProtocol::Responses
                && capabilities.previous_response_id,
            route.downstream_protocol() == ApiProtocol::Responses && capabilities.background,
            capabilities.prompt_caching,
            false,
            capabilities.file_input,
            false,
        ),
    }
}

/// Determines whether model reasoning remains publishable as a downstream request capability after this Route.
fn route_reasoning_support(upstream_api: &UpstreamApi, bridged: bool) -> SupportState {
    let model_support = SupportState::from(upstream_api.model().reasoning());
    if !bridged || model_support != SupportState::Supported {
        return model_support;
    }
    match (upstream_api.protocol(), upstream_api.reasoning_output()) {
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
    let supported_parameters = intersect_sets(
        contributions
            .iter()
            .map(|value| value.interface_parameters.as_slice()),
    );
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
            previous_response_id: SupportState::intersection(
                contributions.iter().map(|value| value.previous_response_id),
            ),
            background: SupportState::intersection(
                contributions.iter().map(|value| value.background),
            ),
        },
    })
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
            supported_parameters: Vec::new(),
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
        tasks: vec![ModelTask::Chat, ModelTask::TextGeneration],
        context_window: ContextWindow::intersection(
            contributions.iter().map(|value| &value.context_window),
        ),
        modalities: declared_modalities.map_or_else(
            || ModelModalities::intersection(contributions.iter().map(|value| &value.modalities)),
            |modalities| ModelModalities::intersection(modalities.into_iter()),
        ),
        supported_parameters: intersect_sets(
            contributions
                .iter()
                .map(|value| value.model_parameters.as_slice()),
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
