//! Per-Route Public Model contract contribution derivation.
//!
//! Each executable Route contributes only facts its Native or Bridged path can guarantee. Public
//! model facts and operation interfaces are intersected by the sibling aggregation module;
//! deployment identity remains private.

use std::collections::BTreeSet;

use crate::{
    core::{
        ApiProtocol, EmbeddingsCapabilities, GenerationRequestField, OperationKind,
        ReasoningOutput, ResponseInclude, StructuredOutputProfile, ToolChoiceMode,
    },
    registry::{
        CanonicalTaskKind, InputModality, OutputModality, ReasoningLevel, Route, RouteMode,
        UpstreamApi, UpstreamApiCapabilities,
    },
};

use super::super::{
    AudioInterfaceCapabilities, ContextWindow, ImageInputInterfaceCapabilities, ModelModalities,
    ReasoningOutputMode, SupportState,
};
use super::{super::execution::ContinuationIssuer, PublicRouteBinding};

#[derive(Clone)]
pub(super) struct RouteContractContribution {
    pub(super) canonical_task: CanonicalTaskKind,
    pub(super) embedding_capabilities: Option<EmbeddingsCapabilities>,
    pub(super) continuation: RouteContinuationContract,
    pub(super) context_window: ContextWindow,
    pub(super) modalities: ModelModalities,
    pub(super) image_input: Option<ImageInputInterfaceCapabilities>,
    pub(super) audio: Option<AudioInterfaceCapabilities>,
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
    pub(super) structured_outputs: Option<StructuredOutputProfile>,
    pub(super) reasoning: SupportState,
    pub(super) reasoning_levels: Vec<ReasoningLevel>,
    pub(super) reasoning_output: ReasoningOutputMode,
    pub(super) response_includes: Vec<ResponseInclude>,
    pub(super) store: SupportState,
    pub(super) background: SupportState,
}

/// Per-Route continuation contract before conservative Public Model aggregation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum RouteContinuationContract {
    /// This Route cannot carry `previous_response_id` safely.
    #[default]
    Unsupported,
    /// This Native Responses Route has one statically known issuing Target/API.
    Supported {
        /// Private issuer identity consumed only by Public Model aggregation.
        issuer: ContinuationIssuer,
    },
}

impl RouteContinuationContract {
    /// Returns whether this one Route contributes continuation support.
    const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    /// Returns the unique issuer carried by a supported Route contract.
    pub(super) const fn issuer(&self) -> Option<&ContinuationIssuer> {
        match self {
            Self::Unsupported => None,
            Self::Supported { issuer } => Some(issuer),
        }
    }
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
        let ProtocolCapabilities {
            continuation,
            background,
            prompt_cache_key,
            response_includes,
            audio,
            file_input,
        } = protocol_specific_capabilities(route, upstream_api, bridged, reasoning);

        // Narrow model parameters and protocol-control fields to those fully accepted by this Route.
        let model_parameters =
            sorted_unique(upstream_api.model().supported_parameters().iter().cloned());
        let ignored_parameters = upstream_api
            .ignored_generation_parameters()
            .map(|parameter| parameter.as_wire_name())
            .collect::<BTreeSet<_>>();
        let interface_parameters = interface_parameters(
            route
                .downstream_protocol()
                .expect("generation Route has a downstream API protocol"),
            route.mode(),
            &model_parameters,
            &ignored_parameters,
            generation.streaming,
            function_tools.is_some(),
            function_tools.is_some_and(|profile| profile.parallel_calls),
            structured_outputs.is_some(),
            reasoning,
            store,
            &continuation,
            background,
            prompt_cache_key,
            &response_includes,
        );
        let mut input = vec![InputModality::Text];
        if image_input.is_some() {
            input.push(InputModality::Image);
        }
        if audio
            .as_ref()
            .is_some_and(AudioInterfaceCapabilities::has_input)
        {
            input.push(InputModality::Audio);
        }
        if file_input {
            input.push(InputModality::File);
        }
        let mut output = vec![OutputModality::Text];
        if audio
            .as_ref()
            .is_some_and(AudioInterfaceCapabilities::has_output)
        {
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
        let model_reasoning = SupportState::from(upstream_api.model().reasoning_support());
        let model_reasoning_levels = if model_reasoning.is_supported() {
            upstream_api.model().reasoning_levels().to_vec()
        } else {
            Vec::new()
        };

        Self {
            canonical_task: upstream_api.model().task_kind(),
            embedding_capabilities: None,
            continuation,
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            image_input,
            audio,
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
            structured_outputs,
            reasoning,
            reasoning_levels,
            reasoning_output: route_reasoning_output(upstream_api, bridged, reasoning),
            response_includes,
            store: SupportState::from_bool(store),
            background: SupportState::from_bool(background),
        }
    }

