//! OpenAI-compatible Provider 共享的 HTTP JSON/SSE wire 实现。
//!
//! Provider 身份、能力、endpoint path 与 request-header hook 仍由各 Provider 的编译期定义拥有；
//! 本模块只复用协议机制，不提供动态 Provider DSL 或运行时转换配置。

use bytes::Bytes;
use http::{
    HeaderMap, HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use zeroize::Zeroizing;

use crate::{
    core::{ApiCapabilities, ApiProtocol, ApiRequest},
    credential::CredentialType,
    provider::{
        AdapterError, ClassifiedSseEvent, PreparedUpstreamRequest, ProviderContract, ProviderKind,
        RetryHint, SafeHeaders, SensitiveHeaders, StatusClassification, StreamEventStatus,
        UpstreamErrorKind,
    },
    registry::{
        StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules,
    },
    transport::sse::SseEvent,
};

/// Provider 编译期 hook，可按自身协议规则转换普通请求头。
pub(crate) type RequestHeaderHook = fn(&HeaderMap, &mut SafeHeaders) -> Result<(), AdapterError>;

#[derive(Clone, Copy)]
enum ResponsesTerminalProfile {
    OpenAiEventField,
    OpenRouterDataOnlyDone,
}

/// 一个静态 OpenAI-compatible wire profile。
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleAdapter {
    kind: ProviderKind,
    contract: &'static ProviderContract,
    chat_path: Option<&'static str>,
    responses_path: Option<&'static str>,
    model_list_path: &'static str,
    request_header_hook: RequestHeaderHook,
    responses_terminal_profile: ResponsesTerminalProfile,
}

impl OpenAiCompatibleAdapter {
    /// 构造由具体 Provider 拥有的静态 wire profile。
    pub(crate) const fn new(
        kind: ProviderKind,
        contract: &'static ProviderContract,
        chat_path: Option<&'static str>,
        responses_path: Option<&'static str>,
        model_list_path: &'static str,
        request_header_hook: RequestHeaderHook,
    ) -> Self {
        Self {
            kind,
            contract,
            chat_path,
            responses_path,
            model_list_path,
            request_header_hook,
            responses_terminal_profile: ResponsesTerminalProfile::OpenAiEventField,
        }
    }

    /// 使用 OpenRouter 在 data JSON 中声明的 `response.done` 终态格式。
    pub(crate) const fn with_openrouter_responses_terminal(mut self) -> Self {
        self.responses_terminal_profile = ResponsesTerminalProfile::OpenRouterDataOnlyDone;
        self
    }

    /// 返回 profile 绑定的 Provider 契约。
    pub(crate) fn contract(self) -> &'static ProviderContract {
        self.contract
    }

    /// 构造模型列表 probe 使用的相对请求。
    pub(crate) fn prepare_model_list_request(self) -> PreparedUpstreamRequest {
        PreparedUpstreamRequest::new(
            Method::GET,
            Uri::from_static(self.model_list_path),
            Bytes::new(),
        )
    }

    /// 替换上游 model，并绑定 profile 声明的相对 endpoint。
    pub(crate) fn prepare_request(
        self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // 按请求协议选择静态相对 endpoint。
        let path = match request.protocol() {
            ApiProtocol::ChatCompletions => self.chat_path,
            ApiProtocol::Responses => self.responses_path,
        }
        .ok_or(AdapterError::UnsupportedProtocol)?;
        let relative_uri = Uri::from_static(path);

        // 解析并替换只能由 adapter 决定的上游 model 字段。
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

    /// 构造 OpenAI-compatible JSON 请求的基础普通 header。
    pub(crate) fn prepare_headers(self) -> Result<SafeHeaders, AdapterError> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }

    /// 执行具体 Provider 编译期定义的普通请求头转换。
    pub(crate) fn apply_request_header_hook(
        self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        (self.request_header_hook)(downstream_headers, headers)
    }

    /// 构造与 Provider 身份绑定的 Bearer 认证 header。
    pub(crate) fn prepare_auth_headers(
        self,
        credential: &crate::credential::UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        // 校验 credential Provider 归属，避免跨 Provider 复用 secret。
        if credential.provider() != self.kind {
            return Err(AdapterError::CredentialProviderMismatch);
        }
        let CredentialType::Upstream(kind) = credential.metadata().credential_type() else {
            return Err(AdapterError::CredentialKindMismatch);
        };
        if !self.contract.credential_kinds().contains(&kind) {
            return Err(AdapterError::CredentialKindMismatch);
        }

        // 在 zeroizing 字符串中组装敏感 Bearer header。
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);
        Ok(headers)
    }

    /// 识别 OpenAI-compatible Chat/Responses SSE terminal 或 failure event。
    pub(crate) fn classify_sse_event(
        self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> ClassifiedSseEvent {
        let status = match protocol {
            ApiProtocol::ChatCompletions if event.data() == "[DONE]" => {
                StreamEventStatus::Completed
            }
            ApiProtocol::Responses => self.classify_responses_sse_event(&event),
            _ => StreamEventStatus::Continue,
        };
        ClassifiedSseEvent::new(event, status)
    }

    /// 按具体 OpenAI-compatible profile 识别 Responses SSE 终态。
    fn classify_responses_sse_event(self, event: &SseEvent) -> StreamEventStatus {
        match self.responses_terminal_profile {
            ResponsesTerminalProfile::OpenAiEventField => match event.event() {
                Some("response.completed") => StreamEventStatus::Completed,
                Some("response.failed" | "response.incomplete") => StreamEventStatus::Failed,
                _ => StreamEventStatus::Continue,
            },
            ResponsesTerminalProfile::OpenRouterDataOnlyDone => {
                classify_openrouter_responses_done(event)
            }
        }
    }

    /// 将 OpenAI-compatible HTTP status 映射为错误与重试分类。
    pub(crate) fn classify_status(self, status: StatusCode) -> StatusClassification {
        let (kind, retry_hint) = match status {
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
        StatusClassification::new(kind, retry_hint)
    }

    /// 校验请求能力没有超过具体 Provider 的静态契约。
    pub(crate) fn validate_capabilities(
        self,
        requested: ApiCapabilities,
    ) -> Result<(), AdapterError> {
        if requested.is_subset_of(*self.contract.capabilities()) {
            Ok(())
        } else {
            Err(AdapterError::UnsupportedCapabilities)
        }
    }
}

/// 从 OpenRouter data-only `response.done` event 提取终态，未知形状保持非终态。
fn classify_openrouter_responses_done(event: &SseEvent) -> StreamEventStatus {
    // 解析最小终态 envelope，不保留或记录业务正文。
    let Ok(document) = serde_json::from_str::<serde_json::Value>(event.data()) else {
        return StreamEventStatus::Continue;
    };
    if document.get("type").and_then(serde_json::Value::as_str) != Some("response.done") {
        return StreamEventStatus::Continue;
    }

    // 依据 response status 区分正常完成与失败终态。
    match document
        .get("response")
        .and_then(|response| response.get("status"))
        .and_then(serde_json::Value::as_str)
    {
        Some("completed") => StreamEventStatus::Completed,
        Some("failed" | "incomplete") => StreamEventStatus::Failed,
        _ => StreamEventStatus::Continue,
    }
}

/// 构造一对显式 Chat/Responses HTTP JSON/SSE Upstream API。
pub(crate) fn native_upstream_apis(
    upstream_model: &str,
    endpoint_profile: &str,
    capabilities: ApiCapabilities,
) -> Vec<UpstreamApiConfig> {
    vec![
        UpstreamApiConfig {
            id: "chat".to_owned(),
            protocol: ApiProtocol::ChatCompletions,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::ChatCompletions(capabilities.chat_completions),
            state_affinity: StateAffinity::Unbound,
        },
        UpstreamApiConfig {
            id: "responses".to_owned(),
            protocol: ApiProtocol::Responses,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: TransportKind::HttpJsonSse,
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(capabilities.responses),
            state_affinity: StateAffinity::TargetBound,
        },
    ]
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue};

    use super::*;

    fn transform_headers(
        downstream: &HeaderMap,
        upstream: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        let source = HeaderName::from_static("x-source-name");
        let target = HeaderName::from_static("x-target-name");
        if let Some(value) = downstream.get(source) {
            upstream.insert(target, value.clone())?;
        }
        upstream.remove(CONTENT_TYPE);
        Ok(())
    }

    #[test]
    fn provider_hook_can_transform_and_drop_regular_headers() {
        let adapter = OpenAiCompatibleAdapter::new(
            ProviderKind::OpenAi,
            &crate::providers::openai::CONTRACT,
            Some("/chat"),
            Some("/responses"),
            "/models",
            transform_headers,
        );
        let mut downstream = HeaderMap::new();
        downstream.insert(
            HeaderName::from_static("x-source-name"),
            HeaderValue::from_static("transformed-value"),
        );
        let mut upstream = adapter.prepare_headers().unwrap();

        adapter
            .apply_request_header_hook(&downstream, &mut upstream)
            .unwrap();

        assert!(upstream.get(CONTENT_TYPE).is_none());
        assert_eq!(
            upstream
                .get(HeaderName::from_static("x-target-name"))
                .unwrap(),
            "transformed-value"
        );
    }
}
