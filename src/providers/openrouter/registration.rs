//! Registers approved OpenRouter model targets and their stateless Native APIs.

use std::time::Duration;

use crate::{
    core::{
        ExecutableResponsesState, JsonSchemaSupport, ResponsesAffinity, StorageSupport,
        StructuredOutputProfile,
    },
    models::{deepseek, google, minimax},
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "openrouter";
const STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);
/// Gemma 4 31B returns markdown-wrapped JSON for strict schema requests, so its
/// executable targets keep the reliable JSON-object profile only.
const GEMMA_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;

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
        dual_protocol_target(
            "openrouter-gemma-4-31b-it",
            google::gemma_4_31b_it::ID,
            "google/gemma-4-31b-it:free",
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
    chat_capabilities.structured_outputs = Some(STRUCTURED_OUTPUTS);
    responses_capabilities.structured_outputs = Some(STRUCTURED_OUTPUTS);
    // DeepSeek V4 Flash is text-only; MiniMax M3 keeps the Chat family image ceiling.
    if canonical_model == deepseek::deepseek_v4_flash::ID {
        chat_capabilities.image_input = None;
    }
    // Gemma 4 31B keeps JSON-object output and does not guarantee strict schema.
    if canonical_model == google::gemma_4_31b_it::ID {
        chat_capabilities.structured_outputs = Some(GEMMA_STRUCTURED_OUTPUTS);
        responses_capabilities.structured_outputs = Some(GEMMA_STRUCTURED_OUTPUTS);
        for profile in [
            chat_capabilities.function_tools.as_mut(),
            responses_capabilities.function_tools.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            profile.strict_schema = false;
        }
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