    /// Converts one Native Embeddings Route into public model facts and its typed interface profile.
    fn from_embedding_binding(
        binding: &PublicRouteBinding<'_>,
        capabilities: EmbeddingsCapabilities,
    ) -> Self {
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
        let model_reasoning = SupportState::from(upstream_api.model().reasoning_support());

        // Populate generation-only fields with explicit unsupported values; they are never projected into this operation.
        Self {
            canonical_task: upstream_api.model().task_kind(),
            embedding_capabilities: Some(capabilities),
            continuation: RouteContinuationContract::Unsupported,
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            image_input: None,
            audio: None,
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
            structured_outputs: None,
            reasoning: SupportState::Unsupported,
            reasoning_levels: Vec::new(),
            reasoning_output: ReasoningOutputMode::Unsupported,
            response_includes: Vec::new(),
            store: SupportState::Unsupported,
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
struct ProtocolCapabilities {
    continuation: RouteContinuationContract,
    background: bool,
    prompt_cache_key: bool,
    response_includes: Vec<ResponseInclude>,
    audio: Option<AudioInterfaceCapabilities>,
    file_input: bool,
}

/// Reads protocol-specific Native endpoint capabilities; the Bridge always narrows state and extra modalities.
fn protocol_specific_capabilities(
    route: &Route,
    upstream_api: &UpstreamApi,
    bridged: bool,
    reasoning: SupportState,
) -> ProtocolCapabilities {
    if bridged {
        let prompt_cache_key = match upstream_api.capabilities() {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities.prompt_cache_key,
            UpstreamApiCapabilities::Responses(capabilities) => capabilities.prompt_cache_key,
            UpstreamApiCapabilities::Embeddings(_) => {
                unreachable!("Embeddings does not use generation protocol capabilities")
            }
        };
        // A readable Responses-to-Chat Bridge can consume the conditional encrypted-content hint
        // without forwarding it or fabricating an opaque output item.
        let response_includes = if route.downstream_operation() == OperationKind::Responses
            && upstream_api.api_protocol() == Some(ApiProtocol::ChatCompletions)
            && reasoning.is_supported()
        {
            vec![ResponseInclude::ReasoningEncryptedContent]
        } else {
            Vec::new()
        };
        return ProtocolCapabilities {
            continuation: RouteContinuationContract::Unsupported,
            background: false,
            prompt_cache_key,
            response_includes,
            audio: None,
            file_input: false,
        };
    }
    match upstream_api.capabilities() {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => ProtocolCapabilities {
            continuation: RouteContinuationContract::Unsupported,
            background: false,
            prompt_cache_key: capabilities.prompt_cache_key,
            response_includes: Vec::new(),
            audio: capabilities
                .audio
                .map(AudioInterfaceCapabilities::from_capabilities),
            file_input: capabilities.file_input,
        },
        UpstreamApiCapabilities::Responses(capabilities) => ProtocolCapabilities {
            continuation: if route.downstream_operation() == OperationKind::Responses
                && upstream_api.supports_previous_response_id()
            {
                RouteContinuationContract::Supported {
                    issuer: ContinuationIssuer::new(
                        route.upstream_target().to_owned(),
                        route.upstream_operation(),
                    ),
                }
            } else {
                RouteContinuationContract::Unsupported
            },
            background: route.downstream_operation() == OperationKind::Responses
                && capabilities.background,
            prompt_cache_key: capabilities.prompt_cache_key,
            response_includes: capabilities.include.to_vec(),
            audio: None,
            file_input: capabilities.file_input,
        },
        UpstreamApiCapabilities::Embeddings(_) => {
            unreachable!("Embeddings does not use generation protocol capabilities")
        }
    }
}

/// Determines whether model reasoning remains publishable as a downstream request capability after this Route.
fn route_reasoning_support(upstream_api: &UpstreamApi, bridged: bool) -> SupportState {
    let model_support = SupportState::from(upstream_api.model().reasoning_support());
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
    ignored_parameters: &BTreeSet<&str>,
    streaming: bool,
    function_calling: bool,
    parallel_tool_calls: bool,
    structured_outputs: bool,
    reasoning: SupportState,
    store: bool,
    continuation: &RouteContinuationContract,
    background: bool,
    prompt_cache_key: bool,
    response_includes: &[ResponseInclude],
) -> Vec<String> {
    // Retain only source-protocol parameters; Bridge also accepts hints removed before conversion.
    let mut parameters = model_parameters
        .iter()
        .filter(|parameter| {
            GenerationRequestField::from_wire(protocol, parameter).is_some()
                && (mode == RouteMode::Native
                    || bridge_parameter_allowed(protocol, parameter)
                    || ignored_parameters.contains(parameter.as_str()))
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
    if continuation.is_supported() {
        parameters.insert("previous_response_id".to_owned());
    }
    if background {
        parameters.insert("background".to_owned());
    }
    if prompt_cache_key {
        parameters.insert("prompt_cache_key".to_owned());
    } else {
        parameters.remove("prompt_cache_key");
    }
    if protocol == ApiProtocol::Responses && !response_includes.is_empty() {
        parameters.insert("include".to_owned());
    } else {
        parameters.remove("include");
    }
    parameters.into_iter().collect()
}

/// Returns whether the current Bridge request converter can represent a parameter completely.
fn bridge_parameter_allowed(protocol: ApiProtocol, parameter: &str) -> bool {
    GenerationRequestField::from_wire(protocol, parameter)
        .is_some_and(|field| field.bridge_representable(protocol))
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
