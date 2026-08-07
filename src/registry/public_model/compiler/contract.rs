//! Per-Route contract contribution and conservative capability aggregation.
//!
//! Each executable Route contributes only facts its Native or Bridged path can guarantee. Public
//! model facts and operation interfaces are intersections; deployment identity remains private.

use std::collections::BTreeSet;

use crate::{
    core::{ApiProtocol, EmbeddingsCapabilities, OperationKind, ReasoningOutput},
    registry::{
        InputModality, ModelContextLength, OutputModality, ReasoningLevel, Route, RouteMode,
        UpstreamApi, UpstreamApiCapabilities,
    },
};

use super::super::{
    ContextWindow, EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities,
    EmbeddingInterfaceCapabilities, EmbeddingLimits, InterfaceReasoningCapabilities,
    ModelCapabilities, ModelInterfaceCapabilities, ModelModalities, ModelReasoningCapabilities,
    ModelTask, ReasoningOutputMode, StateCapabilities, StructuredOutputCapabilities,
    StructuredOutputMode, SupportState, ToolCapabilities, ToolChoiceMode, ToolType,
};
use super::PublicRouteBinding;

impl ContextWindow {
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

impl ModelModalities {
    /// Computes the stable set intersection of multiple Route contracts.
    fn intersection<'a>(values: impl Iterator<Item = &'a Self> + Clone) -> Self {
        Self {
            input: intersect_sets(values.clone().map(|value| value.input.as_slice())),
            output: intersect_sets(values.map(|value| value.output.as_slice())),
        }
    }
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
}

#[derive(Clone)]
pub(super) struct RouteContractContribution {
    model_tasks: Vec<ModelTask>,
    pub(super) embedding_capabilities: Option<EmbeddingsCapabilities>,
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
    pub(super) fn from_binding(binding: &PublicRouteBinding<'_>) -> Self {
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
        let structured_outputs = generation.structured_outputs;
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

    /// Returns the optional canonical description contributed by this Route.
    pub(super) fn model_description(&self) -> Option<&str> {
        self.model_description.as_deref()
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
                | "response_format"
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
                | "text"
                | "stream"
                | "temperature"
                | "tool_choice"
                | "tools"
                | "top_p"
        ),
    }
}

/// Projects the single validated Native Embeddings candidate into its typed public contract.
pub(super) fn aggregate_embedding_interface<'a>(
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
pub(super) fn aggregate_interface<'a>(
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
pub(super) fn aggregate_model_capabilities(
    contributions: &[RouteContractContribution],
) -> ModelCapabilities {
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
pub(super) fn intersect_optional_string<'a>(
    values: impl Iterator<Item = Option<&'a str>>,
) -> Option<String> {
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
