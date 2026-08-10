//! Registers approved OpenRouter model targets and their stateless Native APIs.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport, StructuredOutputProfile},
    models::{deepseek, minimax},
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "openrouter";
const DEEPSEEK_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;

/// Builds the trusted OpenRouter API deployment used by the checked-in target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::OpenRouter,
        base_url: "https://openrouter.ai/api/v1".to_owned(),
    }
}

/// Builds the approved OpenRouter targets built into this compiled version.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        dual_protocol_target(
            "openrouter-deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek/deepseek-v4-flash",
        ),
        dual_protocol_target(
            "openrouter-minimax-m3",
            minimax::minimax_m3::ID,
            "minimax/minimax-m3",
        ),
    ]
}

/// Binds one canonical model to OpenRouter's trusted Chat and Responses endpoints.
fn dual_protocol_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
) -> UpstreamTargetConfig {
    // Narrow the generic OpenRouter ceiling to model-specific DeepSeek JSON Output evidence.
    let mut chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .chat_completions
        .expect("OpenRouter targets require Chat Completions capabilities")
        .to_executable(None);
    let mut responses_capabilities = DEFINITION
        .contract()
        .capabilities()
        .responses
        .expect("OpenRouter targets require Responses capabilities")
        .to_executable(ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::Unbound,
        ));
    let structured_outputs =
        (canonical_model == deepseek::deepseek_v4_flash::ID).then_some(DEEPSEEK_STRUCTURED_OUTPUTS);
    chat_capabilities.structured_outputs = structured_outputs;
    responses_capabilities.structured_outputs = structured_outputs;
    if canonical_model != deepseek::deepseek_v4_flash::ID {
        chat_capabilities.function_tools = chat_capabilities.function_tools.map(|mut profile| {
            profile.parallel_calls = false;
            profile
        });
        responses_capabilities.function_tools =
            responses_capabilities.function_tools.map(|mut profile| {
                profile.parallel_calls = false;
                profile
            });
        responses_capabilities.include = &[];
    }

    // Bind the model-specific capabilities to both stateless Native protocol endpoints.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::OpenRouter.routing_model_id(canonical_model),
        credential_pool: "openrouter-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
            },
            UpstreamApiConfig {
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
            },
        ],
    }
}
