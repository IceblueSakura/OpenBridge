//! OpenAI Provider 的编译期定义。
//!
//! 认证、请求/响应/SSE 与错误行为由 `provider::OpenAiAdapter` 实现；本文件集中声明
//! credential binding、endpoint、模型事实、target/offering 能力和上游 model id。

use std::time::Duration;

use bytes::Bytes;
use http::{
    HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use zeroize::Zeroizing;

use crate::{
    core::{
        CapabilitySet, Protocol, ProtocolCapabilities, ResponsesCapabilities, ValidatedRequest,
    },
    models::CONFIGURED_MODEL_ID,
    provider::{
        AuthAdapter, CapabilityAdapter, ClassifiedProviderError, CredentialKind, CredentialLease,
        DecodedEvent, ErrorAdapter, EventDisposition, HeaderAdapter, ProviderDescriptor,
        ProviderErrorClass, ProviderFailure, ProviderKind, RequestAdapter, ResponseAdapter,
        RetryHint, SafeHeaders, SensitiveHeaders, UpstreamRequestParts,
    },
    registry::{
        CredentialDefinition, ModelConstraints, NativeOfferingCapabilities,
        NativeOfferingDefinition, NativeTransport, StatePolicy, UpstreamTargetDefinition,
    },
    transport::sse::SseEvent,
};

/// OpenAI adapter 的静态能力与允许的 endpoint/credential 范围。
pub static DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::OpenAi,
    CapabilitySet {
        chat_completions: ProtocolCapabilities {
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
    pub(crate) fn encode_list_models_request(self) -> UpstreamRequestParts {
        UpstreamRequestParts::new(Method::GET, Uri::from_static("/v1/models"), Bytes::new())
    }
}

impl RequestAdapter for OpenAiAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
        upstream_model: &str,
    ) -> Result<UpstreamRequestParts, ProviderFailure> {
        let relative_uri = match request.protocol() {
            Protocol::ChatCompletions => Uri::from_static("/v1/chat/completions"),
            Protocol::Responses => Uri::from_static("/v1/responses"),
        };
        let mut document: serde_json::Value = serde_json::from_slice(request.body())
            .map_err(|_| ProviderFailure::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(ProviderFailure::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_model.to_owned()),
            );
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| ProviderFailure::InvalidRequestBody)?;

        Ok(UpstreamRequestParts::new(Method::POST, relative_uri, body))
    }
}

impl HeaderAdapter for OpenAiAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }
}

impl AuthAdapter for OpenAiAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure> {
        if credential.provider() != ProviderKind::OpenAi {
            return Err(ProviderFailure::CredentialProviderMismatch);
        }
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);
        Ok(headers)
    }
}

impl ResponseAdapter for OpenAiAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure> {
        let disposition = match protocol {
            Protocol::ChatCompletions if event.data() == "[DONE]" => EventDisposition::Completed,
            Protocol::Responses if event.event() == Some("response.completed") => {
                EventDisposition::Completed
            }
            Protocol::Responses
                if matches!(
                    event.event(),
                    Some("response.failed" | "response.incomplete")
                ) =>
            {
                EventDisposition::Failed
            }
            _ => EventDisposition::Continue,
        };
        Ok(DecodedEvent::new(event, disposition))
    }
}

impl ErrorAdapter for OpenAiAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError {
        let (class, retry_hint) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                (ProviderErrorClass::InvalidRequest, RetryHint::Never)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                (ProviderErrorClass::Authentication, RetryHint::Never)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                (ProviderErrorClass::RateLimited, RetryHint::BeforeFirstEvent)
            }
            status if status.is_server_error() => (
                ProviderErrorClass::UpstreamUnavailable,
                RetryHint::BeforeFirstEvent,
            ),
            _ => (ProviderErrorClass::UpstreamFailure, RetryHint::Never),
        };
        ClassifiedProviderError::new(class, retry_hint)
    }
}

impl CapabilityAdapter for OpenAiAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure> {
        if requested.is_subset_of(*DESCRIPTOR.capabilities()) {
            Ok(())
        } else {
            Err(ProviderFailure::UnsupportedCapabilities)
        }
    }
}

pub struct OpenAiDefinition {
    pub upstream_targets: Vec<UpstreamTargetDefinition>,
}

/// 构造当前编译版本内置的 OpenAI provider 定义。
pub fn definition() -> OpenAiDefinition {
    OpenAiDefinition {
        upstream_targets: vec![UpstreamTargetDefinition {
            id: "openai-main".to_owned(),
            provider: ProviderKind::OpenAi,
            real_model: CONFIGURED_MODEL_ID.to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            credential: CredentialDefinition {
                id: "openai-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "OPENAI_API_KEY".to_owned(),
            },
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            offerings: native_offerings(
                "configured-model",
                "public-api",
                conservative_openai_capabilities(),
            ),
        }],
    }
}

fn native_offerings(
    upstream_model: &str,
    endpoint_profile: &str,
    capabilities: CapabilitySet,
) -> Vec<NativeOfferingDefinition> {
    vec![
        NativeOfferingDefinition {
            id: "chat".to_owned(),
            protocol: Protocol::ChatCompletions,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: NativeTransport::HttpJsonSse,
            model_constraints: ModelConstraints::default(),
            capabilities: NativeOfferingCapabilities::ChatCompletions(
                capabilities.chat_completions,
            ),
            state_policy: StatePolicy::Stateless,
        },
        NativeOfferingDefinition {
            id: "responses".to_owned(),
            protocol: Protocol::Responses,
            upstream_model: upstream_model.to_owned(),
            endpoint_profile: endpoint_profile.to_owned(),
            transport: NativeTransport::HttpJsonSse,
            model_constraints: ModelConstraints::default(),
            capabilities: NativeOfferingCapabilities::Responses(capabilities.responses),
            state_policy: StatePolicy::ProviderBound,
        },
    ]
}

/// 返回保守的 OpenAI capability 配置，需经实际上游 probe 后再扩大。
pub const fn conservative_openai_capabilities() -> CapabilitySet {
    CapabilitySet {
        chat_completions: ProtocolCapabilities {
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
