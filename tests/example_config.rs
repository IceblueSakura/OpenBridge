//! Verifies that example configuration, the compiled model catalog, and default route facts remain consistent.

use openbridge::{
    config::parse_bootstrap_config,
    core::{ApiProtocol, ApiRequest, OperationKind, ReasoningOutput},
    identity::UserConfigPath,
    pipeline::{analyze_request, plan_request},
    provider::{CredentialKind, ProviderAdapter, ProviderKind},
    providers::{build_compiled_registry, compiled_config},
    registry::{
        InputModality, ModelMode, OutputModality, ReasoningLevel, ReasoningSupport, RouteConfig,
        RouteMode, StateAffinity, UpstreamApiCapabilities, build_registry,
    },
    upstream_credentials::UpstreamCredentialConfiguration,
};

#[path = "example_config/catalog.rs"]
mod catalog;
#[path = "example_config/configuration.rs"]
mod configuration;
#[path = "example_config/providers.rs"]
mod providers;
#[path = "example_config/routing.rs"]
mod routing;
