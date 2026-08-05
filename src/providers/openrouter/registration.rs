//! Registers the OpenRouter DeepSeek V4 Flash target and stateless Native APIs.

use std::time::Duration;

use crate::{
    core::ApiProtocol,
    models::deepseek,
    provider::ProviderKind,
    registry::{
        StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

/// Builds the OpenRouter DeepSeek V4 Flash target built into this compiled version.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openrouter-deepseek-v4-flash".to_owned(),
        provider: ProviderKind::OpenRouter,
        model: deepseek::deepseek_v4_flash::ID.to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        credential_pool: "openrouter-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                id: "chat".to_owned(),
                operation: ApiProtocol::ChatCompletions.operation(),
                upstream_model: "deepseek/deepseek-v4-flash".to_owned(),
                endpoint_profile: "openrouter-chat".to_owned(),
                transport: TransportKind::HttpJsonSse,
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ChatCompletions(
                    CONTRACT.capabilities().chat_completions,
                ),
                state_affinity: StateAffinity::Unbound,
            },
            UpstreamApiConfig {
                id: "responses".to_owned(),
                operation: ApiProtocol::Responses.operation(),
                upstream_model: "deepseek/deepseek-v4-flash".to_owned(),
                endpoint_profile: "openrouter-responses".to_owned(),
                transport: TransportKind::HttpJsonSse,
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
                state_affinity: StateAffinity::Unbound,
            },
        ],
    }]
}
