//! Registers the OpenRouter DeepSeek V4 Flash target and stateless Native APIs.

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

const PROVIDER_INSTANCE_ID: &str = "openrouter";

/// Builds the trusted OpenRouter API deployment used by the checked-in target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::OpenRouter,
        base_url: "https://openrouter.ai/api/v1".to_owned(),
    }
}

/// Builds the OpenRouter DeepSeek V4 Flash target built into this compiled version.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openrouter-deepseek-v4-flash".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        model: deepseek::deepseek_v4_flash::ID.to_owned(),
        credential_pool: "openrouter-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                upstream_model: "deepseek/deepseek-v4-flash".to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ChatCompletions(
                    CONTRACT.capabilities().chat_completions,
                ),
                state_affinity: StateAffinity::Unbound,
            },
            UpstreamApiConfig {
                upstream_model: "deepseek/deepseek-v4-flash".to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
                state_affinity: StateAffinity::Unbound,
            },
        ],
    }]
}
