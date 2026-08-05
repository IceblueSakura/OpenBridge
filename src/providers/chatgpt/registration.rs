//! Registers the disabled ChatGPT target and its sole Responses API.
//!
//! No Route or Public Model references this target in the current OAuth lifecycle stage, so the
//! long-lived service neither selects it nor requires its credential.

use std::time::Duration;

use crate::{
    models::openai,
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

const PROVIDER_INSTANCE_ID: &str = "chatgpt";

/// Builds the trusted ChatGPT backend deployment referenced by the disabled target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::ChatGpt,
        base_url: "https://chatgpt.com/backend-api/codex".to_owned(),
    }
}

/// Builds the fixed disabled ChatGPT target reserved for a later data-plane integration.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "chatgpt-gpt-5-6-sol".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        model: openai::gpt_5_6_sol::ID.to_owned(),
        credential_pool: "chatgpt-codex".to_owned(),
        quota_scope: Some("chatgpt-codex".to_owned()),
        fault_domain: Some("chatgpt-codex-backend".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: false,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: "gpt-5.6-sol".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
            state_affinity: StateAffinity::TargetBound,
        }],
    }]
}
