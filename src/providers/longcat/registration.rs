//! Registers the LongCat Upstream Target and Native Upstream APIs.

use std::time::Duration;

use crate::{
    provider::ProviderKind, providers::openai_compatible::native_upstream_apis,
    registry::UpstreamTargetConfig,
};

use super::CONTRACT;

/// Builds the LongCat-2.0 upstream targets.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "longcat-2".to_owned(),
        provider: ProviderKind::LongCat,
        model: "meituan/longcat-2.0".to_owned(),
        base_url: "https://api.longcat.chat".to_owned(),
        credential_pool: "longcat-primary".to_owned(),
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(
            "LongCat-2.0",
            "longcat-openai",
            *CONTRACT.capabilities(),
        ),
    }]
}
