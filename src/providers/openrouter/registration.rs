//! OpenRouter Nemotron Upstream Target 与无状态 Native API 注册。

use std::time::Duration;

use crate::{
    core::ApiProtocol,
    models::nemotron,
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialConfig, StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

/// 构造当前编译版本内置的 OpenRouter Nemotron target。
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openrouter-nemotron-3-ultra".to_owned(),
        provider: ProviderKind::OpenRouter,
        model: nemotron::NEMOTRON_3_ULTRA_ID.to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        credential: CredentialConfig {
            id: "openrouter-primary".to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "OPENROUTER_API_KEY".to_owned(),
        },
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                id: "chat".to_owned(),
                protocol: ApiProtocol::ChatCompletions,
                upstream_model: "nvidia/nemotron-3-ultra-550b-a55b".to_owned(),
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
                protocol: ApiProtocol::Responses,
                upstream_model: "nvidia/nemotron-3-ultra-550b-a55b".to_owned(),
                endpoint_profile: "openrouter-responses".to_owned(),
                transport: TransportKind::HttpJsonSse,
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
                state_affinity: StateAffinity::Unbound,
            },
        ],
    }]
}
