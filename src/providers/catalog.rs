//! Assembles the built-in Model, Provider instance, Upstream Target, Route, and Public Model catalog.

mod embeddings;
mod public_models;
mod route_compiler;
mod routing;

use std::collections::BTreeSet;

use crate::{
    config::BootstrapConfig,
    models,
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialPoolConfig, RegistryConfig, RegistryError, RuntimeRegistry, build_registry,
        build_registry_with_active_pools,
    },
};

use super::{bailian, chatgpt, deepseek, longcat, mimo, nvidia, openai, openrouter};

/// Version identifier for the built-in provider and model registry.
pub const REGISTRY_VERSION: &str = "dev-1";

/// Returns all Model, Provider instance, Upstream Target, Route, and Public Model entries compiled into the binary.
pub fn compiled_config() -> RegistryConfig {
    // Aggregate provider targets and independent Public Model route registrations.
    let routing = routing::compiled_routing();
    RegistryConfig {
        version: REGISTRY_VERSION.to_owned(),
        models: models::compiled_configs(),
        provider_instances: vec![
            openai::provider_instance(),
            longcat::provider_instance(),
            openrouter::provider_instance(),
            deepseek::provider_instance(),
            mimo::provider_instance(),
            chatgpt::provider_instance(),
            nvidia::provider_instance(),
            bailian::provider_instance(),
        ],
        credential_pools: vec![
            credential_pool("openai-primary", ProviderKind::OpenAi),
            credential_pool("longcat-primary", ProviderKind::LongCat),
            credential_pool("openrouter-primary", ProviderKind::OpenRouter),
            credential_pool("deepseek-primary", ProviderKind::DeepSeek),
            credential_pool("mimo-primary", ProviderKind::MiMo),
            credential_pool("nvidia-primary", ProviderKind::Nvidia),
            credential_pool("bailian-primary", ProviderKind::Bailian),
            credential_pool_with_kind(
                "chatgpt-codex",
                ProviderKind::ChatGpt,
                CredentialKind::OAuth2BearerAccessToken,
            ),
        ],
        upstream_targets: [
            openai::upstream_targets(),
            longcat::upstream_targets(),
            openrouter::upstream_targets(),
            deepseek::upstream_targets(),
            mimo::upstream_targets(),
            chatgpt::upstream_targets(),
            nvidia::upstream_targets(),
            bailian::upstream_targets(),
        ]
        .concat(),
        routes: routing.routes,
        public_models: routing.public_models,
    }
}

/// Builds the Provider credential pool populated from the private upstream credential TOML.
fn credential_pool(id: &str, provider: ProviderKind) -> CredentialPoolConfig {
    credential_pool_with_kind(id, provider, CredentialKind::ApiKey)
}

/// Builds one Provider credential pool with an explicitly bounded credential kind.
fn credential_pool_with_kind(
    id: &str,
    provider: ProviderKind,
    kind: CredentialKind,
) -> CredentialPoolConfig {
    CredentialPoolConfig {
        id: id.to_owned(),
        provider,
        kind,
    }
}

/// Validates and builds the built-in registry.
pub fn build_compiled_registry(
    bootstrap: BootstrapConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry(bootstrap, compiled_config())
}

/// Builds the compiled registry while applying startup credential-pool activation to Targets.
pub fn build_compiled_registry_with_active_pools(
    bootstrap: BootstrapConfig,
    active_pool_ids: &BTreeSet<String>,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry_with_active_pools(bootstrap, compiled_config(), active_pool_ids)
}
