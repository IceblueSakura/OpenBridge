//! LongCat Upstream Target 与原生 Upstream API 注册。

use std::time::Duration;

use crate::{
    provider::{CredentialKind, ProviderKind},
    providers::openai_compatible::native_upstream_apis,
    registry::{CredentialConfig, UpstreamTargetConfig},
};

use super::CONTRACT;

/// 构造 LongCat-2.0 的 upstream targets。
pub(crate) fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "longcat-2".to_owned(),
        provider: ProviderKind::LongCat,
        model: "meituan/longcat-2.0".to_owned(),
        base_url: "https://api.longcat.chat".to_owned(),
        credential: CredentialConfig {
            id: "longcat-primary".to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "LONGCAT_API_KEY".to_owned(),
        },
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
