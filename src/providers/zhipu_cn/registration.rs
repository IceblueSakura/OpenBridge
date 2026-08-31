//! Registers the fixed Zhipu AI China deployment and approved GLM Targets.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport, StructuredOutputProfile},
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

/// Builds the trusted Zhipu China deployment used by the registered GLM Targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::ZhipuCn,
        base_url: "https://open.bigmodel.cn".to_owned(),
    }
}

/// Builds the fixed GLM-5.3, GLM-5.2, and GLM-5.3-Flash generation targets for Zhipu China.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        generation_target("zhipu-cn/glm-5-3", z_ai::glm_5_3::ID, "glm-5.3"),
        generation_target("zhipu-cn/glm-5-2", z_ai::glm_5_2::ID, "glm-5.2"),
        generation_target(
            "zhipu-cn/glm-5-3-flash",
            z_ai::glm_5_3_flash::ID,
            "glm-5.3-flash",
        ),
    ]
}

/// Binds one canonical GLM model to its confirmed trusted generation endpoints.
fn generation_target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
) -> UpstreamTargetConfig {
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
    capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);

    // GLM-5.3 alone has an official direct Responses endpoint; keep its unprobed optional
    // features closed while preserving the existing Chat bridge as a capability fallback.
    let mut upstream_apis = vec![UpstreamApiConfig {
        key: UpstreamApiKey::new(
            crate::core::OperationKind::ChatCompletions,
            CanonicalTaskKind::Generation,
        ),
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];
    if canonical_model == z_ai::glm_5_3::ID {
        let responses = DEFINITION
            .contract()
            .capabilities()
            .operation(crate::core::OperationKind::Responses)
            .and_then(crate::core::ProviderOperationCapabilities::responses)
            .expect("Zhipu GLM-5.3 requires Responses capabilities")
            .to_executable(
                ExecutableResponsesState::new(
                    StorageSupport::Unsupported,
                    ResponsesAffinity::Unbound,
                ),
                crate::core::ResponsesMediaProfile::default(),
            );
        upstream_apis.push(UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::Responses,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(responses),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        });
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
        upstream_apis,
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{OperationKind, ToolChoiceMode};

    use super::*;

    #[test]
    fn official_glm_surfaces_are_narrowed_per_model_and_protocol() {
        let targets = upstream_targets();
        for target in &targets {
            let chat = target
                .upstream_apis
                .iter()
                .find(|api| api.key.operation() == OperationKind::ChatCompletions)
                .unwrap();
            let UpstreamApiCapabilities::ChatCompletions(chat) = &chat.capabilities else {
                panic!("GLM Chat target must bind Chat capabilities");
            };
            assert_eq!(
                chat.structured_outputs,
                Some(StructuredOutputProfile::JsonObject)
            );
            let tools = chat
                .function_tools
                .expect("Zhipu GLM Chat targets require function tools");
            assert_eq!(tools.choice_modes, [ToolChoiceMode::Auto]);
            assert!(!tools.parallel_calls);
            assert!(!tools.strict_schema);
        }

        let glm_5_3 = targets
            .iter()
            .find(|target| target.id == "zhipu-cn/glm-5-3")
            .unwrap();
        assert!(
            glm_5_3
                .upstream_apis
                .iter()
                .any(|api| api.key.operation() == OperationKind::Responses)
        );
        for target_id in ["zhipu-cn/glm-5-2", "zhipu-cn/glm-5-3-flash"] {
            let target = targets
                .iter()
                .find(|target| target.id == target_id)
                .unwrap();
            assert!(
                target
                    .upstream_apis
                    .iter()
                    .all(|api| api.key.operation() != OperationKind::Responses)
            );
        }
    }
}
