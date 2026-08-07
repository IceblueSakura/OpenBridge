//! Per-Route contract contribution and conservative capability aggregation.
//!
//! Each executable Route contributes only facts its Native or Bridged path can guarantee. Public
//! model facts and operation interfaces are intersections; deployment identity remains private.

use std::collections::BTreeSet;

use crate::{
    core::{ApiProtocol, EmbeddingsCapabilities, OperationKind, ReasoningOutput},
    registry::{
        InputModality, OutputModality, ReasoningLevel, Route, RouteMode, UpstreamApi,
        UpstreamApiCapabilities,
    },
};

use super::super::{
    ContextWindow, ImageInputInterfaceCapabilities, ModelModalities, ModelTask,
    ReasoningOutputMode, SupportState,
};
use super::PublicRouteBinding;

mod aggregate;

pub(super) use aggregate::{
    aggregate_embedding_interface, aggregate_interface, aggregate_model_capabilities,
    intersect_optional_string,
};

#[derive(Clone)]
pub(super) struct RouteContractContribution {
    pub(super) model_tasks: Vec<ModelTask>,
    pub(super) embedding_capabilities: Option<EmbeddingsCapabilities>,
    pub(super) continuation_issuer: ContinuationIssuer,
    pub(super) context_window: ContextWindow,
    pub(super) modalities: ModelModalities,
    pub(super) image_input: Option<ImageInputInterfaceCapabilities>,
    pub(super) model_modalities: Option<ModelModalities>,
    pub(super) model_description: Option<String>,
    pub(super) model_tokenizer: Option<String>,
    pub(super) model_knowledge_cutoff: Option<String>,
    pub(super) model_reasoning: SupportState,
    pub(super) model_reasoning_levels: Vec<ReasoningLevel>,
    pub(super) interface_parameters: Vec<String>,
    pub(super) streaming: SupportState,
    pub(super) system_messages: SupportState,
    pub(super) function_calling: SupportState,
    pub(super) parallel_tool_calls: SupportState,
    pub(super) structured_outputs: SupportState,
    pub(super) reasoning: SupportState,
    pub(super) reasoning_levels: Vec<ReasoningLevel>,
    pub(super) reasoning_output: ReasoningOutputMode,
    pub(super) prompt_caching: SupportState,
    pub(super) store: SupportState,
    pub(super) previous_response_id: SupportState,
    pub(super) background: SupportState,
}

/// Internal target/operation identity used only to prove continuation issuer uniqueness.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct ContinuationIssuer {
    pub(super) upstream_target: String,
    pub(super) upstream_operation: OperationKind,
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
        let mut image_input = (!bridged)
            .then_some(generation.image_input)
            .flatten()
            .map(ImageInputInterfaceCapabilities::from_capabilities);
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
        if image_input.is_some() {
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
            if !model_input.contains(&InputModality::Image) {
                image_input = None;
            }
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
            image_input,
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
            image_input: None,
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
