//! Registers DeepSeek V4 targets with model-specific Native protocol surfaces.

use std::time::Duration;

use crate::{
    models::deepseek,
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

const PROVIDER_INSTANCE_ID: &str = "deepseek";

/// Builds the trusted DeepSeek API deployment used by the checked-in targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::DeepSeek,
        base_url: "https://api.deepseek.com".to_owned(),
    }
}

/// Builds the fixed upstream targets for DeepSeek V4 Pro and Flash.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "deepseek-v4-pro",
            deepseek::deepseek_v4_pro::ID,
            "deepseek-v4-pro",
            "deepseek-primary",
            false,
        ),
        target(
            "deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash",
            "deepseek-primary",
            true,
        ),
    ]
}

/// Builds a DeepSeek V4 target and enables Responses only for an explicitly supported model.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
    responses_enabled: bool,
) -> UpstreamTargetConfig {
    // Build the model-specific Native API set without introducing state affinity.
    let mut upstream_apis = vec![UpstreamApiConfig {
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(
            CONTRACT.capabilities().chat_completions,
        ),
        state_affinity: StateAffinity::Unbound,
    }];
    if responses_enabled {
        upstream_apis.push(UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
            state_affinity: StateAffinity::Unbound,
        });
    }

    // Bind the immutable API set to the fixed trusted DeepSeek deployment.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::DeepSeek.routing_model_id(canonical_model),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("deepseek-primary".to_owned()),
        fault_domain: Some("deepseek-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis,
    }
}
