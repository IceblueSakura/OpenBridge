//! 编译期 provider 契约与闭合 dispatch。
//!
//! 路由配置只能选择 `ProviderKind` 中已经编译的变体；adapter 负责相对 path、安全/敏感
//! header 的分离、认证材料组装、SSE terminal 判定和重试分类。HTTP ingress 不需要知道
//! provider 认证格式，也不能用运行时配置注入任意 header 或请求转换。

mod contracts;
mod credential;

pub use contracts::{
    AuthAdapter, CapabilityAdapter, ClassifiedProviderError, DecodedEvent, ErrorAdapter,
    EventDisposition, HeaderAdapter, ProviderErrorClass, ResponseAdapter, RetryHint, SafeHeaders,
    SensitiveHeaders,
};
pub use credential::{CredentialLease, CredentialSource, CredentialSourceError};

use bytes::Bytes;
use http::{Method, StatusCode, Uri};
use thiserror::Error;

use crate::{
    core::{CapabilitySet, Protocol, ValidatedRequest},
    providers::openai::{self, OpenAiAdapter},
    transport::sse::SseEvent,
};

/// 可由 route 配置引用的闭合 provider 集合。
///
/// 新 provider 必须新增 enum 变体及其 adapter/tests；未知字符串在配置加载时失败，不能
/// 退化为“通用 HTTP provider”。这让认证与协议行为保持可审查、可编译的边界。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    OpenAi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    ApiKey,
}

/// provider 的静态能力与可配置范围。
///
/// deployment capability 只能收窄此描述符，不能自行声明 adapter 未实现的特性；endpoint
/// profile 与 credential kind 同样由这里限制，避免 route TOML 变成动态 provider DSL。
#[derive(Debug)]
pub struct ProviderDescriptor {
    kind: ProviderKind,
    capabilities: CapabilitySet,
    endpoint_profiles: &'static [&'static str],
    credential_kinds: &'static [CredentialKind],
}

impl ProviderDescriptor {
    pub const fn new(
        kind: ProviderKind,
        capabilities: CapabilitySet,
        endpoint_profiles: &'static [&'static str],
        credential_kinds: &'static [CredentialKind],
    ) -> Self {
        Self {
            kind,
            capabilities,
            endpoint_profiles,
            credential_kinds,
        }
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn endpoint_profiles(&self) -> &'static [&'static str] {
        self.endpoint_profiles
    }

    pub fn credential_kinds(&self) -> &'static [CredentialKind] {
        self.credential_kinds
    }
}

impl ProviderKind {
    pub fn descriptor(self) -> &'static ProviderDescriptor {
        match self {
            Self::OpenAi => &openai::DESCRIPTOR,
        }
    }

    pub(crate) fn capabilities(self) -> CapabilitySet {
        *self.descriptor().capabilities()
    }

    pub(crate) fn accepts_endpoint_profile(self, profile: &str) -> bool {
        self.descriptor().endpoint_profiles().contains(&profile)
    }

    pub(crate) fn accepts_credential_kind(self, credential: CredentialKind) -> bool {
        self.descriptor().credential_kinds().contains(&credential)
    }
}

#[derive(Debug, Error)]
pub enum ProviderFailure {
    #[error("request protocol is not supported by this provider adapter")]
    UnsupportedProtocol,
    #[error("credential lease identity does not match the provider adapter")]
    CredentialProviderMismatch,
    #[error("sensitive header cannot be emitted by HeaderAdapter")]
    SensitiveHeaderInSafeSet,
    #[error("requested capabilities are not supported by the provider adapter")]
    UnsupportedCapabilities,
    #[error("validated request body could not be transformed by the provider adapter")]
    InvalidRequestBody,
    #[error("provider authentication material cannot be encoded as an HTTP header")]
    InvalidAuthenticationHeader,
}

