//! Registers the LongCat Upstream Target and Native Upstream APIs.

use std::time::Duration;

use crate::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{CanonicalTaskKind, ProviderInstanceConfig, UpstreamTargetConfig},
};

use super::DEFINITION;

const PROVIDER_INSTANCE_ID: &str = "longcat";

/// Builds the trusted LongCat API deployment used by the checked-in target.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::LongCat,
        base_url: "https://api.longcat.chat".to_owned(),
    }
}

/// Builds the LongCat-2.0 upstream targets.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    // Resolve both operations guaranteed by the compiled LongCat Provider contract.
    let capabilities = DEFINITION.contract().capabilities();
    let chat_capabilities = capabilities
        .operation(crate::core::OperationKind::ChatCompletions)
        .and_then(crate::core::ProviderOperationCapabilities::chat_completions)
        .expect("LongCat targets require Chat Completions capabilities")
        .to_executable(None, None);
    let responses_capabilities = capabilities
        .operation(crate::core::OperationKind::Responses)
        .and_then(crate::core::ProviderOperationCapabilities::responses)
        .expect("LongCat targets require Responses capabilities")
        .to_executable(
            ExecutableResponsesState::new(
                StorageSupport::Unsupported,
                ResponsesAffinity::TargetBound,
            ),
            None,
        );

    // Bind the fixed dual-operation surface to the trusted LongCat deployment.
    vec![UpstreamTargetConfig {
        id: "longcat-2".to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        canonical_model: "meituan/longcat-2.0".to_owned(),
        provider_model: ProviderKind::LongCat.routing_model_id("meituan/longcat-2.0"),
        credential_pool: "longcat-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(
            "LongCat-2.0",
            CanonicalTaskKind::Generation,
            chat_capabilities,
            Some(responses_capabilities),
        ),
    }]
}
