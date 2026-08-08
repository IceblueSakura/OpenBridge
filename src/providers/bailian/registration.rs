//! Registers the fixed Alibaba Cloud Model Studio deployment and approved model Targets.

use std::time::Duration;

use crate::{
    models::{deepseek, qwen, z_ai},
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, StateAffinity, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
};

use super::CONTRACT;

const PROVIDER_INSTANCE_ID: &str = "bailian";
const CREDENTIAL_POOL_ID: &str = "bailian-primary";

/// Builds the trusted Model Studio Beijing deployment used by approved Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Bailian,
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_owned(),
    }
}

/// Builds the fixed GLM-5.2, Qwen3.7, and DeepSeek V4 Chat targets for Model Studio.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        chat_target("bailian-glm-5-2", z_ai::glm_5_2::ID, "glm-5.2"),
        chat_target(
            "bailian-qwen3-7-plus",
            qwen::qwen3_7_plus::ID,
            "qwen3.7-plus",
        ),
        chat_target("bailian-qwen3-7-max", qwen::qwen3_7_max::ID, "qwen3.7-max"),
        chat_target(
            "bailian-deepseek-v4-pro",
            deepseek::deepseek_v4_pro::ID,
            "deepseek-v4-pro",
        ),
        chat_target(
            "bailian-deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash",
        ),
    ]
}

/// Binds one canonical model to Model Studio's trusted Chat endpoint and credential pool.
fn chat_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
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
        upstream_apis: vec![UpstreamApiConfig {
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                CONTRACT.capabilities().chat_completions,
            ),
            state_affinity: StateAffinity::Unbound,
        }],
    }
}
