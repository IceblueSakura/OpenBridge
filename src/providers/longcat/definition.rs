//! LongCat Provider 的静态契约与 OpenAI-compatible wire profile。

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// 基于直连验证及 OpenRouter 模型目录的 LongCat OpenAI-compatible 能力上界。
pub(crate) static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::LongCat,
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
    },
    &["longcat-openai"],
    &[CredentialKind::ApiKey],
);

/// LongCat 使用的静态 OpenAI-compatible wire profile。
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::LongCat,
    &CONTRACT,
    Some("/openai/v1/chat/completions"),
    Some("/openai/v1/responses"),
    "/v1/models",
    transform_request_headers,
);

/// LongCat contract 与 adapter 的唯一静态描述符。
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// 应用 LongCat 当前要求的普通请求头转换。
fn transform_request_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }
    Ok(())
}
