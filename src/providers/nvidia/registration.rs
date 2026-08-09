//! Registers the fixed NVIDIA API Catalog deployment and approved model Targets.

use std::time::Duration;

use crate::{
    models::minimax,
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "nvidia";
const CREDENTIAL_POOL_ID: &str = "nvidia-primary";

/// Builds the trusted NVIDIA API Catalog deployment used by approved Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Nvidia,
        base_url: "https://integrate.api.nvidia.com/v1".to_owned(),
    }
}

/// Builds the fixed MiniMax M3 Chat target for NVIDIA API Catalog.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![chat_target(
        "nvidia-minimax-m3",
        minimax::minimax_m3::ID,
        "minimaxai/minimax-m3",
    )]
}

/// Binds one canonical model to NVIDIA's trusted Chat endpoint and credential pool.
fn chat_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::Nvidia.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("nvidia-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                DEFINITION
                    .contract()
                    .capabilities()
                    .chat_completions
                    .expect("NVIDIA targets require Chat Completions capabilities")
                    .to_executable(None),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}
