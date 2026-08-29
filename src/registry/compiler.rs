//! Validates the static registry at startup and compiles it into a read-only snapshot for the request path.
//!
//! Compilation follows registry dependencies: canonical Models, credential pools, Upstream Targets and
//! APIs, and finally Public Models with owned Routes. This module accepts only compile-time Provider, endpoint,
//! credential-pool, and capability definitions; business requests cannot inject upstream URLs, credentials,
//! or capabilities through this path. Each stage validates references and boundaries before writing to a
//! runtime index; a failure at any stage returns no partial snapshot.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{
    config::BootstrapConfig,
    core::{
        EmbeddingEncoding, EmbeddingEncodingPolicy, ExecutableAudioProfile,
        GenerationBridgeDirection, OperationKind,
    },
};

use super::{
    CanonicalTaskKind, CredentialPoolBinding, ModelInfo, ProviderInstance, RegistryConfig,
    RegistryError, RegistryVersion, RouteMode, RuntimeRegistry, UpstreamApi,
    UpstreamApiCapabilities, UpstreamApiKey, UpstreamTarget,
    public_model::{PublicRouteBinding, compile_public_model},
    validation::{
        apply_model_rules, normalize_endpoint_base, validate_model_config,
        validate_namespaced_model_id, validate_public_model_config,
        validate_reasoning_level_mappings,
    },
};

/// Validates a complete `RegistryConfig` at startup and builds the read-only `RuntimeRegistry` snapshot used by the request path.
///
/// Resolves canonical Models, credential pools, Upstream Targets/APIs, and Public Models in
/// dependency order. It rejects unknown references, duplicate IDs, capability elevations beyond the
/// Provider boundary, unsafe endpoints, inconsistent protocol modes, and invalid narrowing rules.
///
/// After success, the snapshot contains the resolved registry relationships and `BootstrapConfig`; the
/// request path does not reread static definitions and cannot use this snapshot to dynamically register
/// or rewrite Providers, targets, credentials, or capabilities.
pub fn build_registry(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry_internal(bootstrap, definition, None)
}

/// Builds a registry while applying startup credential-pool activation to Target eligibility.
///
/// The active pool set is a redacted deployment snapshot derived from private startup
/// configuration. It can disable statically registered Targets, but it cannot add Providers,
/// credential pools, Routes, endpoints, or capabilities. Direct callers that do not provide this
/// deployment snapshot retain the static registry behavior through [`build_registry`].
pub fn build_registry_with_active_pools(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
    active_pool_ids: &BTreeSet<String>,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry_internal(bootstrap, definition, Some(active_pool_ids))
}

