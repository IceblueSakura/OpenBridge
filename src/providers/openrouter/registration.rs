//! Registers approved OpenRouter model targets and their stateless Native APIs.

use std::time::Duration;

use crate::{
    core::{
        ExecutableResponsesState, JsonSchemaSupport, ResponsesAffinity, StorageSupport,
        StructuredOutputProfile, ToolChoiceMode,
    },
    models::{deepseek, google, meta, minimax, xai, z_ai},
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiKey, UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::{DEFINITION, media::IMAGE_INPUT};

const PROVIDER_INSTANCE_ID: &str = "openrouter";
const STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);
const AUTO_TOOL_CHOICE_MODES: &[ToolChoiceMode] = &[ToolChoiceMode::Auto];
/// JSON-object-only profile for targets whose strict schema behavior is not a reliable guarantee.
const JSON_OBJECT_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;

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
            "openrouter/deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek/deepseek-v4-flash",
        ),
        dual_protocol_target(
            "openrouter/minimax-m3",
            minimax::minimax_m3::ID,
            "minimax/minimax-m3",
        ),
        dual_protocol_target(
            "openrouter/gemma-4-31b-it",
            google::gemma_4_31b_it::ID,
            "google/gemma-4-31b-it:free",
        ),
        dual_protocol_target(
            "openrouter/gemini-3-7-flash",
            google::gemini_3_7_flash::ID,
            "google/gemini-3.7-flash",
        ),
        dual_protocol_target("openrouter/grok-4-6", xai::grok_4_6::ID, "x-ai/grok-4.6"),
        dual_protocol_target(
            "openrouter/muse-spark-1.2-contributor",
            meta::muse_spark_1_2_contributor::ID,
            "meta/muse-spark-1.2-contributor",
        ),
        dual_protocol_target(
            "openrouter/glm-5.3-flash",
            z_ai::glm_5_3_flash::ID,
            "z-ai/glm-5.3-flash",
        ),
    ]
}

/// Binds one canonical model to OpenRouter's trusted Chat and Responses endpoints.
fn dual_protocol_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
) -> UpstreamTargetConfig {
    // Expose image input only for models covered by the model-specific Provider probe.
    let supports_image_input = matches!(
        canonical_model,
        google::gemini_3_7_flash::ID | xai::grok_4_6::ID | z_ai::glm_5_3_flash::ID
    );
    let chat_media =
        crate::core::ChatMediaProfile::new(supports_image_input.then_some(IMAGE_INPUT), None, None);
    let mut chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("OpenRouter targets require Chat Completions capabilities")
        .to_executable(chat_media);
    let responses_image = supports_image_input.then_some(IMAGE_INPUT);
    let mut responses_capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("OpenRouter targets require Responses capabilities")
        .to_executable(
            ExecutableResponsesState::new(StorageSupport::Unsupported, ResponsesAffinity::Unbound),
            crate::core::ResponsesMediaProfile::new(responses_image, None),
        );
    chat_capabilities.structured_outputs = Some(STRUCTURED_OUTPUTS);
    responses_capabilities.structured_outputs = Some(STRUCTURED_OUTPUTS);
    // Gemma 4 31B keeps JSON-object output and does not guarantee strict schema.
    if canonical_model == google::gemma_4_31b_it::ID {
        chat_capabilities.structured_outputs = Some(JSON_OBJECT_STRUCTURED_OUTPUTS);
        responses_capabilities.structured_outputs = Some(JSON_OBJECT_STRUCTURED_OUTPUTS);
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
    // GLM-5.3-Flash accepts automatic function-tool selection and JSON-object Chat output. Named
    // tool selection is rejected, and Responses JSON Schema/object output is not reliable enough
    // to publish as a downstream guarantee.
    if canonical_model == z_ai::glm_5_3_flash::ID {
        for profile in [
            chat_capabilities.function_tools.as_mut(),
            responses_capabilities.function_tools.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            profile.choice_modes = AUTO_TOOL_CHOICE_MODES;
        }
        chat_capabilities.structured_outputs = Some(JSON_OBJECT_STRUCTURED_OUTPUTS);
        responses_capabilities.structured_outputs = None;
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
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(120)),
        enabled: true,
        upstream_apis: vec![
            UpstreamApiConfig {
                key: UpstreamApiKey::new(
                    crate::core::OperationKind::ChatCompletions,
                    CanonicalTaskKind::Generation,
                ),
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
            },
            UpstreamApiConfig {
                key: UpstreamApiKey::new(
                    crate::core::OperationKind::Responses,
                    CanonicalTaskKind::Generation,
                ),
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{OperationKind, ToolChoiceMode};

    #[test]
    fn glm_5_3_flash_uses_probed_tool_and_structured_output_profiles() {
        let target = upstream_targets()
            .into_iter()
            .find(|target| target.canonical_model == z_ai::glm_5_3_flash::ID)
            .expect("GLM-5.3-Flash OpenRouter target must remain registered");

        for api in target.upstream_apis {
            match (api.key.operation(), api.capabilities) {
                (
                    OperationKind::ChatCompletions,
                    UpstreamApiCapabilities::ChatCompletions(capabilities),
                ) => {
                    assert!(capabilities.media.file.is_none());
                    let tools = capabilities
                        .function_tools
                        .expect("Chat tools must remain enabled");
                    assert_eq!(tools.choice_modes, &[ToolChoiceMode::Auto]);
                    assert!(tools.parallel_calls);
                    assert!(tools.strict_schema);
                    assert_eq!(
                        capabilities.structured_outputs,
                        Some(StructuredOutputProfile::JsonObject)
                    );
                }
                (OperationKind::Responses, UpstreamApiCapabilities::Responses(capabilities)) => {
                    assert!(capabilities.media.file.is_none());
                    let tools = capabilities
                        .function_tools
                        .expect("Responses tools must remain enabled");
                    assert_eq!(tools.choice_modes, &[ToolChoiceMode::Auto]);
                    assert!(tools.parallel_calls);
                    assert!(tools.strict_schema);
                    assert_eq!(capabilities.structured_outputs, None);
                }
                _ => panic!("unexpected GLM-5.3-Flash operation profile"),
            }
        }
    }
}
