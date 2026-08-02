//! 内置 Model、Upstream Target、Route 与 Public Model 的编译目录装配。

mod routing;

use crate::{
    config::BootstrapConfig,
    models,
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialPoolConfig, RegistryConfig, RegistryError, RuntimeRegistry, build_registry,
    },
};

use super::{deepseek, longcat, mimo, openai, openrouter};

/// 当前内置 provider/model registry 的版本标识。
pub const REGISTRY_VERSION: &str = "dev-1";

/// 返回所有编译进二进制的 Model、Upstream Target、Route 与 Public Model。
pub fn compiled_config() -> RegistryConfig {
    // 聚合各 Provider 的 target 与独立的 Public Model route 注册。
    let routing = routing::compiled_routing();
    RegistryConfig {
        version: REGISTRY_VERSION.to_owned(),
        models: models::compiled_configs(),
        credential_pools: vec![
            credential_pool("openai-primary", ProviderKind::OpenAi),
            credential_pool("longcat-primary", ProviderKind::LongCat),
            credential_pool("openrouter-primary", ProviderKind::OpenRouter),
            credential_pool("deepseek-primary", ProviderKind::DeepSeek),
            credential_pool("mimo-primary", ProviderKind::MiMo),
        ],
        upstream_targets: [
            openai::upstream_targets(),
            longcat::upstream_targets(),
            openrouter::upstream_targets(),
            deepseek::upstream_targets(),
            mimo::upstream_targets(),
        ]
        .concat(),
        routes: routing.routes,
        public_models: routing.public_models,
    }
}

/// 构造由私有 upstream credential TOML 提供 secret 的 Provider credential pool。
fn credential_pool(id: &str, provider: ProviderKind) -> CredentialPoolConfig {
    CredentialPoolConfig {
        id: id.to_owned(),
        provider,
        kind: CredentialKind::ApiKey,
    }
}

/// 校验并构造内置 registry。
pub fn build_compiled_registry(
    bootstrap: BootstrapConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry(bootstrap, compiled_config())
}