/// Compiles a registry with an optional startup credential-pool activation snapshot.
fn build_registry_internal(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
    active_pool_ids: Option<&BTreeSet<String>>,
) -> Result<RuntimeRegistry, RegistryError> {
    // Validate the registry version so the resulting snapshot has a reportable, auditable identity.
    if definition.version.trim().is_empty() {
        return Err(RegistryError::BlankVersion);
    }

    // Validate each canonical Model and index it for subsequent target/API resolution.
    let mut models = BTreeMap::new();
    for model in definition.models {
        // Validate model metadata, context limits, parameters, and reasoning declarations.
        validate_model_config(&model)?;

        // Move configuration fields into request-path ModelInfo, keeping the model ID as both key and entity field.
        let id = model.id.clone();
        let resolved = ModelInfo {
            id: id.clone(),
            name: model.name,
            description: model.description,
            tokenizer: model.tokenizer,
            knowledge_cutoff: model.knowledge_cutoff,
            task: model.task,
        };

        // Build a unique model index so later targets cannot reference an ambiguous model.
        if models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "model",
                id,
            });
        }
    }

    // Validate and index trusted Provider deployments before any target can reference an endpoint.
    let mut provider_instances = BTreeMap::new();
    for instance in definition.provider_instances {
        // Require a stable instance identity because targets may share one deployment explicitly.
        if instance.id.trim().is_empty() {
            return Err(RegistryError::BlankProviderInstanceId);
        }

        // Normalize the sole trusted endpoint before storing it in the immutable runtime instance.
        let endpoint_base = normalize_endpoint_base(&instance.base_url).ok_or_else(|| {
            RegistryError::InvalidProviderBaseUrl {
                provider_instance: instance.id.clone(),
            }
        })?;
        let id = instance.id.clone();
        let resolved = Arc::new(ProviderInstance {
            id: id.clone(),
            kind: instance.kind,
            endpoint_base,
        });

        // Build a unique instance index so one target reference resolves to exactly one deployment.
        if provider_instances.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "provider instance",
                id,
            });
        }
    }

    // Validate credential-pool Provider ownership and credential types, compiling only non-sensitive binding data.
    let mut credential_pools = BTreeMap::new();
    for pool in definition.credential_pools {
        // Reject a blank pool ID so target credential references have a stable key.
        if pool.id.trim().is_empty() {
            return Err(RegistryError::BlankCredentialPoolId);
        }

        // Confirm that the Provider adapter explicitly supports this credential kind, preventing cross-Provider credential use.
        if !pool.provider.accepts_credential_kind(pool.kind) {
            return Err(RegistryError::UnsupportedCredentialPoolKind {
                credential_pool: pool.id,
            });
        }

        // Build a runtime object containing only the Provider binding and type; do not place credential secrets in the registry snapshot.
        let resolved = CredentialPoolBinding {
            id: pool.id.clone(),
            provider: pool.provider,
            kind: pool.kind,
        };

        // Build a unique pool index so targets cannot reference an ambiguous credential pool.
        if credential_pools.insert(pool.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "credential pool",
                id: pool.id,
            });
        }
    }

    // Resolve each target's Provider instance, pool, and model dependencies, then compile all Upstream APIs.
    let mut upstream_targets = BTreeMap::new();
    for target in definition.upstream_targets {
        // Resolve the Provider instance so both adapter kind and endpoint come from one trusted deployment.
        let provider_instance = provider_instances
            .get(&target.provider_instance)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "upstream target",
                id: target.id.clone(),
                target: "provider instance",
                reference: target.provider_instance.clone(),
            })?;

        // Resolve the credential-pool reference so the target can consume only a declared pool.
        let credential_pool = credential_pools
            .get(&target.credential_pool)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "upstream target",
                id: target.id.clone(),
                target: "credential pool",
                reference: target.credential_pool.clone(),
            })?;

        // Confirm that the target and credential pool belong to the same Provider, preventing one adapter from receiving another adapter's credential.
        if credential_pool.provider() != provider_instance.kind() {
            return Err(RegistryError::CredentialPoolProviderMismatch {
                upstream_target: target.id,
                credential_pool: target.credential_pool,
            });
        }

        // Validate the canonical and routing identities before resolving model facts.
        validate_namespaced_model_id(&target.canonical_model, "canonical_model")?;
        validate_namespaced_model_id(&target.provider_model, "provider_model")?;
        let canonical_model_name = target
            .canonical_model
            .rsplit_once('/')
            .expect("validated canonical model identity has a namespace")
            .1;
        let expected_provider_model = format!(
            "{}/{}",
            provider_instance.kind().slug(),
            canonical_model_name
        );
        if target.provider_model != expected_provider_model {
            return Err(RegistryError::ProviderModelMismatch {
                upstream_target: target.id,
                provider_model: target.provider_model,
                expected: expected_provider_model,
            });
        }

        // Combine static code eligibility with the redacted startup credential activation state.
        let target_enabled = target.enabled
            && active_pool_ids.is_none_or(|active| active.contains(&target.credential_pool));

        // Resolve the canonical Model as the model-fact baseline for every Upstream API under this target.
        let model = models
            .get(&target.canonical_model)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "upstream target",
                id: target.id.clone(),
                target: "canonical model",
                reference: target.canonical_model.clone(),
            })?;

        // Require every mandatory timeout phase and any optional stream total to be non-zero.
        if !target.timeout_policy.is_valid() {
            return Err(RegistryError::InvalidTimeoutPolicy {
                upstream_target: target.id,
            });
        }

        // Require at least one Native Upstream API; otherwise no Route can use this target.
        if target.upstream_apis.is_empty() {
            return Err(RegistryError::EmptyUpstreamTarget {
                upstream_target: target.id,
            });
        }

        let mut upstream_apis = BTreeMap::new();
        for upstream_api in target.upstream_apis {
            let upstream_key = upstream_api.key;
            let upstream_operation = upstream_key.operation();

            // Establish one authoritative operation/task identity before any operation-specific validation.
            validate_upstream_api_identity(
                &target.id,
                &model,
                upstream_key,
                upstream_api.capabilities,
            )?;

            // Validate streaming-only declarations before they can affect public contracts or request planning.
            let generation_streaming = upstream_api
                .capabilities
                .generation_capabilities()
                .map(|capabilities| capabilities.streaming);
            if upstream_api.streaming_policy.requires_streaming()
                && generation_streaming != Some(true)
            {
                return Err(RegistryError::InvalidUpstreamStreamingPolicy {
                    upstream_target: target.id,
                    upstream_operation,
                    detail: "required streaming needs generation streaming support",
                });
            }
            if upstream_api.streaming_policy.buffers_responses_sse()
                && upstream_operation != crate::core::OperationKind::Responses
            {
                return Err(RegistryError::InvalidUpstreamStreamingPolicy {
                    upstream_target: target.id,
                    upstream_operation,
                    detail: "Responses SSE buffering is valid only for the Responses operation",
                });
            }

            // Validate mutable executable operation payloads before Provider ceiling containment.
            if let Some(capabilities) = upstream_api.capabilities.embeddings() {
                capabilities.validate().map_err(|detail| {
                    RegistryError::InvalidEmbeddingsCapabilities {
                        upstream_target: target.id.clone(),
                        upstream_operation,
                        detail,
                    }
                })?;
            }
            if let Some(capabilities) = upstream_api.capabilities.images_generations() {
                capabilities.validate().map_err(|detail| {
                    RegistryError::InvalidImagesCapabilities {
                        upstream_target: target.id.clone(),
                        upstream_operation,
                        detail,
                    }
                })?;
            }

            // Require a non-blank model ID for the upstream request; the Provider adapter writes this value into the egress request.
            if upstream_api.upstream_model.trim().is_empty() {
                return Err(RegistryError::BlankUpstreamModel {
                    upstream_target: target.id,
                    upstream_operation,
                });
            }

            // Restrict Upstream API capabilities to the Provider capability ceiling; the registry cannot self-grant unimplemented capabilities.
            if !upstream_api
                .capabilities
                .is_subset_of(provider_instance.kind().capabilities())
            {
                return Err(RegistryError::CapabilityElevation {
                    upstream_target: target.id,
                    upstream_operation,
                });
            }

            // Validate task-specific profile details after Provider ceiling containment.
            validate_upstream_api_model_task(
                &target.id,
                &model,
                upstream_key,
                upstream_api.capabilities,
            )?;

            // Build the model-rule validation context from the target/operation identity; this string is not a credential key.
            let api_key = format!("{}/{upstream_operation}", target.id);

            // Preserve the original reasoning mappings; model-rule application consumes the configuration, then the mappings are checked against the narrowed model.
            let mapping_config = upstream_api.model_rules.reasoning_level_mappings.clone();

            // Preserve the validated ordinary-parameter ignore set for final routed egress preparation.
            let ignored_parameters = upstream_api
                .model_rules
                .ignored_parameters
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let serial_tool_calls_only = upstream_api.model_rules.serial_tool_calls_only;
            let embedding_encoding_policy = upstream_api.model_rules.embedding_encoding_policy;

            // Keep Embeddings wire translation scoped to one explicit API and executable encoding pair.
            if embedding_encoding_policy != EmbeddingEncodingPolicy::Preserve {
                let Some(capabilities) = upstream_api.capabilities.embeddings() else {
                    return Err(RegistryError::InconsistentUpstreamApiModelRules {
                        upstream_api: api_key.clone(),
                        detail: "embedding encoding translation requires an Embeddings API",
                    });
                };
                let supports = |encoding| {
                    capabilities.default_encoding == encoding
                        || capabilities
                            .allowed_encodings
                            .is_some_and(|values| values.contains(&encoding))
                };
                if !supports(EmbeddingEncoding::Float) || !supports(EmbeddingEncoding::Base64) {
                    return Err(RegistryError::InconsistentUpstreamApiModelRules {
                        upstream_api: api_key.clone(),
                        detail: "Base64-via-float translation requires float and base64 interface encodings",
                    });
                }
            }

            // Accept serial-only omission only as an explicit narrowing of a function-tool API.
            if serial_tool_calls_only
                && !upstream_api
                    .capabilities
                    .generation_capabilities()
                    .and_then(|capabilities| capabilities.function_tools)
                    .is_some_and(|tools| !tools.parallel_calls)
            {
                return Err(RegistryError::InconsistentUpstreamApiModelRules {
                    upstream_api: api_key.clone(),
                    detail: "serial-only tool control requires non-parallel function tools",
                });
            }

            // Keep generation-only ignore semantics out of the independently typed Embeddings operation.
            if upstream_operation == crate::core::OperationKind::EmbeddingsCreate
                && !ignored_parameters.is_empty()
            {
                return Err(RegistryError::InconsistentUpstreamApiModelRules {
                    upstream_api: api_key,
                    detail: "ignored generation parameters require a Chat or Responses API",
                });
            }

            // Apply the Upstream API model rules to the canonical Model, allowing only narrower confirmed model facts.
            let effective_model =
                apply_model_rules(model.clone(), &api_key, upstream_api.model_rules)?;

            // Keep the Embeddings endpoint parameter contract within the narrowed canonical model ceiling.
            if upstream_api
                .capabilities
                .embeddings()
                .is_some_and(|capabilities| {
                    capabilities.supported_parameters.iter().any(|parameter| {
                        !effective_model
                            .supported_parameters()
                            .iter()
                            .any(|model_parameter| model_parameter == parameter)
                    })
                })
            {
                return Err(RegistryError::InvalidEmbeddingsCapabilities {
                    upstream_target: target.id,
                    upstream_operation,
                    detail: "supported parameters must be declared by the effective model",
                });
            }

            // Confirm that reasoning-level mappings still match the narrowed model and satisfy target-protocol wire-value constraints.
            let reasoning_level_mappings =
                validate_reasoning_level_mappings(&api_key, &effective_model, mapping_config)?;

            // Assemble the validated model, capability, and state-affinity facts into the runtime API.
            let resolved = UpstreamApi {
                key: upstream_key,
                model: effective_model,
                upstream_model: upstream_api.upstream_model,
                capabilities: upstream_api.capabilities,
                streaming_policy: upstream_api.streaming_policy,
                reasoning_level_mappings,
                ignored_parameters,
                serial_tool_calls_only,
                embedding_encoding_policy,
            };

            // Build a unique typed operation/task index within the target.
            if upstream_apis.insert(upstream_key, resolved).is_some() {
                return Err(RegistryError::DuplicateUpstreamOperation {
                    upstream_target: target.id,
                    upstream_operation,
                });
            }
        }

        // Assemble the resolved Provider instance, credential binding, resource policy, and operation index into the runtime target.
        let resolved = UpstreamTarget {
            id: target.id.clone(),
            provider_instance,
            credential_pool: target.credential_pool,
            canonical_model_id: target.canonical_model,
            canonical_task: model.task_kind(),
            provider_model_id: target.provider_model,
            quota_scope: target.quota_scope,
            fault_domain: target.fault_domain,
            timeout_policy: target.timeout_policy,
            enabled: target_enabled,
            upstream_apis,
        };

        // Build the global target index so later Routes can bind only to one validated target.
        if upstream_targets
            .insert(target.id.clone(), resolved)
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "upstream target",
                id: target.id,
            });
        }
    }

    // Validate each Public Model and resolve its owned Route candidates in fixed priority order.
    let mut public_models = BTreeMap::new();
    for public_model in definition.public_models {
        // Validate the Public Model ID, display fields, and lifecycle before processing its candidate Routes.
        validate_public_model_config(&public_model)?;

        // Require at least one candidate Route; otherwise the Public Model has no executable request path.
        if public_model.routes.is_empty() {
            return Err(RegistryError::EmptyPublicModel {
                public_model: public_model.id,
            });
        }

        // Reject structurally duplicate candidates while preserving the configured Route priority.
        let mut seen = BTreeSet::new();
        let mut bindings = Vec::with_capacity(public_model.routes.len());
        let mut embedding_candidates = 0_usize;
        for route in &public_model.routes {
            let identity = (
                route.upstream_target.clone(),
                route.upstream_operation,
                route.downstream_operation,
            );
            if !seen.insert(identity) {
                return Err(RegistryError::DuplicatePublicModelCandidate {
                    public_model: public_model.id.clone(),
                    upstream_target: route.upstream_target.clone(),
                    upstream_operation: route.upstream_operation,
                    downstream_operation: route.downstream_operation,
                });
            }

            // Resolve the target and API directly from this Public Model-owned Route.
            let target = upstream_targets
                .get(&route.upstream_target)
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "public model route",
                    id: public_model.id.clone(),
                    target: "upstream target",
                    reference: route.upstream_target.clone(),
                })?;
            let upstream_api = target
                .upstream_api(UpstreamApiKey::new(
                    route.upstream_operation,
                    target.canonical_task(),
                ))
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "public model route",
                    id: public_model.id.clone(),
                    target: "upstream operation",
                    reference: format!("{}/{}", route.upstream_target, route.upstream_operation),
                })?;
            let mode = derive_route_mode(
                target.canonical_task(),
                route.upstream_operation,
                route.downstream_operation,
            )
            .ok_or_else(|| RegistryError::InvalidRouteOperationPair {
                public_model: public_model.id.clone(),
                upstream_target: route.upstream_target.clone(),
                upstream_operation: route.upstream_operation,
                downstream_operation: route.downstream_operation,
            })?;

            // Keep the initial Embeddings execution interface to one statically selectable Native candidate.
            if target.enabled() && route.downstream_operation == OperationKind::EmbeddingsCreate {
                embedding_candidates += 1;
                if embedding_candidates > 1 {
                    return Err(RegistryError::MultipleEmbeddingsCandidates {
                        public_model: public_model.id,
                    });
                }
            }

            // Collect the resolved Target/API and derived mode in the Public Model candidate order.
            bindings.push(PublicRouteBinding {
                upstream_target: target,
                upstream_api,
                downstream_operation: route.downstream_operation,
                mode,
            });
        }

        // Compile the client-visible contract from the complete bindings; publish only the conservative intersection of executable Route capabilities.
        let id = public_model.id.clone();
        let resolved = compile_public_model(public_model, &bindings, bootstrap.limits())?;

        // Build a unique Public Model index so one downstream model ID cannot map to multiple contracts.
        if public_models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "public model",
                id,
            });
        }
    }

    // Require one usable project fallback only when compilation retained a general Generation interface.
    if public_models
        .values()
        .any(|model| model.is_available() && model.has_general_generation_interface())
        && bootstrap
            .default_instructions()
            .is_none_or(|instructions| instructions.trim().is_empty())
    {
        return Err(RegistryError::MissingDefaultInstructions);
    }

    // Assemble the complete request-path read-only snapshot only after all entities, references, and capability boundaries pass validation.
    Ok(RuntimeRegistry {
        version: RegistryVersion(definition.version),
        bootstrap,
        models,
        provider_instances,
        credential_pools,
        upstream_targets,
        public_models,
    })
}

