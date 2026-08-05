//! Registers the disabled ChatGPT Codex probe target and its sole Responses API.
//!
//! No Route or Public Model references this target during the first OAuth delivery stage, so the
//! long-lived service neither selects it nor requires its credential.

use std::time::Duration;

use crate::{
    core::ApiProtocol,
    models::openai,
    provider::ProviderKind,
    registry::{
        StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

/// Builds the fixed ChatGPT target available only to explicit administrative probes.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "chatgpt-gpt-5-6-sol".to_owned(),
        provider: ProviderKind::ChatGpt,
        model: openai::gpt_5_6_sol::ID.to_owned(),
        base_url: "https://chatgpt.com/backend-api/codex".to_owned(),
        credential_pool: "chatgpt-codex".to_owned(),
        quota_scope: Some("chatgpt-codex".to_owned()),
        fault_domain: Some("chatgpt-codex-backend".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: false,
        upstream_apis: vec![UpstreamApiConfig {
            id: "responses".to_owned(),
            operation: ApiProtocol::Responses.operation(),
            upstream_model: "gpt-5.6-sol".to_owned(),
            endpoint_profile: "chatgpt-codex".to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
            state_affinity: StateAffinity::TargetBound,
        }],
    }]
}
