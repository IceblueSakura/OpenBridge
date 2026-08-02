//! Xiaomi MiMo V2.5 Upstream Target 与双协议 Upstream API 注册。

use std::time::Duration;

use crate::{
    models::mimo,
    provider::{CredentialKind, ProviderKind},
    providers::openai_compatible::native_upstream_apis,
    registry::{CredentialConfig, UpstreamTargetConfig},
};

use super::CONTRACT;

/// 构造 MiMo V2.5 Pro 与 V2.5 的固定 upstream targets。
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![
        target(
            "mimo-v2-5-pro",
            mimo::v2_5_pro::ID,
            "mimo-v2.5-pro",
            "mimo-v2-5-pro",
        ),
        target("mimo-v2-5", mimo::v2_5::ID, "mimo-v2.5", "mimo-v2-5"),
    ]
}

/// 为一个 MiMo V2.5 模型构造 Chat/Responses target。
fn target(
    id: &str,
    canonical_model: &str,
    upstream_model: &str,
    credential_id: &str,
) -> UpstreamTargetConfig {
    UpstreamTargetConfig {
        id: id.to_owned(),
        provider: ProviderKind::MiMo,
        model: canonical_model.to_owned(),
        base_url: "https://api.xiaomimimo.com".to_owned(),
        credential: CredentialConfig {
            id: credential_id.to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "MIMO_API_KEY".to_owned(),
        },
        quota_scope: Some("mimo-primary".to_owned()),
        fault_domain: Some("mimo-api".to_owned()),
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: native_upstream_apis(
            upstream_model,
            "mimo-openai",
            *CONTRACT.capabilities(),
        ),
    }
}
