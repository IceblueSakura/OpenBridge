//! Startup compilation of Public Model contracts and execution interfaces.
//!
//! Compilation includes only statically executable Route bindings, derives each operation contract
//! from the same candidates used by planning, and freezes a topology-free public projection.

use std::collections::BTreeSet;

use crate::{
    config::RuntimeLimits,
    core::{OperationKind, ResponseIncludePolicy},
    registry::{
        CanonicalTaskKind, PublicModelConfig, ReasoningLevelPolicy, RegistryError, Route,
        UpstreamApi,
    },
};

use super::{
    MODEL_INFO_SCHEMA_VERSION, PublicModelInfo, StandardModel,
    execution::{
        ModelExecutionInterface, ModelExecutionInterfaces, OperationExecutionContract,
        OperationInterfaceIndexError, OperationResponseBudget, PublicContinuationContract,
        PublicModel, RouteExecutionCandidate,
    },
};

mod aggregate;
mod contribution;
mod embedding_budget;

use aggregate::{
    aggregate_embedding_interface, aggregate_images_interface, aggregate_interface,
    aggregate_model_capabilities, intersect_optional_string,
};
use contribution::{RouteContractContribution, forwarded_response_includes};
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
/// Returns an error when executable Routes disagree on canonical task or interface payload, or
/// when an Embeddings interface cannot fit one worst-case valid result within the JSON budget.
pub(in crate::registry) fn compile_public_model(
    config: PublicModelConfig,
    bindings: &[PublicRouteBinding<'_>],
    limits: &RuntimeLimits,
) -> Result<PublicModel, RegistryError> {
    // Compile static eligibility once so every protocol contract and request plan shares the same candidates.
    let mut candidates = bindings
        .iter()
        .filter_map(PrecompiledRouteCandidate::from_binding)
        .collect::<Vec<_>>();

    // Reject cross-operation canonical task mixtures before operation-specific narrowing.
    let canonical_task = validate_public_model_task(&config.id, &candidates)?;

    // Keep positive reasoning normalization exclusive to generation Public Model definitions.
    validate_reasoning_level_policy(&config.id, config.reasoning_level_policy, bindings)?;

    // Narrow an Embeddings batch contract to what one bounded validated response can always contain.
    constrain_embedding_response_budget(
        &config.id,
        limits.max_json_response_body_bytes(),
        &mut candidates,
    )?;

    // Derive protocol contracts and model facts exclusively from the compiled static candidates.
    let contributions = candidates
        .iter()
        .map(|candidate| candidate.contribution.clone())
        .collect::<Vec<_>>();
    let execution_interfaces = compile_execution_interfaces(
        &config.id,
        config.reasoning_level_policy,
        limits,
        &candidates,
    )?;
    let capabilities = aggregate_model_capabilities(&contributions, canonical_task);
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

/// Compiles unique operation interfaces and rejects an empty same-variant profile intersection.
fn compile_execution_interfaces(
    public_model: &str,
    reasoning_level_policy: ReasoningLevelPolicy,
    limits: &RuntimeLimits,
    candidates: &[PrecompiledRouteCandidate],
) -> Result<ModelExecutionInterfaces, RegistryError> {
    // Derive deterministic operation coverage from the candidates so a future closed variant cannot be omitted from a manual list.
    let operations = candidates
        .iter()
        .map(|candidate| candidate.execution.downstream_operation())
        .collect::<BTreeSet<_>>();
    let mut interfaces = Vec::new();
    for operation in operations {
        if let Some(interface) = compile_execution_interface(
            public_model,
            operation,
            reasoning_level_policy,
            limits,
            candidates
                .iter()
                .filter(|candidate| candidate.execution.downstream_operation() == operation),
        )? {
            interfaces.push(interface);
        }
    }
    ModelExecutionInterfaces::try_from_iter(interfaces).map_err(|error| match error {
        OperationInterfaceIndexError::Duplicate(downstream_operation) => {
            RegistryError::DuplicatePublicModelOperationInterface {
                public_model: public_model.to_owned(),
                downstream_operation,
            }
        }
        OperationInterfaceIndexError::Inconsistent(downstream_operation) => {
            RegistryError::PublicModelInterfaceProfileMismatch {
                public_model: public_model.to_owned(),
                downstream_operation,
            }
        }
    })
}

/// Pairs one operation's conservative capability contract with its fixed static candidates.
fn compile_execution_interface<'a>(
    public_model: &str,
    operation: OperationKind,
    reasoning_level_policy: ReasoningLevelPolicy,
    limits: &RuntimeLimits,
    candidates: impl Iterator<Item = &'a PrecompiledRouteCandidate>,
) -> Result<Option<ModelExecutionInterface>, RegistryError> {
    // Materialize one operation's static candidates without changing their configuration order.
    let candidates = candidates.collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    let task = candidates[0].contribution.canonical_task;
    debug_assert!(
        candidates
            .iter()
            .all(|candidate| candidate.contribution.canonical_task == task)
    );
    let contributions = candidates
        .iter()
        .map(|candidate| candidate.contribution.clone())
        .collect::<Vec<_>>();
    let (contract, continuation, response_budget) = match operation {
        OperationKind::ChatCompletions | OperationKind::Responses => {
            let (capabilities, continuation) =
                aggregate_interface(contributions.iter(), reasoning_level_policy).map_err(
                    |()| RegistryError::PublicModelInterfaceProfileMismatch {
                        public_model: public_model.to_owned(),
                        downstream_operation: operation,
                    },
                )?;
            let capabilities =
                capabilities.ok_or_else(|| RegistryError::PublicModelInterfaceProfileMismatch {
                    public_model: public_model.to_owned(),
                    downstream_operation: operation,
                })?;
            (
                OperationExecutionContract::Generation(Box::new(capabilities)),
                continuation,
                OperationResponseBudget::Generation {
                    max_json_body_bytes: limits.max_json_response_body_bytes(),
                    max_sse_event_bytes: limits.max_sse_event_bytes(),
                },
            )
        }
        OperationKind::EmbeddingsCreate => {
            let capabilities =
                aggregate_embedding_interface(contributions.iter()).ok_or_else(|| {
                    RegistryError::PublicModelInterfaceProfileMismatch {
                        public_model: public_model.to_owned(),
                        downstream_operation: operation,
                    }
                })?;
            (
                OperationExecutionContract::Embeddings(capabilities),
                PublicContinuationContract::Unsupported,
                OperationResponseBudget::Embeddings {
                    max_json_body_bytes: limits.max_json_response_body_bytes(),
                },
            )
        }
        OperationKind::ImagesGenerations => {
            let capabilities =
                aggregate_images_interface(contributions.iter()).ok_or_else(|| {
                    RegistryError::PublicModelInterfaceProfileMismatch {
                        public_model: public_model.to_owned(),
                        downstream_operation: operation,
                    }
                })?;
            (
                OperationExecutionContract::ImagesGenerations(Box::new(capabilities)),
                PublicContinuationContract::Unsupported,
                OperationResponseBudget::ImagesGenerations {
                    max_json_body_bytes: limits.max_json_response_body_bytes(),
                },
            )
        }
    };

    // Freeze the matching planning data beside the contract that was derived from it.
    Ok(Some(ModelExecutionInterface {
        operation,
        task,
        contract,
        continuation,
        response_budget,
        candidates: candidates
            .into_iter()
            .map(|candidate| candidate.execution.clone())
            .collect(),
    }))
}

/// Validates the one canonical task shared by every executable Public Model candidate.
fn validate_public_model_task(
    public_model: &str,
    candidates: &[PrecompiledRouteCandidate],
) -> Result<Option<CanonicalTaskKind>, RegistryError> {
    let Some(first) = candidates
        .first()
        .map(|value| value.contribution.canonical_task)
    else {
        return Ok(None);
    };
    if candidates
        .iter()
        .any(|value| value.contribution.canonical_task != first)
    {
        return Err(RegistryError::PublicModelTaskMismatch {
            public_model: public_model.to_owned(),
        });
    }
    Ok(Some(first))
}

/// Rejects reasoning-level normalization on task-specific non-generation Public Models.
fn validate_reasoning_level_policy(
    public_model: &str,
    policy: ReasoningLevelPolicy,
    bindings: &[PublicRouteBinding<'_>],
) -> Result<(), RegistryError> {
    // Strict input validation is valid for every canonical task.
    if policy == ReasoningLevelPolicy::Strict {
        return Ok(());
    }

    // Inspect every configured binding so disabled targets cannot hide an invalid task policy.
    let has_non_generation = bindings.iter().any(|binding| {
        RouteContractContribution::from_binding(binding).canonical_task
            != CanonicalTaskKind::Generation
    });
    if has_non_generation {
        return Err(RegistryError::PublicModelReasoningPolicyTaskMismatch {
            public_model: public_model.to_owned(),
        });
    }
    Ok(())
}

/// Static candidate and capability input compiled together from one resolved Route binding.
struct PrecompiledRouteCandidate {
    execution: RouteExecutionCandidate,
    contribution: RouteContractContribution,
}

impl PrecompiledRouteCandidate {
    /// Includes only a statically enabled Target and preserves its validated Route and API facts.
    fn from_binding(binding: &PublicRouteBinding<'_>) -> Option<Self> {
        // Reject disabled Targets before either capability aggregation or request planning can see them.
        if !binding.target_enabled {
            return None;
        }

        // Freeze every planning fact that otherwise required a request-time Route/API lookup.
        let execution = RouteExecutionCandidate {
            route_id: binding.route_id.clone(),
            upstream_target_id: binding.route.upstream_target().to_owned(),
            downstream_operation: binding.route.downstream_operation(),
            upstream_api_key: binding.upstream_api.key(),
            mode: binding.route.mode(),
            upstream_model: binding.upstream_api.upstream_model().to_owned(),
            reasoning_output: binding.upstream_api.reasoning_output(),
            streaming_policy: binding.upstream_api.streaming_policy(),
            ignored_parameters: binding
                .upstream_api
                .ignored_generation_parameters()
                .collect(),
            forwarded_response_includes: forwarded_response_includes(
                binding.route,
                binding.upstream_api,
            ),
        };
        let contribution = RouteContractContribution::from_binding(binding);

        // Stop startup if private forwarding escapes acceptance or omission widens beyond policy.
        assert!(
            execution
                .forwarded_response_includes
                .iter()
                .all(|include| contribution.response_includes.contains(include))
        );
        assert!(contribution.response_includes.iter().all(|include| {
            execution.forwarded_response_includes.contains(include)
                || include.policy() == ResponseIncludePolicy::ForwardOrOmit
        }));
        Some(Self {
            execution,
            contribution,
        })
    }
}
