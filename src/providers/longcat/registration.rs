//! LongCat Upstream Target 与原生 Upstream API 注册。

use std::time::Duration;

use crate::{
    core::{ApiCapabilities, ApiProtocol},
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialConfig, StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

/// 构造 LongCat-2.0 的 upstream targets。
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "longcat-2".to_owned(),
        provider: ProviderKind::LongCat,
        model: "meituan/longcat-2.0".to_owned(),
        base_url: "https://api.longcat.chat".to_owned(),
        credential: CredentialConfig {
            id: "longcat-primary".to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "LONGCAT_API_KEY".to_owned(),
        },
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: upstream_apis("LongCat-2.0", "longcat-openai", *CONTRACT.capabilities()),
    }]
}

fn upstream_apis(
    upstream_model: &str,
    endpoint_profile: &str,
    capabilities: ApiCapabilities,
) -> Vec<UpstreamApiConfig> {
    vec![
        UpstreamApiConfig {
            id: "chat".to_owned(),
            protocol: ApiProtocol::ChatCompletions,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(capabilities.chat_completions),
            state_affinity: StateAffinity::Unbound,
        },
        UpstreamApiConfig {
            id: "responses".to_owned(),
            protocol: ApiProtocol::Responses,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(capabilities.responses),
            state_affinity: StateAffinity::TargetBound,
        },
    ]
}
