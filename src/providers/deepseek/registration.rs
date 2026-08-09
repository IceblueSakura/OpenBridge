//! Registers DeepSeek V4 targets with model-specific Native protocol surfaces.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    models::deepseek,
    provider::ProviderKind,
    registry::{
        ProviderInstanceConfig, UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiModelRules,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "deepseek";

/// Native operation set exposed by one DeepSeek model target.
enum ModelApiSurface {
    /// Exposes only Chat Completions.
    ChatOnly,
    /// Exposes both Chat Completions and Responses.
    ChatAndResponses,
}

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
            ModelApiSurface::ChatOnly,
        ),
        target(
            "deepseek-v4-flash",
            deepseek::deepseek_v4_flash::ID,
            "deepseek-v4-flash",
            "deepseek-primary",
            ModelApiSurface::ChatAndResponses,
        ),
    ]
}

/// Builds a DeepSeek V4 target with its explicit Native operation surface.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
    api_surface: ModelApiSurface,
) -> UpstreamTargetConfig {
    // Resolve the Chat profile required by every DeepSeek target.
    let chat_capabilities = DEFINITION
        .contract()
        .capabilities()
        .chat_completions
        .expect("DeepSeek targets require Chat Completions capabilities")
        .to_executable(None);

    // Build the model-specific Native API set without introducing Target-bound state.
    let mut upstream_apis = vec![UpstreamApiConfig {
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];
    match api_surface {
        ModelApiSurface::ChatOnly => {}
        ModelApiSurface::ChatAndResponses => {
            let responses_capabilities = DEFINITION
                .contract()
                .capabilities()
                .responses
                .expect("DeepSeek Responses targets require Responses capabilities")
                .to_executable(ExecutableResponsesState::new(
                    StorageSupport::Unsupported,
                    ResponsesAffinity::Unbound,
                ));
            upstream_apis.push(UpstreamApiConfig {
                upstream_model: upstream_model.to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
                streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
            });
        }
    }

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