/// Derives the only valid execution mode from one typed downstream/upstream operation pair.
fn derive_route_mode(
    canonical_task: CanonicalTaskKind,
    upstream_operation: OperationKind,
    downstream_operation: OperationKind,
) -> Option<RouteMode> {
    if upstream_operation == downstream_operation {
        return Some(RouteMode::Native);
    }
    if canonical_task != CanonicalTaskKind::Generation {
        return None;
    }
    match (downstream_operation, upstream_operation) {
        (OperationKind::ChatCompletions, OperationKind::Responses) => Some(
            RouteMode::GenerationBridge(GenerationBridgeDirection::ChatToResponses),
        ),
        (OperationKind::Responses, OperationKind::ChatCompletions) => Some(
            RouteMode::GenerationBridge(GenerationBridgeDirection::ResponsesToChat),
        ),
        _ => None,
    }
}

/// Validates the closed canonical-task and executable-operation compatibility matrix.
fn validate_upstream_api_model_task(
    upstream_target: &str,
    model: &ModelInfo,
    key: UpstreamApiKey,
    capabilities: UpstreamApiCapabilities,
) -> Result<(), RegistryError> {
    let compatible = match capabilities {
        UpstreamApiCapabilities::Embeddings(_) => key.task() == CanonicalTaskKind::Embedding,
        UpstreamApiCapabilities::ImagesGenerations(_) => {
            key.task() == CanonicalTaskKind::ImageGeneration
        }
        UpstreamApiCapabilities::Responses(_) => key.task() == CanonicalTaskKind::Generation,
        UpstreamApiCapabilities::ChatCompletions(capabilities) => {
            match (key.task(), capabilities.media.audio) {
                (CanonicalTaskKind::Generation, None) => true,
                (
                    CanonicalTaskKind::Generation,
                    Some(ExecutableAudioProfile::AudioUnderstanding(_)),
                ) => {
                    model
                        .input_modalities()
                        .is_some_and(|modalities| modalities.contains(&super::InputModality::Audio))
                        && model.output_modalities().is_some_and(|modalities| {
                            modalities.contains(&super::OutputModality::Text)
                        })
                }
                (
                    CanonicalTaskKind::SpeechRecognition,
                    Some(ExecutableAudioProfile::SpeechRecognition(_)),
                )
                | (
                    CanonicalTaskKind::SpeechSynthesis,
                    Some(ExecutableAudioProfile::SpeechSynthesis(_)),
                )
                | (CanonicalTaskKind::VoiceDesign, Some(ExecutableAudioProfile::VoiceDesign(_)))
                | (CanonicalTaskKind::VoiceClone, Some(ExecutableAudioProfile::VoiceClone(_))) => {
                    true
                }
                _ => false,
            }
        }
    };
    if compatible {
        return Ok(());
    }
    Err(RegistryError::UpstreamApiModelTaskMismatch {
        upstream_target: upstream_target.to_owned(),
        upstream_operation: key.operation(),
        canonical_model: model.id().to_owned(),
    })
}

