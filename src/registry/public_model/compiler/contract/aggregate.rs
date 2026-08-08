//! Conservative aggregation of per-Route Public Model contract contributions.
//!
//! This module intersects only capabilities guaranteed by every executable candidate. It owns
//! public model/interface aggregation and conversion of the single Embeddings profile; Route-level
//! contribution derivation remains in the parent module.

use std::collections::BTreeSet;

use crate::{
    core::{AudioTask, EmbeddingsCapabilities},
    registry::ModelContextLength,
};

use super::RouteContractContribution;
use crate::registry::public_model::{
    AudioInputInterfaceCapabilities, AudioOutputInterfaceCapabilities, ContextWindow,
    EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities, EmbeddingInterfaceCapabilities,
    EmbeddingLimits, ImageInputInterfaceCapabilities, InterfaceReasoningCapabilities,
    ModelCapabilities, ModelInterfaceCapabilities, ModelModalities, ModelReasoningCapabilities,
    MultimodalInputCapabilities, MultimodalOutputCapabilities, ReasoningOutputMode,
    StateCapabilities, StructuredOutputCapabilities, StructuredOutputMode, SupportState,
    ToolCapabilities, ToolChoiceMode, ToolType,
};

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

/// Projects the single validated Native Embeddings candidate into its typed public contract.
pub(crate) fn aggregate_embedding_interface<'a>(
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
pub(crate) fn aggregate_interface<'a>(
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
    let image_input = ImageInputInterfaceCapabilities::intersection(
        contributions.iter().map(|value| value.image_input.as_ref()),
    );
    let audio_input = AudioInputInterfaceCapabilities::intersection(
        contributions.iter().map(|value| value.audio_input.as_ref()),
    );
    let voice_conditioning = AudioInputInterfaceCapabilities::intersection(
        contributions
            .iter()
            .map(|value| value.voice_conditioning.as_ref()),
    );
    let audio_output = AudioOutputInterfaceCapabilities::intersection(
        contributions
            .iter()
            .map(|value| value.audio_output.as_ref()),
    );
    let audio_task = intersect_audio_task(contributions.iter().map(|value| value.audio_task));
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
        multimodal_input: MultimodalInputCapabilities {
            image: image_input,
            audio: audio_input,
            voice_conditioning,
        },
        multimodal_output: MultimodalOutputCapabilities {
            audio: audio_output,
        },
        audio_task,
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

/// Publishes one audio task only when every executable Route exposes the same task identity.
fn intersect_audio_task(values: impl Iterator<Item = Option<AudioTask>>) -> Option<AudioTask> {
    let mut values = values;
    let first = values.next()?;
    values
        .all(|value| value == first)
        .then_some(first)
        .flatten()
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
pub(crate) fn aggregate_model_capabilities(
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
pub(crate) fn intersect_optional_string<'a>(
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
