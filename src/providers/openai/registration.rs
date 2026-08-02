//! OpenAI Upstream Target 与原生 Upstream API 注册。

use std::time::Duration;

use crate::{
    core::{ApiCapabilities, ApiProtocol, EndpointCapabilities, ResponsesCapabilities},
    models::gpt,
    provider::{CredentialKind, ProviderKind},
    registry::{
        CredentialConfig, StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

/// 构造当前编译版本内置的 OpenAI upstream targets。
pub fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openai-main".to_owned(),
        provider: ProviderKind::OpenAi,
        model: gpt::GPT_5_6_SOL_ID.to_owned(),
        base_url: "https://api.openai.com".to_owned(),
        credential: CredentialConfig {
            id: "openai-primary".to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "OPENAI_API_KEY".to_owned(),
        },
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: upstream_apis(
            "gpt-5.6-sol",
            "public-api",
            conservative_openai_capabilities(),
        ),
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

/// 返回保守的 OpenAI capability 配置，需经实际上游 probe 后再扩大。
pub const fn conservative_openai_capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            previous_response_id: false,
            background: false,
        },
    }
}
