//! Assembles the built-in Model, Upstream Target, Route, and Public Model catalog.

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

/// Version identifier for the built-in provider and model registry.
pub const REGISTRY_VERSION: &str = "dev-1";

/// Returns all Model, Upstream Target, Route, and Public Model entries compiled into the binary.
pub fn compiled_config() -> RegistryConfig {
    // Aggregate provider targets and independent Public Model route registrations.
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

/// Builds the Provider credential pool populated from the private upstream credential TOML.
fn credential_pool(id: &str, provider: ProviderKind) -> CredentialPoolConfig {
    CredentialPoolConfig {
        id: id.to_owned(),
        provider,
        kind: CredentialKind::ApiKey,
    }
}

/// Validates and builds the built-in registry.
pub fn build_compiled_registry(
    bootstrap: BootstrapConfig,
) -> Result<RuntimeRegistry, RegistryError> {
    build_registry(bootstrap, compiled_config())
}
