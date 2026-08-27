//! Registers the fixed Alibaba Cloud Model Studio deployment and approved model Targets.

use std::time::Duration;

use crate::{
    core::{
        ExecutableResponsesState, JsonSchemaSupport, ReasoningOutput, ResponsesAffinity,
        StorageSupport, StructuredOutputProfile,
    },
    models::{deepseek, moonshotai, qwen, z_ai},
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, IgnorableGenerationParameter, ProviderInstanceConfig,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::{
    DEFINITION,
    media::{KIMI_IMAGE_INPUT, QWEN_IMAGE_INPUT},
};

const PROVIDER_INSTANCE_ID: &str = "bailian";
const NATIVE_PROVIDER_INSTANCE_ID: &str = "bailian-native";
const CREDENTIAL_POOL_ID: &str = "bailian-primary";
const DEEPSEEK_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const QWEN3_7_PLUS_STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);

/// Builds the trusted Model Studio Beijing deployment used by approved Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Bailian,
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_owned(),
    }
}

/// Builds the trusted DashScope-native Beijing deployment used by Images generation Targets.
pub(crate) fn native_provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: NATIVE_PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Bailian,
        base_url: "https://dashscope.aliyuncs.com/api/v1".to_owned(),
    }
}

/// Builds the fixed GLM-5.2, Qwen, Kimi, and DeepSeek V4 targets for Model Studio.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        chat_target(
            "bailian/glm-5-2",
            z_ai::glm_5_2::ID,
            "glm-5.2",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian/qwen3-7-plus",
            qwen::qwen3_7_plus::ID,
            "qwen3.7-plus",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian/qwen3-7-max",
            qwen::qwen3_7_max::ID,
            "qwen3.7-max",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian/qwen3-8-max",
            qwen::qwen3_8_max::ID,
            "qwen3.8-max",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian/qwen3-8-27b",
            qwen::qwen3_8_27b::ID,
            "qwen3.8-27b",
            ReasoningOutput::PlainText,
        ),
        // Keep Kimi Chat-only: the 2026-08-24 probe rejected its native Responses operation.
        chat_target(
            "bailian/kimi-k3",
            moonshotai::kimi_k3::ID,
            "kimi-k3",
            ReasoningOutput::PlainText,
        ),
        image_target(
            "bailian/qwen-image-3-0",
            qwen::qwen_image_3_0::ID,
            "qwen-image-3.0",
        ),
        image_target(
            "bailian/qwen-image-3-0-pro",
            qwen::qwen_image_3_0_pro::ID,
            "qwen-image-3.0-pro",
        ),
        embedding_target(),
        chat_target(
            "bailian/deepseek-v4-pro",
            deepseek::deepseek_v4_pro::ID,
            "deepseek-v4-pro-0813",
            ReasoningOutput::PlainText,
        ),
        chat_target(
            "bailian/deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash-0731",
            ReasoningOutput::PlainText,
        ),
    ]
}

/// Binds Qwen3.7 Text Embedding to Model Studio's trusted Embeddings endpoint.
fn embedding_target() -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: "bailian/qwen3-7-text-embedding".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: qwen::qwen3_7_text_embedding::ID.to_owned(),
        provider_model: ProviderKind::Bailian.routing_model_id(qwen::qwen3_7_text_embedding::ID),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("bailian-api".to_owned()),
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(150)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::EmbeddingsCreate,
                CanonicalTaskKind::Embedding,
            ),
            upstream_model: "qwen3.7-text-embedding".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Embeddings(
                DEFINITION
                    .contract()
                    .capabilities()
                    .operation(crate::core::OperationKind::EmbeddingsCreate)
                    .and_then(crate::core::ProviderOperationCapabilities::embeddings)
                    .expect("Bailian embedding targets require Embeddings capabilities"),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}

