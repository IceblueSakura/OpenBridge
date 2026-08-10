//! Registers the fixed Alibaba Cloud Model Studio deployment and approved model Targets.

use std::time::Duration;

use crate::{
    core::{
        ExecutableResponsesState, ReasoningOutput, ResponsesAffinity, StorageSupport,
        StructuredOutputProfile,
    },
    models::{deepseek, qwen, z_ai},
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "bailian";
const CREDENTIAL_POOL_ID: &str = "bailian-primary";
const DEEPSEEK_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;

/// Builds the trusted Model Studio Beijing deployment used by approved Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Bailian,
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_owned(),
    }
}

/// Builds the fixed GLM-5.2, Qwen, and DeepSeek V4 targets for Model Studio.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        chat_target(
            "bailian-glm-5-2",
            z_ai::glm_5_2::ID,
            "glm-5.2",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian-qwen3-7-plus",
            qwen::qwen3_7_plus::ID,
            "qwen3.7-plus",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian-qwen3-7-max",
            qwen::qwen3_7_max::ID,
            "qwen3.7-max",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian-qwen3-8-max",
            qwen::qwen3_8_max::ID,
            "qwen3.8-max",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian-qwen-image-3-0",
            qwen::qwen_image_3_0::ID,
            "qwen-image-3.0",
            ReasoningOutput::Unknown,
        ),
        chat_target(
            "bailian-qwen-image-3-0-pro",
            qwen::qwen_image_3_0_pro::ID,
            "qwen-image-3.0-pro",
            ReasoningOutput::Unknown,
        ),
        chat_target(
            "bailian-qwen3-5-livetranslate-flash-realtime",
            qwen::qwen3_5_livetranslate_flash_realtime::ID,
            "qwen3.5-livetranslate-flash-realtime",
            ReasoningOutput::Unknown,
        ),
        chat_target(
            "bailian-qwen3-6-27b",
            qwen::qwen3_6_27b::ID,
            "qwen3.6-27b",
            ReasoningOutput::PlainText,
        ),
        embedding_target(),
        chat_target(
            "bailian-deepseek-v4-pro",
            deepseek::deepseek_v4_pro::ID,
            "deepseek-v4-pro",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian-deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash-0731",
            ReasoningOutput::Unknown,
        ),
    ]
}

/// Binds Qwen3.7 Text Embedding to Model Studio's trusted Embeddings endpoint.
fn embedding_target() -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: "bailian-qwen3-7-text-embedding".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: qwen::qwen3_7_text_embedding::ID.to_owned(),
        provider_model: ProviderKind::Bailian.routing_model_id(qwen::qwen3_7_text_embedding::ID),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("bailian-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: "qwen3.7-text-embedding".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(
                DEFINITION
                    .contract()
                    .capabilities()
                    .embeddings
                    .expect("Bailian embedding targets require Embeddings capabilities"),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}

/// Binds one canonical model to Model Studio's trusted Chat endpoint and credential pool.
fn chat_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    reasoning_output: ReasoningOutput,
) -> UpstreamTargetConfig {
    // Narrow the Provider ceilings to the reasoning output confirmed for this specific model.
    let mut chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .chat_completions
        .expect("Bailian generation targets require Chat Completions capabilities")
        .to_executable(None);
    chat_capabilities.function_tools = chat_capabilities.function_tools.map(|mut profile| {
        profile.parallel_calls = matches!(
            canonical_model,
            z_ai::glm_5_2::ID | deepseek::deepseek_v4_flash::ID
        );
        profile
    });
    chat_capabilities.reasoning_output = reasoning_output;
    chat_capabilities.prompt_cache_key = matches!(
        canonical_model,
        z_ai::glm_5_2::ID | qwen::qwen3_6_27b::ID | deepseek::deepseek_v4_pro::ID
    );
    chat_capabilities.structured_outputs = matches!(
        canonical_model,
        deepseek::deepseek_v4_pro::ID | deepseek::deepseek_v4_flash::ID
    )
    .then_some(DEEPSEEK_STRUCTURED_OUTPUTS);
    // Bind Chat for every target and Responses only for the documented stable Qwen models.
    let mut upstream_apis = vec![UpstreamApiConfig {
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];
    if matches!(
        canonical_model,
        qwen::qwen3_8_max::ID | qwen::qwen3_7_max::ID | qwen::qwen3_7_plus::ID
    ) {
        let mut responses_capabilities = DEFINITION
            .contract()
            .capabilities()
            .responses
            .expect("Bailian Qwen targets require Responses capabilities")
            .to_executable(ExecutableResponsesState::new(
                StorageSupport::Unsupported,
                ResponsesAffinity::TargetBound,
            ));
        responses_capabilities.function_tools =
            responses_capabilities.function_tools.map(|mut profile| {
                profile.parallel_calls = false;
                profile
            });
        upstream_apis.push(UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        });
    }

    // Bind the narrowed generation contract to the trusted deployment and credential pool.
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::Bailian.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("bailian-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis,
    }
}
