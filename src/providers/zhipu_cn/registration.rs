//! Registers the fixed Zhipu AI China deployment and approved GLM Targets.

use std::time::Duration;

use crate::{
    core::{StructuredOutputProfile, ToolChoiceMode},
    models::z_ai,
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiKey, UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::{DEFINITION, media::IMAGE_INPUT};

const PROVIDER_INSTANCE_ID: &str = "zhipu-cn";
const CREDENTIAL_POOL_ID: &str = "zhipu-primary";
const AUTO_TOOL_CHOICE_MODES: &[ToolChoiceMode] = &[ToolChoiceMode::Auto];

/// Builds the trusted Zhipu China deployment used by the registered GLM Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::ZhipuCn,
        base_url: "https://open.bigmodel.cn/api/paas/v4/".to_owned(),
    }
}

/// Builds the fixed GLM-5.3, GLM-5.2, and GLM-5.3-Flash Chat targets for Zhipu China.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        chat_target("zhipu-cn-glm-5-3", z_ai::glm_5_3::ID, "glm-5.3"),
        chat_target("zhipu-cn-glm-5-2", z_ai::glm_5_2::ID, "glm-5.2"),
        chat_target(
            "zhipu-cn-glm-5-3-flash",
            z_ai::glm_5_3_flash::ID,
            "glm-5.3-flash",
        ),
    ]
}

/// Binds one canonical GLM model to the trusted Chat endpoint and shared credential pool.
fn chat_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
    let mut capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("Zhipu China targets require Chat Completions capabilities")
        .to_executable(crate::core::ChatMediaProfile::new(
            (canonical_model == z_ai::glm_5_3_flash::ID).then_some(IMAGE_INPUT),
            None,
            None,
        ));
    // Flash accepts only automatic tool selection; the text models accept every standard mode.
    if canonical_model == z_ai::glm_5_3_flash::ID {
        capabilities
            .function_tools
            .as_mut()
            .expect("Zhipu GLM targets require function tools")
            .choice_modes = AUTO_TOOL_CHOICE_MODES;
    }
    // Only Flash completed bounded JSON-object probes; keep the text Targets closed for now.
    if canonical_model != z_ai::glm_5_3_flash::ID {
        capabilities.structured_outputs = None;
    } else {
        capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
    }

    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::ZhipuCn.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("zhipu-cn-api".to_owned()),
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(120)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::ChatCompletions,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(capabilities),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}
