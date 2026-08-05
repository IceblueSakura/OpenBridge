//! Registers DeepSeek V4 Upstream Targets and Chat Upstream APIs.

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

/// Builds the fixed upstream targets for DeepSeek V4 Pro and Flash.
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

/// Builds a Chat-only target for a DeepSeek V4 model.
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
            operation: ApiProtocol::ChatCompletions.operation(),
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
