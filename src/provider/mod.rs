//! 编译期 provider 契约与闭合 dispatch。
//!
//! 路由配置只能选择 `ProviderKind` 中已经编译的变体；adapter 负责相对 path、安全/敏感
//! header 的分离、认证材料组装、SSE terminal 判定和重试分类。HTTP ingress 不需要知道
//! provider 认证格式。下游 header 只能经 Provider 的受信 hook 选择后写入 `SafeHeaders`，
//! 不能用运行时配置注入任意 header 或请求转换。

mod contracts;

use bytes::Bytes;
pub use contracts::{
    ClassifiedSseEvent, RetryHint, SafeHeaders, SensitiveHeaders, StatusClassification,
    StreamEventStatus, UpstreamErrorKind,
};
use http::{HeaderMap, Method, StatusCode, Uri};
use thiserror::Error;

use crate::{
    core::{ApiCapabilities, ApiProtocol, ApiRequest},
    credential::UpstreamCredential,
    providers::{
        longcat::{self, LongCatAdapter},
        openai::{self, OpenAiAdapter},
    },
    transport::sse::SseEvent,
};

/// 可由 route 配置引用的闭合 provider 集合。
///
/// 新 provider 必须新增 enum 变体及其 adapter/tests；未知字符串在配置加载时失败，不能
/// 退化为“通用 HTTP provider”。这让认证与协议行为保持可审查、可编译的边界。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    /// OpenAI-compatible provider。
    OpenAi,
    /// LongCat OpenAI-compatible provider。
    LongCat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// provider 支持的 credential 类型。
pub enum CredentialKind {
    /// 使用 HTTP Bearer API key。
    ApiKey,
}

/// provider 的静态能力与可配置范围。
///
/// Upstream API capability 只能收窄此契约，不能自行声明 adapter 未实现的特性；endpoint
/// profile 与 credential kind 同样由这里限制，避免 route TOML 变成动态 provider DSL。
#[derive(Debug)]
pub struct ProviderContract {
    kind: ProviderKind,
    capabilities: ApiCapabilities,
    endpoint_profiles: &'static [&'static str],
    credential_kinds: &'static [CredentialKind],
}

impl ProviderContract {
    /// 创建 provider 的静态契约。
    pub const fn new(
        kind: ProviderKind,
        capabilities: ApiCapabilities,
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

    /// 返回契约对应的 provider kind。
    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// 返回 adapter 支持的能力上界。
    pub fn capabilities(&self) -> &ApiCapabilities {
        &self.capabilities
    }

    /// 返回允许配置的 endpoint profile 名称。
    pub fn endpoint_profiles(&self) -> &'static [&'static str] {
        self.endpoint_profiles
    }

    /// 返回允许配置的 credential 类型。
    pub fn credential_kinds(&self) -> &'static [CredentialKind] {
        self.credential_kinds
    }
}

impl ProviderKind {
    /// 返回该 provider 的编译期契约。
    pub fn contract(self) -> &'static ProviderContract {
        match self {
            Self::OpenAi => &openai::CONTRACT,
            Self::LongCat => &longcat::CONTRACT,
        }
    }

    pub(crate) fn capabilities(self) -> ApiCapabilities {
        *self.contract().capabilities()
    }

    pub(crate) fn accepts_endpoint_profile(self, profile: &str) -> bool {
        self.contract().endpoint_profiles().contains(&profile)
    }

    pub(crate) fn accepts_credential_kind(self, credential: CredentialKind) -> bool {
        self.contract().credential_kinds().contains(&credential)
    }
}

/// provider adapter 在请求、认证、响应或能力校验阶段报告的失败。
#[derive(Debug, Error)]
pub enum AdapterError {
    /// 请求协议不在 adapter 的支持范围内。
    #[error("request protocol is not supported by this provider adapter")]
    UnsupportedProtocol,
    /// credential 所属 provider 与 adapter 不一致。
    #[error("credential provider does not match the provider adapter")]
    CredentialProviderMismatch,
    /// 敏感 header 被错误地放入普通 header 集合。
    #[error("sensitive header cannot be emitted as a regular provider header")]
    SensitiveHeaderInSafeSet,
    /// 请求声明了 adapter 不支持的 capability。
    #[error("requested capabilities are not supported by the provider adapter")]
    UnsupportedCapabilities,
    /// 请求正文无法解析或改写为合法 JSON object。
    #[error("request body could not be transformed by the provider adapter")]
    InvalidRequestBody,
    /// credential 无法编码为合法 HTTP header。
    #[error("provider authentication material cannot be encoded as an HTTP header")]
    InvalidAuthenticationHeader,
}

/// 已经选择协议、但尚未绑定 Upstream Target origin 的上游请求。
///
/// adapter 只能产生相对 URI；transport 将其与配置中已 allowlist 的 origin 拼接。这是阻止
/// provider adapter 或下游请求绕过 egress allowlist 的第二道边界。
#[derive(Clone)]
pub struct PreparedUpstreamRequest {
    method: Method,
    relative_uri: Uri,
    body: Bytes,
}

impl PreparedUpstreamRequest {
    pub(crate) fn new(method: Method, relative_uri: Uri, body: Bytes) -> Self {
        Self {
            method,
            relative_uri,
            body,
        }
    }