/// Validates the explicit API key before its operation-specific payload is interpreted.
fn validate_upstream_api_identity(
    upstream_target: &str,
    model: &ModelInfo,
    key: UpstreamApiKey,
    capabilities: UpstreamApiCapabilities,
) -> Result<(), RegistryError> {
    if key.operation() == capabilities.operation() && key.task() == model.task_kind() {
        return Ok(());
    }
    Err(RegistryError::UpstreamApiIdentityMismatch {
        upstream_target: upstream_target.to_owned(),
        key,
        profile_operation: capabilities.operation(),
        canonical_task: model.task_kind(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_mode_derivation_keeps_bridges_generation_only() {
        assert_eq!(
            derive_route_mode(
                CanonicalTaskKind::Embedding,
                OperationKind::EmbeddingsCreate,
                OperationKind::EmbeddingsCreate,
            ),
            Some(RouteMode::Native)
        );
        assert_eq!(
            derive_route_mode(
                CanonicalTaskKind::Generation,
                OperationKind::Responses,
                OperationKind::ChatCompletions,
            ),
            Some(RouteMode::GenerationBridge(
                GenerationBridgeDirection::ChatToResponses
            ))
        );
        assert_eq!(
            derive_route_mode(
                CanonicalTaskKind::SpeechSynthesis,
                OperationKind::ChatCompletions,
                OperationKind::Responses,
            ),
            None
        );
        assert_eq!(
            derive_route_mode(
                CanonicalTaskKind::Embedding,
                OperationKind::EmbeddingsCreate,
                OperationKind::ImagesGenerations,
            ),
            None
        );
    }
}
