//! LongCat Provider 的静态契约与 OpenAI-compatible wire profile。

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{AdapterError, CredentialKind, ProviderContract, ProviderKind, SafeHeaders},
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
pub(crate) static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::LongCat,
    &CONTRACT,
    Some("/openai/v1/chat/completions"),
    Some("/openai/v1/responses"),
    "/v1/models",
    transform_request_headers,
);

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
