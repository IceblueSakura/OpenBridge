//! Conservative aggregation of per-Route Public Model contract contributions.
//!
//! This module intersects only capabilities guaranteed by every executable candidate. It owns
//! public model/interface aggregation and conversion of the single Embeddings profile; Route-level
//! contribution derivation remains in the parent module.

use std::collections::BTreeSet;

use crate::{
    core::{EmbeddingsCapabilities, StructuredOutputProfile},
    registry::{CanonicalTaskKind, ModelContextLength},
};

use super::RouteContractContribution;
use crate::registry::public_model::execution::PublicContinuationContract;
use crate::registry::public_model::{
    AudioInterfaceCapabilities, ContextWindow, EmbeddingDimensionCapabilities,
    EmbeddingEncodingCapabilities, EmbeddingInterfaceCapabilities, EmbeddingLimits,
    ImageInputInterfaceCapabilities, InterfaceReasoningCapabilities, ModelCapabilities,
    ModelInterfaceCapabilities, ModelModalities, ModelReasoningCapabilities, ModelTask,
    ReasoningOutputMode, StateCapabilities, SupportState, ToolCapabilities, ToolType,
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

/// Intersects complete structured-output profiles and closes any absent or empty result.
fn intersect_structured_outputs(
    values: impl Iterator<Item = Option<StructuredOutputProfile>>,
) -> Option<StructuredOutputProfile> {
    let mut values = values;
    let first = values.next().flatten()?;
    values.try_fold(first, |intersection, profile| {
        intersection.intersection(profile?)
    })
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
) -> Result<
    (
        Option<ModelInterfaceCapabilities>,
        PublicContinuationContract,
    ),
    (),
> {
    let contributions = contributions.collect::<Vec<_>>();
    if contributions.is_empty() {
        return Ok((None, PublicContinuationContract::Unsupported));
    }

    // Compute conservative intersections for scalars, sets, closed profiles, and reasoning output.
    let context_window =
        ContextWindow::intersection(contributions.iter().map(|value| &value.context_window));
    let modalities =
        ModelModalities::intersection(contributions.iter().map(|value| &value.modalities));
    let image_input = ImageInputInterfaceCapabilities::intersection(
        contributions.iter().map(|value| value.image_input.as_ref()),
    );
    let audio = AudioInterfaceCapabilities::intersection(
        contributions.iter().map(|value| value.audio.as_ref()),
    )?;
    let continuation = aggregate_continuation(&contributions);
    let structured_outputs =
        intersect_structured_outputs(contributions.iter().map(|value| value.structured_outputs));
    let mut supported_parameters = intersect_sets(
        contributions
            .iter()
            .map(|value| value.interface_parameters.as_slice()),
    );
    if !continuation.is_supported() {
        supported_parameters.retain(|parameter| parameter != "previous_response_id");
    }
    if structured_outputs.is_none() {
        supported_parameters.retain(|parameter| {
            !matches!(
                parameter.as_str(),
                "response_format" | "structured_outputs" | "text"
            )
        });
    }
    let streaming = SupportState::intersection(contributions.iter().map(|value| value.streaming));
    let non_streaming =
        SupportState::intersection(contributions.iter().map(|value| value.non_streaming));
    let function_tools =
        SupportState::intersection(contributions.iter().map(|value| value.function_tools));
    let function_tool_choice_modes = intersect_sets(
        contributions
            .iter()
            .map(|value| value.function_tool_choice_modes.as_slice()),
    );
    let tool_strict_schema =
        SupportState::intersection(contributions.iter().map(|value| value.tool_strict_schema));
    let parallel_tool_calls =
        SupportState::intersection(contributions.iter().map(|value| value.parallel_tool_calls));
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

    // Build stable tool and state subobjects beside the already closed capability profiles.
    let capabilities = ModelInterfaceCapabilities {
        context_window,
        modalities,
        image_input,
        audio,
        supported_parameters,
        streaming,
        non_streaming,
        system_messages: SupportState::intersection(
            contributions.iter().map(|value| value.system_messages),
        ),
        tools: ToolCapabilities {
            support: function_tools,
            types: function_tools
                .is_supported()
                .then_some(ToolType::Function)
                .into_iter()
                .collect(),
            tool_choice_modes: function_tool_choice_modes,
            parallel_calls: parallel_tool_calls,
            strict_schema: tool_strict_schema,
        },
        structured_outputs,
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
            previous_response_id: SupportState::from_bool(continuation.is_supported()),
            background: SupportState::intersection(
                contributions.iter().map(|value| value.background),
            ),
        },
    };
    Ok((Some(capabilities), continuation))
}

/// Exposes continuation only when every Route supports it and one target/API is the unique issuer.
fn aggregate_continuation(
    contributions: &[&RouteContractContribution],
) -> PublicContinuationContract {
    // Require the first Route to carry a statically known issuer before comparing the full set.
    let Some(first_issuer) = contributions
        .first()
        .and_then(|contribution| contribution.continuation.issuer())
    else {
        return PublicContinuationContract::Unsupported;
    };

    // Publish continuation only when every Route carries the exact same Target/API identity.
    if contributions
        .iter()
        .all(|contribution| contribution.continuation.issuer() == Some(first_issuer))
    {
        PublicContinuationContract::supported(first_issuer.clone())
    } else {
        PublicContinuationContract::Unsupported
    }
}

/// Aggregates Public Model model capabilities without mixing in Provider or Route identity.
pub(crate) fn aggregate_model_capabilities(
    contributions: &[RouteContractContribution],
    canonical_task: Option<CanonicalTaskKind>,
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
        tasks: canonical_task.map_or_else(Vec::new, ModelTask::from_canonical),
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
