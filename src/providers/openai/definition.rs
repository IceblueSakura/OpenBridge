//! OpenAI Provider 的静态契约与 OpenAI-compatible wire profile。

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
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

/// OpenAI 使用的静态 OpenAI-compatible wire profile。
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenAi,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    "/v1/models",
    transform_request_headers,
);

/// OpenAI contract 与 adapter 的唯一静态描述符。
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// 应用 OpenAI 当前要求的普通请求头转换。
fn transform_request_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }
    Ok(())
}