/// 已经选择协议、但尚未绑定 deployment origin 的上游请求。
///
/// adapter 只能产生相对 URI；transport 将其与配置中已 allowlist 的 origin 拼接。这是阻止
/// provider adapter 或下游请求绕过 egress allowlist 的第二道边界。
#[derive(Clone)]
pub struct UpstreamRequestParts {
    method: Method,
    relative_uri: Uri,
    body: Bytes,
}

impl UpstreamRequestParts {
    pub(crate) fn new(method: Method, relative_uri: Uri, body: Bytes) -> Self {
        Self {
            method,
            relative_uri,
            body,
        }
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn relative_uri(&self) -> &Uri {
        &self.relative_uri
    }

    pub fn body(&self) -> &Bytes {
        &self.body
    }
}

pub trait RequestAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
        upstream_model: &str,
    ) -> Result<UpstreamRequestParts, ProviderFailure>;
}

#[derive(Clone, Copy)]
pub enum ProviderAdapter {
    OpenAi(OpenAiAdapter),
}

impl ProviderAdapter {
    pub fn for_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::OpenAi => Self::OpenAi(OpenAiAdapter),
        }
    }

    pub fn descriptor(&self) -> &'static ProviderDescriptor {
        match self {
            Self::OpenAi(_) => ProviderKind::OpenAi.descriptor(),
        }
    }

    pub(crate) fn build_outbound_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<http::HeaderMap, ProviderFailure> {
        let mut headers = self.build_headers()?.into_inner();
        self.build_auth_headers(credential)?
            .append_to(&mut headers)?;
        Ok(headers)
    }

    /// 由编译期 adapter 固定生成的上游模型发现请求。
    ///
    /// 该请求只用于管理员显式 probe；它不会成为下游 `/v1/models` 的实现，后者始终
    /// 只暴露 OpenBridge 的 public alias。
    pub(crate) fn encode_list_models_request(&self) -> UpstreamRequestParts {
        match self {
            Self::OpenAi(adapter) => adapter.encode_list_models_request(),
        }
    }
}

impl RequestAdapter for ProviderAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
        upstream_model: &str,
    ) -> Result<UpstreamRequestParts, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.encode_request(request, upstream_model),
        }
    }
}

impl HeaderAdapter for ProviderAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.build_headers(),
        }
    }
}

impl AuthAdapter for ProviderAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.build_auth_headers(credential),
        }
    }
}

impl ResponseAdapter for ProviderAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.decode_event(protocol, event),
        }
    }
}

impl ErrorAdapter for ProviderAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError {
        match self {
            Self::OpenAi(adapter) => adapter.classify_status(status),
        }
    }
}

impl CapabilityAdapter for ProviderAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure> {
        match self {
            Self::OpenAi(adapter) => adapter.validate_capabilities(requested),
        }
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderValue, header::AUTHORIZATION};
    use secrecy::SecretString;

    use super::*;

    #[test]
    fn safe_header_debug_output_omits_values() {
        let mut headers = SafeHeaders::default();
        headers
            .insert(
                http::HeaderName::from_static("x-provider-metadata"),
                HeaderValue::from_static("metadata-test-value"),
            )
            .unwrap();

        assert!(!format!("{headers:?}").contains("metadata-test-value"));
    }

    #[test]
    fn safe_headers_reject_authentication_material() {
        let mut headers = SafeHeaders::default();

        let error = headers
            .insert(AUTHORIZATION, HeaderValue::from_static("forbidden"))
            .unwrap_err();

        assert!(matches!(error, ProviderFailure::SensitiveHeaderInSafeSet));
    }

    #[test]
    fn openai_auth_adapter_builds_the_expected_bearer_value_inside_the_crate_boundary() {
        let adapter = OpenAiAdapter;
        let lease = CredentialLease::new(
            ProviderKind::OpenAi,
            "binding",
            "version",
            SecretString::from("credential-test-value".to_owned()),
        );

        let headers = adapter.build_auth_headers(&lease).unwrap();

        assert_eq!(
            headers.expose(AUTHORIZATION),
            Some("Bearer credential-test-value")
        );
    }
}
