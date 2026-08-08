//! Registers approved OpenRouter model targets and their stateless Native APIs.

use std::time::Duration;

use crate::{
    models::{deepseek, minimax},
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

/// Builds the approved OpenRouter targets built into this compiled version.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        dual_protocol_target(
            "openrouter-deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek/deepseek-v4-flash",
        ),
        dual_protocol_target(
            "openrouter-minimax-m3",
            minimax::minimax_m3::ID,
            "minimax/minimax-m3",
        ),
    ]
}

/// Binds one canonical model to OpenRouter's trusted Chat and Responses endpoints.
fn dual_protocol_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::OpenRouter.routing_model_id(canonical_model),
        credential_pool: "openrouter-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ChatCompletions(
                    CONTRACT.capabilities().chat_completions,
                ),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
                state_affinity: StateAffinity::Unbound,
            },
            UpstreamApiConfig {
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
                state_affinity: StateAffinity::Unbound,
            },
        ],
    }
}
