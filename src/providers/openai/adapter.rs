//! OpenAI 请求、认证、SSE 与 HTTP status adapter。

use bytes::Bytes;
use http::{
    HeaderMap, HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
};
use zeroize::Zeroizing;

use crate::{
    core::{ApiCapabilities, ApiProtocol, ApiRequest},
    provider::{
        AdapterError, ClassifiedSseEvent, PreparedUpstreamRequest, ProviderKind, RetryHint,
        SafeHeaders, SensitiveHeaders, StatusClassification, StreamEventStatus, UpstreamErrorKind,
    },
    transport::sse::SseEvent,
};

use super::CONTRACT;

#[derive(Clone, Copy)]
/// OpenAI-compatible 请求与响应 adapter。
pub struct OpenAiAdapter;

impl OpenAiAdapter {
    /// 构造 OpenAI 模型列表 probe 使用的相对请求。
    pub(crate) fn prepare_model_list_request(self) -> PreparedUpstreamRequest {
        PreparedUpstreamRequest::new(Method::GET, Uri::from_static("/v1/models"), Bytes::new())
    }
}

impl OpenAiAdapter {
    /// 将下游原生请求绑定到 OpenAI endpoint，并替换上游 model 字段。
    pub fn prepare_request(
        &self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // 选择固定的 OpenAI 相对 endpoint。
        let relative_uri = match request.protocol() {
            ApiProtocol::ChatCompletions => Uri::from_static("/v1/chat/completions"),
            ApiProtocol::Responses => Uri::from_static("/v1/responses"),
        };
        // 解析并替换仅允许由 adapter 决定的上游 model 字段。
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_model.to_owned()),
            );
        // 重新序列化原生 JSON，保留其余协议字段不变。
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;

        Ok(PreparedUpstreamRequest::new(
            Method::POST,
            relative_uri,
            body,
        ))
    }
}

impl OpenAiAdapter {
    /// 构造 OpenAI JSON 请求的普通 header。
    pub fn prepare_headers(&self) -> Result<SafeHeaders, AdapterError> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }

    /// 选择允许由下游覆盖的 OpenAI 普通请求 header。
    pub fn apply_request_header_hook(
        &self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        headers.override_from(downstream_headers, USER_AGENT)
    }
}

impl OpenAiAdapter {
    /// 为 OpenAI 请求构造 Bearer 认证 header。
    pub fn prepare_auth_headers(
        &self,
        credential: &crate::credential::UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        // 校验 credential provider 归属，避免跨 provider 复用 secret。
        if credential.provider() != ProviderKind::OpenAi {
            return Err(AdapterError::CredentialProviderMismatch);
        }
        // 在 zeroizing 字符串中组装敏感 Bearer header。
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);
        Ok(headers)
    }
}

impl OpenAiAdapter {
    /// 按 Chat/Responses 协议识别 OpenAI SSE terminal 或 failure event。
    pub fn classify_sse_event(
        &self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> Result<ClassifiedSseEvent, AdapterError> {
        // 按协议识别正常终止和 provider failure event。
        let status = match protocol {
            ApiProtocol::ChatCompletions if event.data() == "[DONE]" => {
                StreamEventStatus::Completed
            }
            ApiProtocol::Responses if event.event() == Some("response.completed") => {
                StreamEventStatus::Completed
            }
            ApiProtocol::Responses
                if matches!(
                    event.event(),
                    Some("response.failed" | "response.incomplete")
                ) =>
            {
                StreamEventStatus::Failed
            }
            _ => StreamEventStatus::Continue,
        };
        Ok(ClassifiedSseEvent::new(event, status))
    }
}

impl OpenAiAdapter {
    /// 将 OpenAI HTTP status 映射为重试分类。
    pub fn classify_status(&self, status: StatusCode) -> StatusClassification {
        // 将 status 映射为粗粒度错误类别和未输出前的重试边界。
        let (class, retry_hint) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                (UpstreamErrorKind::InvalidRequest, RetryHint::Never)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                (UpstreamErrorKind::Authentication, RetryHint::Never)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                (UpstreamErrorKind::RateLimited, RetryHint::BeforeFirstEvent)
            }
            status if status.is_server_error() => (
                UpstreamErrorKind::UpstreamUnavailable,
                RetryHint::BeforeFirstEvent,
            ),
            _ => (UpstreamErrorKind::UpstreamFailure, RetryHint::Never),
        };
        StatusClassification::new(class, retry_hint)
    }
}

impl OpenAiAdapter {
    /// 校验请求能力没有超过 OpenAI adapter 的静态契约。
    pub fn validate_capabilities(&self, requested: ApiCapabilities) -> Result<(), AdapterError> {
        if requested.is_subset_of(*CONTRACT.capabilities()) {
            Ok(())
        } else {
            Err(AdapterError::UnsupportedCapabilities)
        }
    }
}
