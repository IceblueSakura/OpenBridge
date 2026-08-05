//! Registers DeepSeek V4 Upstream Targets and Chat Upstream APIs.

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
        ),
        target(
            "deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash",
            "deepseek-primary",
        ),
    ]
}

/// Builds a Chat-only target for a DeepSeek V4 model.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        model: canonical_model.to_owned(),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("deepseek-primary".to_owned()),
        fault_domain: Some("deepseek-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                CONTRACT.capabilities().chat_completions,
            ),
            state_affinity: StateAffinity::Unbound,
        }],
    }
}
