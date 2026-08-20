//! Registers DeepSeek V4 targets with model-specific Native protocol surfaces.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    models::deepseek,
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, IgnorableGenerationParameter, ProviderInstanceConfig,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "deepseek";

/// Builds the trusted DeepSeek API deployment used by the checked-in targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::DeepSeek,
        base_url: "https://api.deepseek.com".to_owned(),
    }
}

/// Builds the fixed upstream targets for DeepSeek V4 Pro and Flash.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "deepseek-v4-pro",
            deepseek::deepseek_v4_pro::ID,
            "deepseek-v4-pro",
            "deepseek-primary",
        ),
        target(
            "deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash",
            "deepseek-primary",
        ),
    ]
}

/// Builds a DeepSeek V4 target with its explicit Native operation surface.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
) -> UpstreamTargetConfig {
    // Resolve the Chat profile required by every DeepSeek target.
    let chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("DeepSeek targets require Chat Completions capabilities")
        .to_executable(None, None);
    let mut unsupported_parameters = vec![
        "include_reasoning",
        "logit_bias",
        "min_p",
        "repetition_penalty",
        "seed",
        "top_k",
    ];
    if canonical_model == deepseek::deepseek_v4_flash::ID {
        unsupported_parameters.push("top_a");
    }
    let ignored_parameters = vec![
        IgnorableGenerationParameter::FrequencyPenalty,
        IgnorableGenerationParameter::PresencePenalty,
    ];

    // Build the confirmed Native Chat API and drop fields absent from DeepSeek's direct contract.
    let mut upstream_apis = vec![UpstreamApiConfig {
        key: UpstreamApiKey::new(
            crate::core::OperationKind::ChatCompletions,
            CanonicalTaskKind::Generation,
        ),
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules {
            disabled_parameters: unsupported_parameters
                .iter()
                .chain(["user"].iter())
                .map(|parameter| (*parameter).to_owned())
                .collect(),
            ignored_parameters: ignored_parameters.clone(),
            ..UpstreamApiModelRules::default()
        },
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];
    let responses_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("DeepSeek Responses targets require Responses capabilities")
        .to_executable(
            ExecutableResponsesState::new(StorageSupport::Unsupported, ResponsesAffinity::Unbound),
            None,
        );
    upstream_apis.push(UpstreamApiConfig {
        key: UpstreamApiKey::new(
            crate::core::OperationKind::Responses,
            CanonicalTaskKind::Generation,
        ),
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules {
            disabled_parameters: unsupported_parameters
                .iter()
                .chain(["logprobs", "stop"].iter())
                .map(|parameter| (*parameter).to_owned())
                .collect(),
            ignored_parameters,
            ..UpstreamApiModelRules::default()
        },
        capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    });

    // Bind the immutable API set to the fixed trusted DeepSeek deployment.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::DeepSeek.routing_model_id(canonical_model),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("deepseek-primary".to_owned()),
        fault_domain: Some("deepseek-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis,
    }
}
