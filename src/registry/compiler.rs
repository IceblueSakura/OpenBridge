//! 静态注册表的校验与运行时快照编译。

use std::collections::{BTreeMap, BTreeSet};

use crate::config::BootstrapConfig;

use super::{
    CredentialPoolBinding, ModelInfo, PublicModel, RegistryConfig, RegistryError, RegistryVersion,
    Route, RouteMode, RuntimeRegistry, UpstreamApi, UpstreamTarget,
    validation::{
        apply_model_rules, normalize_endpoint_base, validate_model_config,
        validate_reasoning_level_mappings,
    },
};

/// 校验完整 registry 定义并构造请求路径只读的运行时 snapshot。
///
/// 校验阶段拒绝未知引用、重复 id、能力越权、非安全 endpoint 和不一致的模型收窄规则；
/// 成功后返回值不再依赖运行时配置注册新 provider 或 target。
pub fn build_registry(
    bootstrap: BootstrapConfig,
    definition: RegistryConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    // 校验版本并建立 canonical model 索引。
    if definition.version.trim().is_empty() {
        return Err(RegistryError::BlankVersion);
    }

    let mut models = BTreeMap::new();
    for model in definition.models {
        validate_model_config(&model)?;
        let id = model.id.clone();
        let resolved = ModelInfo {
            id: id.clone(),
            name: model.name,
            description: model.description,
            context_length: model.context_length,
            mode: model.mode,
            input_modalities: model.input_modalities,
            output_modalities: model.output_modalities,
            supported_parameters: model.supported_parameters,
            reasoning: model.reasoning,
            reasoning_levels: model.reasoning_levels,
        };
        if models.insert(id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "model",
                id,
            });
        }
    }

    // 校验 credential pool 的 Provider 归属与认证类型。
    let mut credential_pools = BTreeMap::new();
    for pool in definition.credential_pools {
        if pool.id.trim().is_empty() {
            return Err(RegistryError::BlankCredentialPoolId);
        }
        if !pool.provider.accepts_credential_kind(pool.kind) {
            return Err(RegistryError::UnsupportedCredentialPoolKind {
                credential_pool: pool.id,
            });
        }
        let resolved = CredentialPoolBinding {
            id: pool.id.clone(),
            provider: pool.provider,
            kind: pool.kind,
        };
        if credential_pools.insert(pool.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "credential pool",
                id: pool.id,
            });
        }
    }

    // 校验 target、pool 引用、endpoint 和 Upstream API，并解析模型收窄规则。
    let mut upstream_targets = BTreeMap::new();
    for target in definition.upstream_targets {
        let credential_pool = credential_pools
            .get(&target.credential_pool)
            .ok_or_else(|| RegistryError::UnknownReference {
                entity: "upstream target",
                id: target.id.clone(),
                target: "credential pool",
                reference: target.credential_pool.clone(),
            })?;
        if credential_pool.provider() != target.provider {
            return Err(RegistryError::CredentialPoolProviderMismatch {
                upstream_target: target.id,
                credential_pool: target.credential_pool,
            });
        }
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
        if target.request_timeout.is_zero() {
            return Err(RegistryError::InvalidRequestTimeout {
                upstream_target: target.id,
            });
        }
        if target.upstream_apis.is_empty() {
            return Err(RegistryError::EmptyUpstreamTarget {
                upstream_target: target.id,
            });
        }
        let endpoint_base = normalize_endpoint_base(&target.base_url).ok_or_else(|| {
            RegistryError::InvalidBaseUrl {
                upstream_target: target.id.clone(),
            }
        })?;
        let mut upstream_apis = BTreeMap::new();
        for upstream_api in target.upstream_apis {
            if upstream_api.protocol != upstream_api.capabilities.protocol() {
                return Err(RegistryError::UpstreamApiProtocolMismatch {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
            if upstream_api.upstream_model.trim().is_empty() {
                return Err(RegistryError::BlankUpstreamModel {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
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
            if !upstream_api
                .capabilities
                .is_subset_of(target.provider.capabilities())
            {
                return Err(RegistryError::CapabilityElevation {
                    upstream_target: target.id,
                    upstream_api: upstream_api.id,
                });
            }
            let api_key = format!("{}/{}", target.id, upstream_api.id);
            let mapping_config = upstream_api.model_rules.reasoning_level_mappings.clone();
            let effective_model =
                apply_model_rules(model.clone(), &api_key, upstream_api.model_rules)?;
            let reasoning_level_mappings =
                validate_reasoning_level_mappings(&api_key, &effective_model, mapping_config)?;
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

    // 校验 route 引用及 native 协议一致性。
    let mut routes = BTreeMap::new();
    for route in definition.routes {
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
        if route.mode == RouteMode::Native && route.downstream_protocol != upstream_api.protocol() {
            return Err(RegistryError::NativeRouteProtocolMismatch { route: route.id });
        }
        if route.mode == RouteMode::Bridged && route.downstream_protocol == upstream_api.protocol()
        {
            return Err(RegistryError::BridgedRouteProtocolMatch { route: route.id });
        }
        let resolved = Route {
            upstream_target: route.upstream_target,
            upstream_api: route.upstream_api,
            downstream_protocol: route.downstream_protocol,
            mode: route.mode,
        };
        if routes.insert(route.id.clone(), resolved).is_some() {
            return Err(RegistryError::DuplicateId {
                entity: "route",
                id: route.id,
            });
        }
    }

    // 校验 Public Model 的 route 顺序、唯一性和完整引用。
    let mut public_models = BTreeMap::new();
    for public_model in definition.public_models {
        if public_model.routes.is_empty() {
            return Err(RegistryError::EmptyPublicModel {
                public_model: public_model.name,
            });
        }
        let mut seen = BTreeSet::new();
        for route in &public_model.routes {
            if !seen.insert(route) {
                return Err(RegistryError::DuplicatePublicModelRoute {
                    public_model: public_model.name,
                    route: route.clone(),
                });
            }
            if !routes.contains_key(route) {
                return Err(RegistryError::UnknownReference {
                    entity: "public model",
                    id: public_model.name,
                    target: "route",
                    reference: route.clone(),
                });
            }
        }
        if public_models
            .insert(
                public_model.name.clone(),
                PublicModel {
                    routes: public_model.routes,
                },
            )
            .is_some()
        {
            return Err(RegistryError::DuplicateId {
                entity: "public model",
                id: public_model.name,
            });
        }
    }

    // 固化所有解析结果为请求路径只读 snapshot。
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
