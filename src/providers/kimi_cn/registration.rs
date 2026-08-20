//! Registers the fixed Moonshot Kimi China deployment and approved model Target.

use std::time::Duration;

use crate::{
    models::moonshotai,
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, IgnorableGenerationParameter, ProviderInstanceConfig,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "kimi-cn";
const CREDENTIAL_POOL_ID: &str = "kimi-primary";

/// Builds the trusted Moonshot China deployment used by the approved Kimi K3 Target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::KimiCn,
        base_url: "https://api.moonshot.cn".to_owned(),
    }
}

/// Builds the fixed Kimi K3 Chat target for the Moonshot China API.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![chat_target(
        "kimi-cn-kimi-k3",
        moonshotai::kimi_k3::ID,
        "kimi-k3",
    )]
}

/// Binds one canonical model to Moonshot's trusted Chat endpoint and credential pool.
fn chat_target(id: &str, canonical_model: &str, upstream_model: &str) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: canonical_model.to_owned(),
        provider_model: ProviderKind::KimiCn.routing_model_id(canonical_model),
        credential_pool: CREDENTIAL_POOL_ID.to_owned(),
        quota_scope: Some(CREDENTIAL_POOL_ID.to_owned()),
        fault_domain: Some("kimi-cn-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::ChatCompletions,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules {
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
            },
            capabilities: UpstreamApiCapabilities::ChatCompletions(
                DEFINITION
                    .contract()
                    .capabilities()
                    .operation(crate::core::OperationKind::ChatCompletions)
                    .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
                    .expect("Kimi China targets require Chat Completions capabilities")
                    .to_executable(None, None),
            ),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        }],
    }
}
