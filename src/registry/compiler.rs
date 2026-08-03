//! Validates the static registry at startup and compiles it into a read-only snapshot for the request path.
//!
//! Compilation follows registry dependencies: canonical Models, credential pools, Upstream Targets and
//! APIs, Routes, and finally Public Models. This module accepts only compile-time Provider, endpoint,
//! credential-pool, and capability definitions; business requests cannot inject upstream URLs, credentials,
//! or capabilities through this path. Each stage validates references and boundaries before writing to a
//! runtime index; a failure at any stage returns no partial snapshot.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::BootstrapConfig;

use super::{
    CredentialPoolBinding, ModelInfo, RegistryConfig, RegistryError, RegistryVersion, Route,
    RouteMode, RuntimeRegistry, UpstreamApi, UpstreamTarget,
    public_model::{PublicRouteBinding, compile_public_model},
    validation::{
        apply_model_rules, normalize_endpoint_base, validate_model_config,
        validate_public_model_config, validate_reasoning_level_mappings,
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

    // Resolve target pool/model dependencies, validate the endpoint and timeout, and compile all Upstream APIs.
    let mut upstream_targets = BTreeMap::new();
    for target in definition.upstream_targets {
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
        if credential_pool.provider() != target.provider {
            return Err(RegistryError::CredentialPoolProviderMismatch {
                upstream_target: target.id,
                credential_pool: target.credential_pool,
            });
        }

        // Resolve the canonical Model as the model-fact baseline for every Upstream API under this target.
        let model =
            models
                .get(&target.model)
                .cloned()
                .ok_or_else(|| RegistryError::UnknownReference {
                    entity: "upstream target",
                    id: target.id.clone(),
                    target: "real model",
                    reference: target.model.clone(),
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

        // Normalize and validate the endpoint base at startup so the request path receives only a safe HTTPS base and path prefix.
        let endpoint_base = normalize_endpoint_base(&target.base_url).ok_or_else(|| {
            RegistryError::InvalidBaseUrl {
                upstream_target: target.id.clone(),
            }
        })?;
        let mut upstream_apis = BTreeMap::new();
        for upstream_api in target.upstream_apis {
            // Confirm that the protocol tag matches the capability variant, preventing upstream capabilities from being interpreted under the wrong protocol.
            if upstream_api.protocol != upstream_api.capabilities.protocol() {
                return Err(RegistryError::UpstreamApiProtocolMismatch {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }

            // Require a non-blank model ID for the upstream request; the Provider adapter writes this value into the egress request.
            if upstream_api.upstream_model.trim().is_empty() {
                return Err(RegistryError::BlankUpstreamModel {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }

            // Allow only endpoint profiles declared by the Provider at compile time; configuration cannot expand the egress target shape.
            if !target
                .provider
                .accepts_endpoint_profile(&upstream_api.endpoint_profile)
            {
                return Err(RegistryError::UnsupportedEndpointProfile {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                    profile: upstream_api.endpoint_profile,
                });
            }

            // Restrict Upstream API capabilities to the Provider capability ceiling; the registry cannot self-grant unimplemented capabilities.
            if !upstream_api
                .capabilities
                .is_subset_of(target.provider.capabilities())
            {
                return Err(RegistryError::CapabilityElevation {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }

            // Build the model-rule validation context from the target/API identity; this string is not a credential key.
            let api_key = format!("{}/{}", target.id, upstream_api.id);

            // Preserve the original reasoning mappings; model-rule application consumes the configuration, then the mappings are checked against the narrowed model.
            let mapping_config = upstream_api.model_rules.reasoning_level_mappings.clone();

            // Apply the Upstream API model rules to the canonical Model, allowing only narrower confirmed model facts.
            let effective_model =
                apply_model_rules(model.clone(), &api_key, upstream_api.model_rules)?;

            // Confirm that reasoning-level mappings still match the narrowed model and satisfy target-protocol wire-value constraints.
            let reasoning_level_mappings =
                validate_reasoning_level_mappings(&api_key, &effective_model, mapping_config)?;

            // Assemble the validated model, protocol, transport, capability, and state-affinity facts into the runtime API.
            let resolved = UpstreamApi {
                protocol: upstream_api.protocol,
                model: effective_model,
                upstream_model: upstream_api.upstream_model,
                endpoint_profile: upstream_api.endpoint_profile,
                transport: upstream_api.transport,
                capabilities: upstream_api.capabilities,
                state_affinity: upstream_api.state_affinity,
                reasoning_level_mappings,
            };

            // Build a unique Upstream API index within the target so API IDs remain unambiguous.
            if upstream_apis
                .insert(upstream_api.id.clone(), resolved)
                .is_some()
            {
                return Err(RegistryError::DuplicateUpstreamApi {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
        }

        // Assemble the normalized endpoint, credential-pool binding, resource policy, and Upstream API index into the runtime target.
        let resolved = UpstreamTarget {
            id: target.id.clone(),
            kind: target.provider,
            credential_pool: target.credential_pool,
            model_id: target.model,
            endpoint_base,
            quota_scope: target.quota_scope,
            fault_domain: target.fault_domain,
            request_timeout: target.request_timeout,
            enabled: target.enabled,
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

    // Resolve Route references and validate the relationship between Native/Bridged modes and the two protocols.
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
        let upstream_api = target.upstream_api(&route.upstream_api).ok_or_else(|| {
            RegistryError::UnknownReference {
                entity: "route",
                id: route.id.clone(),
                target: "upstream API",
                reference: format!("{}/{}", route.upstream_target, route.upstream_api),
            }
        })?;

        // A Native Route must keep the downstream and upstream protocols identical; an identical pair should not pass through the converter.
        if route.mode == RouteMode::Native && route.downstream_protocol != upstream_api.protocol() {
            return Err(RegistryError::NativeRouteProtocolMismatch { route: route.id });
        }

        // A Bridged Route must connect different protocols; an identical pair must use Native mode to avoid hidden conversion.
        if route.mode == RouteMode::Bridged && route.downstream_protocol == upstream_api.protocol()
        {
            return Err(RegistryError::BridgedRouteProtocolMatch { route: route.id });
        }

        // Store only stable references, the downstream protocol, and the handling mode in the Route; the runtime indexes retain target/API ownership.
        let resolved = Route {
            upstream_target: route.upstream_target,
            upstream_api: route.upstream_api,
            downstream_protocol: route.downstream_protocol,
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
            let upstream_api = target.upstream_api(route.upstream_api()).ok_or_else(|| {
                RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.id.clone(),
                    target: "upstream API",
                    reference: format!("{}/{}", route.upstream_target(), route.upstream_api()),
                }
            })?;

            // Collect the Route, Upstream API, and target-enabled snapshot while preserving the Public Model candidate order.
            bindings.push(PublicRouteBinding {
                route,
                upstream_api,
                target_enabled: target.enabled(),
            });
        }

        // Compile the client-visible contract from the complete bindings; publish only the conservative intersection of executable Route capabilities.
        let id = public_model.id.clone();
        let resolved = compile_public_model(public_model, &bindings);

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
        credential_pools,
        upstream_targets,
        routes,
        public_models,
    })
}
