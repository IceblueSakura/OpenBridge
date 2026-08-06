//! Registers fixed ChatGPT model targets and their Responses-only APIs.
//!
//! Every target shares one account-bound OAuth2 credential and one trusted Codex backend origin.
//! Request-time model selection cannot change the origin, credential binding, or API surface.

use std::time::Duration;

use crate::{
    models::chatgpt,
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

const PROVIDER_INSTANCE_ID: &str = "chatgpt";

/// Builds the trusted ChatGPT backend deployment referenced by all fixed targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::ChatGpt,
        base_url: "https://chatgpt.com/backend-api/codex".to_owned(),
    }
}

/// Builds the fixed Responses-only ChatGPT targets exposed by the compiled Route catalog.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        upstream_target(
            "chatgpt-gpt-5-3-codex-spark",
            chatgpt::gpt_5_3_codex_spark::ID,
            "gpt-5.3-codex-spark",
            false,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-luna",
            chatgpt::gpt_5_6_luna::ID,
            "gpt-5.6-luna",
            true,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-terra",
            chatgpt::gpt_5_6_terra::ID,
            "gpt-5.6-terra",
            true,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-sol",
            chatgpt::gpt_5_6_sol::ID,
            "gpt-5.6-sol",
            true,
        ),
    ]
}

/// Binds one canonical ChatGPT profile to its fixed upstream model and shared OAuth2 context.
fn upstream_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    disable_output_limit_parameters: bool,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        model: canonical_model.to_owned(),
        credential_pool: "chatgpt-codex".to_owned(),
        quota_scope: Some("chatgpt-codex".to_owned()),
        fault_domain: Some("chatgpt-codex-backend".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules {
                disabled_parameters: if disable_output_limit_parameters {
                    vec!["max_completion_tokens".to_owned(), "max_tokens".to_owned()]
                } else {
                    Vec::new()
                },
                ..UpstreamApiModelRules::default()
            },
            capabilities: UpstreamApiCapabilities::Responses(CONTRACT.capabilities().responses),
            state_affinity: StateAffinity::TargetBound,
        }],
    }
}
