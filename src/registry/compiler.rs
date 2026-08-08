//! Validates the static registry at startup and compiles it into a read-only snapshot for the request path.
//!
//! Compilation follows registry dependencies: canonical Models, credential pools, Upstream Targets and
//! APIs, Routes, and finally Public Models. This module accepts only compile-time Provider, endpoint,
//! credential-pool, and capability definitions; business requests cannot inject upstream URLs, credentials,
//! or capabilities through this path. Each stage validates references and boundaries before writing to a
//! runtime index; a failure at any stage returns no partial snapshot.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::config::BootstrapConfig;

use super::{
    CredentialPoolBinding, ModelInfo, ModelMode, ProviderInstance, RegistryConfig, RegistryError,
    RegistryVersion, Route, RouteMode, RuntimeRegistry, UpstreamApi, UpstreamTarget,
    public_model::{PublicRouteBinding, compile_public_model},
    validation::{
        apply_model_rules, normalize_endpoint_base, validate_model_config,
        validate_namespaced_model_id, validate_public_model_config,
        validate_reasoning_level_mappings,
    },
};

/// Validates a complete `RegistryConfig` at startup and builds the read-only `RuntimeRegistry` snapshot used by the request path.
///
/// Resolves canonical Models, credential pools, Upstream Targets/APIs, Routes, and Public Models in
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
            context_length: model.context_length,
            mode: model.mode,
            input_modalities: model.input_modalities,
            output_modalities: model.output_modalities,
            tokenizer: model.tokenizer,
            knowledge_cutoff: model.knowledge_cutoff,
            supported_parameters: model.supported_parameters,
            reasoning: model.reasoning,
            reasoning_levels: model.reasoning_levels,
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

        // Require a finite, non-zero timeout for each upstream request on this target.
        if target.request_timeout.is_zero() {
            return Err(RegistryError::InvalidRequestTimeout {
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
            let upstream_operation = upstream_api.capabilities.operation();

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
                    detail: "required streaming needs an enabled generation streaming capability",
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

            // Validate the complete Embeddings profile before capability comparison or public projection.
            if let Some(capabilities) = upstream_api.capabilities.embeddings() {
                capabilities.validate().map_err(|detail| {
                    RegistryError::InvalidEmbeddingsCapabilities {
                        upstream_target: target.id.clone(),
                        upstream_operation,
                        detail,
                    }
                })?;
                if model.mode() != Some(ModelMode::Embedding) {
                    return Err(RegistryError::EmbeddingsModelTaskMismatch {
                        upstream_target: target.id,
                        upstream_operation,
                    });
                }
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

            // Build the model-rule validation context from the target/operation identity; this string is not a credential key.
            let api_key = format!("{}/{upstream_operation}", target.id);

            // Preserve the original reasoning mappings; model-rule application consumes the configuration, then the mappings are checked against the narrowed model.
            let mapping_config = upstream_api.model_rules.reasoning_level_mappings.clone();

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
                model: effective_model,
                upstream_model: upstream_api.upstream_model,
                capabilities: upstream_api.capabilities,
                streaming_policy: upstream_api.streaming_policy,
                state_affinity: upstream_api.state_affinity,
                reasoning_level_mappings,
            };

            // Build a unique typed operation index within the target.
            if upstream_apis.insert(upstream_operation, resolved).is_some() {
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
            provider_model_id: target.provider_model,
            quota_scope: target.quota_scope,
            fault_domain: target.fault_domain,
            request_timeout: target.request_timeout,
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

    // Resolve Route references and validate Native operation identity and generation-only Bridge directions.
    let mut routes = BTreeMap::new();
    for route in definition.routes {
        // Resolve the target first, then resolve the Upstream API from that target's local index.
        let target = upstream_targets
            .get(&route.upstream_target)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "route",
                id: route.id.clone(),
                target: "upstream target",
                reference: route.upstream_target.clone(),
            })?;
        let upstream_api = target
            .upstream_api(route.upstream_operation)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "route",
                id: route.id.clone(),
                target: "upstream operation",
                reference: format!("{}/{}", route.upstream_target, route.upstream_operation),
            })?;

        // A Native Route must keep the downstream and upstream operations identical.
        if route.mode == RouteMode::Native && route.downstream_operation != upstream_api.operation()
        {
            return Err(RegistryError::NativeRouteOperationMismatch { route: route.id });
        }

        // A Bridged Route must connect the two distinct generation protocols; Embeddings has no Bridge.
        if route.mode == RouteMode::Bridged
            && (route.downstream_operation.api_protocol().is_none()
                || upstream_api.api_protocol().is_none()
                || route.downstream_operation == upstream_api.operation())
        {
            return Err(RegistryError::InvalidBridgedRouteOperations { route: route.id });
        }

        // Store only stable references, the downstream operation, and the handling mode.
        let resolved = Route {
            upstream_target: route.upstream_target,
            upstream_operation: route.upstream_operation,
            downstream_operation: route.downstream_operation,
            mode: route.mode,
        };

        // Build a unique Route index; the Public Model will preserve candidate priority separately in a Vec.
        if routes.insert(route.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "route",
                id: route.id,
            });
        }
    }

    // Validate Public Model metadata and Route candidate order, then compile the complete binding for each candidate.
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

        // Use a local set to reject duplicate candidates while a Vec preserves the configured Route priority.
        let mut seen = BTreeSet::new();
        let mut bindings = Vec::with_capacity(public_model.routes.len());
        let mut embedding_candidates = 0_usize;
        for route_id in &public_model.routes {
            // Reject a repeated Route while preserving the configured order.
            if !seen.insert(route_id) {
                return Err(RegistryError::DuplicatePublicModelRoute {
                    public_model: public_model.id,
                    route: route_id.clone(),
                });
            }

            // Resolve the Route reference so the Public Model combines only Routes that passed base validation.
            let route = routes
                .get(route_id)
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.id.clone(),
                    target: "route",
                    reference: route_id.clone(),
                })?;

            // Resolve the target referenced by the Route to capture the target-owned enabled state.
            let target = upstream_targets
                .get(route.upstream_target())
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.id.clone(),
                    target: "upstream target",
                    reference: route.upstream_target().to_owned(),
                })?;

            // Resolve the Upstream API within that target to provide complete upstream facts for the Public Model capability intersection.
            let upstream_api =
                target
                    .upstream_api(route.upstream_operation())
                    .ok_or_else(|| RegistryError::UnknownReference {
                        entity: "public model",
                        id: public_model.id.clone(),
                        target: "upstream operation",
                        reference: format!(
                            "{}/{}",
                            route.upstream_target(),
                            route.upstream_operation()
                        ),
                    })?;

            // Keep the initial Embeddings execution interface to one statically executable Native candidate.
            if target.enabled()
                && upstream_api.capabilities().enabled()
                && route.downstream_operation() == crate::core::OperationKind::EmbeddingsCreate
            {
                embedding_candidates += 1;
                if embedding_candidates > 1 {
                    return Err(RegistryError::MultipleEmbeddingsCandidates {
                        public_model: public_model.id,
                    });
                }
            }

            // Collect the Route, Upstream API, and target-enabled snapshot while preserving the Public Model candidate order.
            bindings.push(PublicRouteBinding {
                route_id: route_id.clone(),
                route,
                upstream_api,
                target_enabled: target.enabled(),
            });
        }

        // Compile the client-visible contract from the complete bindings; publish only the conservative intersection of executable Route capabilities.
        let id = public_model.id.clone();
        let resolved = compile_public_model(
            public_model,
            &bindings,
            bootstrap.limits().max_json_response_body_bytes(),
        )?;

        // Build a unique Public Model index so one downstream model ID cannot map to multiple contracts.
        if public_models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "public model",
                id,
            });
        }
    }

    // Assemble the complete request-path read-only snapshot only after all entities, references, and capability boundaries pass validation.
    Ok(RuntimeRegistry {
        version: RegistryVersion(definition.version),
        bootstrap,
        models,
        provider_instances,
        credential_pools,
        upstream_targets,
        routes,
        public_models,
    })
}
