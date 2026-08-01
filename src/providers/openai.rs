//! OpenAI Provider 的编译期定义。
//!
//! 认证、请求/响应/SSE 与错误行为由 `provider::OpenAiAdapter` 实现；本文件集中声明
//! credential binding、endpoint、模型事实、target/upstream API 能力和上游 model id。

use std::time::Duration;

use bytes::Bytes;
use http::{
    HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use zeroize::Zeroizing;

use crate::{
    core::{ApiCapabilities, ApiProtocol, ApiRequest, EndpointCapabilities, ResponsesCapabilities},
    models::CONFIGURED_MODEL_ID,
    provider::{
        AdapterError, ClassifiedSseEvent, CredentialKind, CredentialValue, PreparedUpstreamRequest,
        ProviderContract, ProviderKind, RetryHint, SafeHeaders, SensitiveHeaders,
        StatusClassification, StreamEventStatus, UpstreamErrorKind,
    },
    registry::{
        CredentialConfig, StateAffinity, TransportKind, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiModelRules, UpstreamTargetConfig,
    },
    transport::sse::SseEvent,
};

/// OpenAI adapter 的静态能力与允许的 endpoint/credential 范围。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::OpenAi,
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: true,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: true,
            previous_response_id: true,
            background: false,
        },
    },
    &["public-api"],
    &[CredentialKind::ApiKey],
);

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
}

impl OpenAiAdapter {
    /// 为 OpenAI 请求构造 Bearer 认证 header。
    pub fn prepare_auth_headers(
        &self,
        credential: &CredentialValue,
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

/// 构造当前编译版本内置的 OpenAI upstream targets。
pub fn upstream_targets() -> Vec<UpstreamTargetConfig> {
    vec![UpstreamTargetConfig {
        id: "openai-main".to_owned(),
        provider: ProviderKind::OpenAi,
        model: CONFIGURED_MODEL_ID.to_owned(),
        base_url: "https://api.openai.com".to_owned(),
        credential: CredentialConfig {
            id: "openai-primary".to_owned(),
            kind: CredentialKind::ApiKey,
            environment_variable: "OPENAI_API_KEY".to_owned(),
        },
        quota_scope: None,
        fault_domain: None,
        request_timeout: Duration::from_secs(120),
        enabled: true,
        upstream_apis: upstream_apis(
            "configured-model",
            "public-api",
            conservative_openai_capabilities(),
        ),
    }]
}

fn upstream_apis(
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

/// 返回保守的 OpenAI capability 配置，需经实际上游 probe 后再扩大。
pub const fn conservative_openai_capabilities() -> ApiCapabilities {
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            previous_response_id: false,
            background: false,
        },
    }
}
