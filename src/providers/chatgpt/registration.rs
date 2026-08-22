//! Registers fixed ChatGPT model targets and their Responses-only APIs.
//!
//! Every target shares one account-bound OAuth2 credential and one trusted Codex backend origin.
//! Request-time model selection cannot change the origin, credential binding, or API surface.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    models::chatgpt,
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, IgnorableGenerationParameter, NonStreamingConversion,
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey,
        UpstreamApiModelRules, UpstreamStreamingPolicy, UpstreamTargetConfig,
    },
};

use super::{DEFINITION, media::IMAGE_INPUT};

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
            false,
        ),
        upstream_target(
            "chatgpt-gpt-5-5",
            chatgpt::gpt_5_5::ID,
            "gpt-5.5",
            true,
            true,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-luna",
            chatgpt::gpt_5_6_luna::ID,
            "gpt-5.6-luna",
            true,
            true,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-terra",
            chatgpt::gpt_5_6_terra::ID,
            "gpt-5.6-terra",
            true,
            true,
        ),
        upstream_target(
            "chatgpt-gpt-5-6-sol",
            chatgpt::gpt_5_6_sol::ID,
            "gpt-5.6-sol",
            true,
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
    advanced_capabilities: bool,
) -> UpstreamTargetConfig {
    // Narrow parameters that the private Responses backend rejects instead of hiding output changes.
    let mut disabled_parameters = Vec::new();
    if disable_output_limit_parameters {
        disabled_parameters.extend(["max_completion_tokens".to_owned(), "max_tokens".to_owned()]);
    }
    if advanced_capabilities {
        disabled_parameters.push("include_reasoning".to_owned());
    }

    // Build the static target and retain seed as the only accepted backend sampling hint to omit.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::ChatGpt.routing_model_id(canonical_model),
        credential_pool: "chatgpt-codex".to_owned(),
        quota_scope: Some("chatgpt-codex".to_owned()),
        fault_domain: Some("chatgpt-codex-backend".to_owned()),
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(120)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::Responses,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules {
                disabled_parameters,
                ignored_parameters: if advanced_capabilities {
                    vec![IgnorableGenerationParameter::Seed]
                } else {
                    Vec::new()
                },
                ..UpstreamApiModelRules::default()
            },
            capabilities: responses_capabilities(advanced_capabilities),
            streaming_policy: UpstreamStreamingPolicy::Required {
                non_streaming: NonStreamingConversion::BufferResponsesSse,
            },
        }],
    }
}

/// Narrows the family contract to the capabilities guaranteed by one fixed ChatGPT model.
fn responses_capabilities(advanced_capabilities: bool) -> UpstreamApiCapabilities {
    let mut capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("ChatGPT targets require Responses capabilities");
    if !advanced_capabilities {
        capabilities.function_tools = None;
        capabilities.structured_outputs = None;
    }
    UpstreamApiCapabilities::Responses(capabilities.to_executable(
        ExecutableResponsesState::new(StorageSupport::Unsupported, ResponsesAffinity::TargetBound),
        crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
    ))
}
