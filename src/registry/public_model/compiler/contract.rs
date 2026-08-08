//! Per-Route contract contribution and conservative capability aggregation.
//!
//! Each executable Route contributes only facts its Native or Bridged path can guarantee. Public
//! model facts and operation interfaces are intersections; deployment identity remains private.

use std::collections::BTreeSet;

use crate::{
    core::{
        ApiProtocol, AudioTask, EmbeddingsCapabilities, OperationKind, ReasoningOutput,
        StructuredOutputMode, ToolChoiceMode,
    },
    registry::{
        InputModality, OutputModality, ReasoningLevel, Route, RouteMode, UpstreamApi,
        UpstreamApiCapabilities,
    },
};

use super::super::{
    AudioInputInterfaceCapabilities, AudioOutputInterfaceCapabilities, ContextWindow,
    ImageInputInterfaceCapabilities, ModelModalities, ModelTask, ReasoningOutputMode, SupportState,
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
    pub(super) audio_input: Option<AudioInputInterfaceCapabilities>,
    pub(super) voice_conditioning: Option<AudioInputInterfaceCapabilities>,
    pub(super) audio_output: Option<AudioOutputInterfaceCapabilities>,
    pub(super) audio_task: Option<AudioTask>,
    pub(super) model_modalities: Option<ModelModalities>,
    pub(super) model_description: Option<String>,
    pub(super) model_tokenizer: Option<String>,
    pub(super) model_knowledge_cutoff: Option<String>,
    pub(super) model_reasoning: SupportState,
    pub(super) model_reasoning_levels: Vec<ReasoningLevel>,
    pub(super) interface_parameters: Vec<String>,
    pub(super) streaming: SupportState,
    pub(super) non_streaming: SupportState,
    pub(super) system_messages: SupportState,
    pub(super) function_tools: SupportState,
    pub(super) function_tool_choice_modes: Vec<ToolChoiceMode>,
    pub(super) tool_strict_schema: SupportState,
    pub(super) parallel_tool_calls: SupportState,
    pub(super) structured_outputs: SupportState,
    pub(super) structured_output_modes: Vec<StructuredOutputMode>,
    pub(super) structured_output_strict_schema: SupportState,
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
        let function_tools = generation.function_tools;
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
            voice_conditioning,
            file_input,
            audio_output,
            audio_task,
        ) = protocol_specific_capabilities(route, upstream_api, bridged);
        // Provider ceilings may use `Any`, but an executable API must publish one concrete task identity.
        if audio_task == Some(AudioTask::Any) {
            panic!("audio task ceiling cannot be used as an executable Route identity");
        }

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
            function_tools.is_some(),
            function_tools.is_some_and(|profile| profile.parallel_calls),
            structured_outputs.is_some(),
            reasoning,
            store,
            previous_response_id,
            background,
        );
        let mut input = vec![InputModality::Text];
        if image_input.is_some() {
            input.push(InputModality::Image);
        }
        if audio_input.is_some() {
            input.push(InputModality::Audio);
        }
        if voice_conditioning.is_some() && !input.contains(&InputModality::Audio) {
            input.push(InputModality::Audio);
        }
        if file_input {
            input.push(InputModality::File);
        }
        let mut output = vec![OutputModality::Text];
        if audio_output.is_some() {
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
            audio_input,
            voice_conditioning,
            audio_output,
            audio_task,
            model_modalities,
            model_description: upstream_api.model().description().map(str::to_owned),
            model_tokenizer: upstream_api.model().tokenizer().map(str::to_owned),
            model_knowledge_cutoff: upstream_api.model().knowledge_cutoff().map(str::to_owned),
            model_reasoning,
            model_reasoning_levels,
            interface_parameters,
            streaming: SupportState::from_bool(generation.streaming),
            non_streaming: SupportState::from_bool(
                upstream_api.streaming_policy().supports_non_streaming(),
            ),
            system_messages: SupportState::Unknown,
            function_tools: SupportState::from_bool(function_tools.is_some()),
            function_tool_choice_modes: function_tools
                .map_or_else(Vec::new, |profile| profile.choice_modes.to_vec()),
            tool_strict_schema: SupportState::from_bool(
                function_tools.is_some_and(|profile| profile.strict_schema),
            ),
            parallel_tool_calls: SupportState::from_bool(
                function_tools.is_some_and(|profile| profile.parallel_calls),
            ),
            structured_outputs: SupportState::from_bool(structured_outputs.is_some()),
            structured_output_modes: structured_outputs
                .map_or_else(Vec::new, |profile| profile.modes.to_vec()),
            structured_output_strict_schema: SupportState::from_bool(
                structured_outputs.is_some_and(|profile| profile.strict_schema),
            ),
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
            audio_input: None,
            voice_conditioning: None,
            audio_output: None,
            audio_task: None,
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
            non_streaming: SupportState::Unsupported,
            system_messages: SupportState::Unsupported,
            function_tools: SupportState::Unsupported,
            function_tool_choice_modes: Vec::new(),
            tool_strict_schema: SupportState::Unsupported,
            parallel_tool_calls: SupportState::Unsupported,
            structured_outputs: SupportState::Unsupported,
            structured_output_modes: Vec::new(),
            structured_output_strict_schema: SupportState::Unsupported,
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

/// Protocol-specific capability facts returned to the Public Model contribution builder.
type ProtocolCapabilities = (
    bool,
    bool,
    bool,
    Option<AudioInputInterfaceCapabilities>,
    Option<AudioInputInterfaceCapabilities>,
    bool,
    Option<AudioOutputInterfaceCapabilities>,
    Option<AudioTask>,
);

/// Reads protocol-specific Native endpoint capabilities; the Bridge always narrows state and extra modalities.
fn protocol_specific_capabilities(
    route: &Route,
    upstream_api: &UpstreamApi,
    bridged: bool,
) -> ProtocolCapabilities {
    if bridged {
        return (false, false, false, None, None, false, None, None);
    }
    match upstream_api.capabilities() {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => {
            let audio = capabilities.audio;
            (
                false,
                false,
                capabilities.prompt_caching,
                audio
                    .and_then(|audio| audio.input)
                    .map(AudioInputInterfaceCapabilities::from_capabilities),
                audio
                    .and_then(|audio| audio.voice_conditioning)
                    .map(AudioInputInterfaceCapabilities::from_capabilities),
                capabilities.file_input,
                audio
                    .and_then(|audio| audio.output)
                    .map(AudioOutputInterfaceCapabilities::from_capabilities),
                audio.map(|audio| audio.task),
            )
        }
        UpstreamApiCapabilities::Responses(capabilities) => (
            route.downstream_operation() == OperationKind::Responses
                && capabilities.previous_response_id,
            route.downstream_operation() == OperationKind::Responses && capabilities.background,
            capabilities.prompt_caching,
            None,
            None,
            capabilities.file_input,
            None,
            None,
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