    /// 返回 HTTP method。
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// 返回不含 authority 的相对 URI。
    pub fn relative_uri(&self) -> &Uri {
        &self.relative_uri
    }

    /// 返回改写后的请求 body。
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}

/// 已编译 provider adapter 的闭合集合。
#[derive(Clone, Copy)]
/// 已编译 provider adapter 的 dispatch 变体。
pub enum ProviderAdapter {
    /// OpenAI adapter。
    OpenAi(OpenAiAdapter),
    /// LongCat adapter。
    LongCat(LongCatAdapter),
}

impl ProviderAdapter {
    /// 根据注册表中的 provider kind 选择 adapter。
    pub fn for_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::OpenAi => Self::OpenAi(OpenAiAdapter),
            ProviderKind::LongCat => Self::LongCat(LongCatAdapter),
        }
    }

    /// 返回 adapter 的静态 provider 契约。
    pub fn contract(&self) -> &'static ProviderContract {
        match self {
            Self::OpenAi(_) => ProviderKind::OpenAi.contract(),
            Self::LongCat(_) => ProviderKind::LongCat.contract(),
        }
    }

    pub(crate) fn build_outbound_headers(
        &self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
    ) -> Result<HeaderMap, AdapterError> {
        // 构造 Provider 基础 header，并运行只允许写入 SafeHeaders 的请求 hook。
        let mut safe_headers = self.prepare_headers()?;
        self.apply_request_header_hook(downstream_headers, &mut safe_headers)?;
        let mut headers = safe_headers.into_inner();

        // 最后附加 credential adapter 生成的敏感 header，避免下游 hook 覆盖认证材料。
        self.prepare_auth_headers(credential)?
            .append_to(&mut headers)?;
        Ok(headers)
    }

    /// 运行 Provider 的受信 request-header hook。
    ///
    /// hook 只能从下游选择显式允许的普通 header 并写入 `SafeHeaders`，不能写入认证、
    /// cookie、Host 或 proxy authentication header。
    pub fn apply_request_header_hook(
        &self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.apply_request_header_hook(downstream_headers, headers),
            Self::LongCat(adapter) => {
                adapter.apply_request_header_hook(downstream_headers, headers)
            }
        }
    }

    /// 由编译期 adapter 固定生成的上游模型发现请求。
    ///
    /// 该请求只用于管理员显式 probe；它不会成为下游 `/v1/models` 的实现，后者始终
    /// 只暴露 OpenBridge 的 Public Model。
    pub(crate) fn prepare_model_list_request(&self) -> PreparedUpstreamRequest {
        match self {
            Self::OpenAi(adapter) => adapter.prepare_model_list_request(),
            Self::LongCat(adapter) => adapter.prepare_model_list_request(),
        }
    }

    /// 按下游协议和上游模型 id 构造相对上游请求。
    pub fn prepare_request(
        &self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.prepare_request(request, upstream_model),
            Self::LongCat(adapter) => adapter.prepare_request(request, upstream_model),
        }
    }

    /// 构造不包含认证材料的安全请求头。
    pub fn prepare_headers(&self) -> Result<SafeHeaders, AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.prepare_headers(),
            Self::LongCat(adapter) => adapter.prepare_headers(),
        }
    }

    /// 构造只在 egress 前附加的敏感认证请求头。
    pub fn prepare_auth_headers(
        &self,
        credential: &UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.prepare_auth_headers(credential),
            Self::LongCat(adapter) => adapter.prepare_auth_headers(credential),
        }
    }

    /// 判断一个已完成 framing 的 SSE event 是否终止或失败。
    pub fn classify_sse_event(
        &self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> Result<ClassifiedSseEvent, AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.classify_sse_event(protocol, event),
            Self::LongCat(adapter) => adapter.classify_sse_event(protocol, event),
        }
    }

    /// 将上游 HTTP status 映射为粗粒度错误和重试边界。
    pub fn classify_status(&self, status: StatusCode) -> StatusClassification {
        match self {
            Self::OpenAi(adapter) => adapter.classify_status(status),
            Self::LongCat(adapter) => adapter.classify_status(status),
        }
    }

    /// 在发送请求前校验请求能力是否属于 adapter 的静态上界。
    pub fn validate_capabilities(&self, requested: ApiCapabilities) -> Result<(), AdapterError> {
        match self {
            Self::OpenAi(adapter) => adapter.validate_capabilities(requested),
            Self::LongCat(adapter) => adapter.validate_capabilities(requested),
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

        assert!(matches!(error, AdapterError::SensitiveHeaderInSafeSet));
    }

    #[test]
    fn openai_auth_adapter_builds_the_expected_bearer_value_inside_the_crate_boundary() {
        let adapter = OpenAiAdapter;
        let mut credentials = crate::credential::CredentialStoreBuilder::new();
        credentials
            .insert_upstream(
                ProviderKind::OpenAi,
                "binding",
                SecretString::from("credential-test-value".to_owned()),
            )
            .unwrap();
        let credentials = credentials.build();
        let credential = credentials
            .upstream(ProviderKind::OpenAi, "binding")
            .unwrap();

        let headers = adapter.prepare_auth_headers(&credential).unwrap();

        assert_eq!(
            headers.expose(AUTHORIZATION),
            Some("Bearer credential-test-value")
        );
    }
}
