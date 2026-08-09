//! Startup compilation of Public Model contracts and execution interfaces.
//!
//! Compilation includes only statically executable Route bindings, derives each operation contract
//! from the same candidates used by planning, and freezes a topology-free public projection.

use crate::{
    core::OperationKind,
    registry::{PublicModelConfig, RegistryError, Route, UpstreamApi},
};

use super::{
    MODEL_INFO_SCHEMA_VERSION, PublicModelInfo, StandardModel,
    execution::{
        ModelExecutionInterface, ModelExecutionInterfaces, PublicModel, RouteExecutionCandidate,
    },
};

mod contract;
mod embedding_budget;

use contract::{
    RouteContractContribution, aggregate_embedding_interface, aggregate_interface,
    aggregate_model_capabilities, intersect_optional_string,
};
use embedding_budget::constrain_embedding_response_budget;

/// Validated Route binding used to compile one Public Model's static execution interfaces.
pub(in crate::registry) struct PublicRouteBinding<'a> {
    pub(in crate::registry) route_id: String,
    pub(in crate::registry) route: &'a Route,
    pub(in crate::registry) upstream_api: &'a UpstreamApi,
    pub(in crate::registry) target_enabled: bool,
}

/// Compiles a fixed Public Model without deployment details from the complete Route set.
///
/// Returns an error when an Embeddings interface cannot fit one worst-case valid result within
/// the configured JSON response budget.
pub(in crate::registry) fn compile_public_model(
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
                .map(|contribution| contribution.model_description()),
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
                streaming_policy: binding.upstream_api.streaming_policy(),
                ignored_parameters: binding
                    .upstream_api
                    .ignored_generation_parameters()
                    .collect(),
            },
            contribution: RouteContractContribution::from_binding(binding),
        })
    }
}
