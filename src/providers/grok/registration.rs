//! Registers the fixed Grok model target and its Responses-only subscription API.
//!
//! The target shares one subscription-bound OAuth2 credential and one trusted CLI proxy origin.
//! Request-time model selection cannot change the origin, credential binding, or API surface.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    models::xai,
    provider::ProviderKind,
    registry::{
        CanonicalTaskKind, NonStreamingConversion, ProviderInstanceConfig, UpstreamApiCapabilities,
        UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules, UpstreamStreamingPolicy,
        UpstreamTargetConfig,
    },
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "grok";

/// Builds the trusted Grok Build CLI proxy deployment referenced by the fixed target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::Grok,
        base_url: "https://cli-chat-proxy.grok.com/v1".to_owned(),
    }
}

/// Builds the fixed Responses-only Grok target exposed by the compiled Route catalog.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "grok/grok-4-6".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: xai::grok_4_6::ID.to_owned(),
        provider_model: ProviderKind::Grok.routing_model_id(xai::grok_4_6::ID),
        credential_pool: "grok-cli".to_owned(),
        quota_scope: Some("grok-cli".to_owned()),
        fault_domain: Some("grok-cli-proxy-backend".to_owned()),
        timeout_policy: crate::registry::UpstreamTimeoutPolicy::new(Duration::from_secs(120)),
        enabled: true,
        upstream_apis: vec![UpstreamApiConfig {
            key: UpstreamApiKey::new(
                crate::core::OperationKind::Responses,
                CanonicalTaskKind::Generation,
            ),
            upstream_model: "grok-4.6".to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: responses_capabilities(),
            streaming_policy: UpstreamStreamingPolicy::Required {
                non_streaming: NonStreamingConversion::BufferResponsesSse,
            },
        }],
    }]
}

/// Narrows the family contract to the capabilities guaranteed by the fixed Grok target.
fn responses_capabilities() -> UpstreamApiCapabilities {
    let capabilities = DEFINITION
        .contract()
        .capabilities()
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("Grok targets require Responses capabilities");
    // The subscription proxy's image-input wire has no local evidence; declare no media input
    // until a focused probe proves it, even though the canonical model record includes images.
    UpstreamApiCapabilities::Responses(capabilities.to_executable(
        ExecutableResponsesState::new(StorageSupport::Unsupported, ResponsesAffinity::TargetBound),
        crate::core::ResponsesMediaProfile::new(None, None),
    ))
}
