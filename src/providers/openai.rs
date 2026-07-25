//! OpenAI Provider 的编译期定义。
//!
//! 认证、请求/响应/SSE 与错误行为由 `provider::OpenAiAdapter` 实现；本文件集中声明
//! credential binding、endpoint、模型事实、deployment 能力和上游 model id。

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
    provider::{
        AuthAdapter, CapabilityAdapter, ClassifiedProviderError, CredentialKind, CredentialLease,
        DecodedEvent, ErrorAdapter, EventDisposition, HeaderAdapter, ProviderDescriptor,
        ProviderErrorClass, ProviderFailure, ProviderKind, RequestAdapter, ResponseAdapter,
        RetryHint, SafeHeaders, SensitiveHeaders, UpstreamRequestParts,
    },
    registry::{
        CredentialDefinition, DeploymentDefinition, ModelConstraints, ModelContextLength,
        ModelDefinition, ProviderDefinition, ReasoningSupport,
    },
    transport::sse::SseEvent,
};

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
    pub provider: ProviderDefinition,
    pub models: Vec<ModelDefinition>,
    pub deployments: Vec<DeploymentDefinition>,
}

pub fn definition() -> OpenAiDefinition {
    OpenAiDefinition {
        provider: ProviderDefinition {
            id: "openai".to_owned(),
            kind: ProviderKind::OpenAi,
            credential: CredentialDefinition {
                id: "openai-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "OPENAI_API_KEY".to_owned(),
            },
        },
        models: vec![ModelDefinition {
            id: "openai/configured-model".to_owned(),
            name: "Configured OpenAI-compatible model".to_owned(),
            description: Some(
                "Replace this placeholder with metadata verified for the real upstream model."
                    .to_owned(),
            ),
            context_length: ModelContextLength::default(),
            supported_parameters: Vec::new(),
            reasoning: ReasoningSupport::Unknown,
            reasoning_levels: Vec::new(),
        }],
        deployments: vec![DeploymentDefinition {
            id: "openai-main".to_owned(),
            provider: "openai".to_owned(),
            model: "openai/configured-model".to_owned(),
            upstream_model: "configured-model".to_owned(),
            endpoint_profile: "public-api".to_owned(),
            base_url: "https://api.openai.com".to_owned(),
            request_timeout: Duration::from_secs(120),
            model_constraints: ModelConstraints::default(),
            capabilities: conservative_openai_capabilities(),
        }],
    }
}

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
