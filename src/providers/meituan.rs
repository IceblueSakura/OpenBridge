//! 美团 LongCat Provider 的编译期定义。
//!
//! LongCat-2.0 通过 LongCat OpenAI-compatible 端点原生接收 Chat Completions 和
//! Responses 请求。两种协议均保持原始 JSON/SSE wire 语义；本 adapter 只固定相对路径、
//! 注入上游模型 id 与构造 Bearer 认证，不执行协议桥接。

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
    models::longcat,
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

/// 基于直连验证及 OpenRouter 模型目录的 LongCat OpenAI-compatible 能力上界。
pub(crate) static DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::Meituan,
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
    },
    &["longcat-openai"],
    &[CredentialKind::ApiKey],
);

/// LongCat 的 OpenAI-compatible adapter。
#[derive(Clone, Copy)]
pub struct MeituanAdapter;

impl MeituanAdapter {
    pub(crate) fn encode_list_models_request(self) -> UpstreamRequestParts {
        UpstreamRequestParts::new(Method::GET, Uri::from_static("/v1/models"), Bytes::new())
    }
}

impl RequestAdapter for MeituanAdapter {
    fn encode_request(
        &self,
        request: &ValidatedRequest,
        upstream_model: &str,
    ) -> Result<UpstreamRequestParts, ProviderFailure> {
        let relative_uri = match request.protocol() {
            Protocol::ChatCompletions => Uri::from_static("/openai/v1/chat/completions"),
            Protocol::Responses => Uri::from_static("/openai/v1/responses"),
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

impl HeaderAdapter for MeituanAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }
}

impl AuthAdapter for MeituanAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure> {
        if credential.provider() != ProviderKind::Meituan {
            return Err(ProviderFailure::CredentialProviderMismatch);
        }
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);
        Ok(headers)
    }
}

impl ResponseAdapter for MeituanAdapter {
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

impl ErrorAdapter for MeituanAdapter {
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

impl CapabilityAdapter for MeituanAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure> {
        if requested.is_subset_of(*DESCRIPTOR.capabilities()) {
            Ok(())
        } else {
            Err(ProviderFailure::UnsupportedCapabilities)
        }
    }
}

pub(crate) struct MeituanDefinition {
    pub(crate) upstream_targets: Vec<UpstreamTargetDefinition>,
}

/// 构造 LongCat-2.0 的代码注册定义。
pub(crate) fn definition() -> MeituanDefinition {
    MeituanDefinition {
        upstream_targets: vec![UpstreamTargetDefinition {
            id: "meituan-longcat-2".to_owned(),
            provider: ProviderKind::Meituan,
            real_model: longcat::MODEL_ID.to_owned(),
            base_url: "https://api.longcat.chat".to_owned(),
            credential: CredentialDefinition {
                id: "meituan-longcat-primary".to_owned(),
                kind: CredentialKind::ApiKey,
                environment_variable: "LONGCAT_API_KEY".to_owned(),
            },
            quota_scope: None,
            fault_domain: None,
            request_timeout: Duration::from_secs(120),
            enabled: true,
            offerings: native_offerings(
                "LongCat-2.0",
                "longcat-openai",
                *DESCRIPTOR.capabilities(),
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
