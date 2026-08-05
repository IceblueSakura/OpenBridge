//! Registers Xiaomi MiMo V2.5 Upstream Targets and dual-protocol Upstream APIs.

use std::time::Duration;

use crate::{
    models::xiaomi,
    provider::ProviderKind,
    providers::openai_compatible::native_upstream_apis,
    registry::{ProviderInstanceConfig, UpstreamTargetConfig},
};

use super::CONTRACT;

const PROVIDER_INSTANCE_ID: &str = "mimo";

/// Builds the trusted MiMo API deployment used by the checked-in targets.
pub(crate) fn provider_instance() -> ProviderInstanceConfig {
    ProviderInstanceConfig {
        id: PROVIDER_INSTANCE_ID.to_owned(),
        kind: ProviderKind::MiMo,
        base_url: "https://api.xiaomimimo.com".to_owned(),
    }
}

/// Builds the fixed upstream targets for MiMo V2.5 Pro and V2.5.
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "mimo-v2-5-pro",
            xiaomi::mimo_v2_5_pro::ID,
            "mimo-v2.5-pro",
            "mimo-primary",
        ),
        target(
            "mimo-v2-5",
            xiaomi::mimo_v2_5::ID,
            "mimo-v2.5",
            "mimo-primary",
        ),
    ]
}

/// Builds a Chat/Responses target for a MiMo V2.5 model.
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider_instance: PROVIDER_INSTANCE_ID.to_owned(),
        model: canonical_model.to_owned(),
        credential_pool: credential_id.to_owned(),
        quota_scope: Some("mimo-primary".to_owned()),
        fault_domain: Some("mimo-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(upstream_model, *CONTRACT.capabilities()),
    }
}
