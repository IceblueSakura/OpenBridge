//! DeepSeek V4 Upstream Target 与 Chat Upstream API 注册。

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

/// 构造 DeepSeek V4 Pro 与 Flash 的固定 upstream targets。
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "deepseek-v4-pro",
            deepseek::v4_pro::ID,
            "deepseek-v4-pro",
            "deepseek-primary",
        ),
        target(
            "deepseek-v4-flash",
            deepseek::v4_flash::ID,
            "deepseek-v4-flash",
            "deepseek-primary",
        ),
    ]
}

/// 为一个 DeepSeek V4 模型构造 Chat-only target。
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider: ProviderKind::DeepSeek,
        model: canonical_model.to_owned(),
        base_url: "https://api.deepseek.com".to_owned(),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("deepseek-primary".to_owned()),
        fault_domain: Some("deepseek-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            id: "chat".to_owned(),
            protocol: ApiProtocol::ChatCompletions,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: "deepseek-openai".to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                CONTRACT.capabilities().chat_completions,
            ),
            state_affinity: StateAffinity::Unbound,
        }],
    }
}