/// Binds one canonical model to its confirmed Model Studio Generation endpoints and credential pool.
fn chat_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    reasoning_output: ReasoningOutput,
) -> UpstreamTargetConfig {
    // Narrow the Provider ceilings to the reasoning output confirmed for this specific model.
    let image_input = match canonical_model {
        qwen::qwen3_7_plus::ID | qwen::qwen3_8_max::ID | qwen::qwen3_8_27b::ID => {
            Some(QWEN_IMAGE_INPUT)
        }
        moonshotai::kimi_k3::ID => Some(KIMI_IMAGE_INPUT),
        _ => None,
    };
    let mut chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("Bailian generation targets require Chat Completions capabilities")
        .to_executable(crate::core::ChatMediaProfile::new(image_input, None, None));
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
        z_ai::glm_5_2::ID | deepseek::deepseek_v4_pro::ID
    );
    chat_capabilities.structured_outputs = match canonical_model {
        deepseek::deepseek_v4_pro::ID | deepseek::deepseek_v4_flash::ID => {
            Some(DEEPSEEK_STRUCTURED_OUTPUTS)
        }
        qwen::qwen3_7_plus::ID => Some(QWEN3_7_PLUS_STRUCTURED_OUTPUTS),
        _ => None,
    };
    // Bind Chat for every target and Responses only for Qwen models confirmed on that endpoint.
    let mut upstream_apis = vec![UpstreamApiConfig {
        key: UpstreamApiKey::new(
            crate::core::OperationKind::ChatCompletions,
            CanonicalTaskKind::Generation,
        ),
        upstream_model: upstream_model.to_owned(),
        model_rules: generation_model_rules(canonical_model),
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];
    if matches!(
        canonical_model,
        qwen::qwen3_8_max::ID
            | qwen::qwen3_8_27b::ID
            | qwen::qwen3_7_max::ID
            | qwen::qwen3_7_plus::ID
    ) {
        let mut responses_capabilities = DEFINITION
            .contract()
            .capabilities()
            .operation(crate::core::OperationKind::Responses)
            .and_then(crate::core::ProviderOperationCapabilities::responses)
            .expect("Bailian Qwen targets require Responses capabilities")
            .to_executable(
                ExecutableResponsesState::new(
                    StorageSupport::Unsupported,
                    ResponsesAffinity::TargetBound,
                ),
                crate::core::ResponsesMediaProfile::new(image_input, None),
            );
        responses_capabilities.function_tools =
            responses_capabilities.function_tools.map(|mut profile| {
                profile.parallel_calls = false;
                profile
            });
        // Real probing (2026-08-11) shows qwen3.7-plus Responses accepts json_object only;
        // json_schema is silently downgraded, so it is not advertised. Other Responses
        // targets in this branch (qwen3.8-max, qwen3.8-27b, qwen3.7-max) are not covered by that probe
        // and stay narrowed to no structured outputs despite the Provider ceiling.
        responses_capabilities.structured_outputs = if canonical_model == qwen::qwen3_7_plus::ID {
            Some(StructuredOutputProfile::JsonObject)
        } else {
            None
        };
        upstream_apis.push(UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::Responses,
                CanonicalTaskKind::Generation,
            ),
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
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(150)),
        enabled: true,
        upstream_apis,
    }
}

/// Applies model-specific parameter boundaries not shared by the Bailian Provider ceiling.
fn generation_model_rules(canonical_model: &str) -> UpstreamApiModelRules {
    if canonical_model == moonshotai::kimi_k3::ID {
        return UpstreamApiModelRules {
            disabled_parameters: vec![
                "logprobs".to_owned(),
                "n".to_owned(),
                "top_logprobs".to_owned(),
            ],
            ignored_parameters: vec![
                IgnorableGenerationParameter::FrequencyPenalty,
                IgnorableGenerationParameter::PresencePenalty,
                IgnorableGenerationParameter::Temperature,
                IgnorableGenerationParameter::TopP,
            ],
            ..UpstreamApiModelRules::default()
        };
    }
    UpstreamApiModelRules::default()
}

/// Binds one image model to the DashScope-native multimodal-generation endpoint and credential pool.
fn image_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
    // Narrow the Provider Images ceiling exactly to the one trusted native operation.
    let images_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::ImagesGenerations)
        .and_then(crate::core::ProviderOperationCapabilities::images_generations)
        .expect("Bailian image targets require Images Generations capabilities");
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: NATIVE_PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::Bailian.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("bailian-native-api".to_owned()),
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(180)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::ImagesGenerations,
                CanonicalTaskKind::ImageGeneration,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ImagesGenerations(images_capabilities),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}
