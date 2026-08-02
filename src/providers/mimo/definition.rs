//! Xiaomi MiMo Provider 的静态契约与双协议 OpenAI-compatible profile。

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// MiMo Chat Completions 与 Responses 的保守能力上界。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::MiMo,
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
    &["mimo-openai"],
    &[CredentialKind::ApiKey],
);

/// MiMo 使用的双协议 OpenAI-compatible wire profile。
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::MiMo,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    "/v1/models",
    transform_request_headers,
);

/// MiMo contract 与 adapter 的唯一静态描述符。
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// 保留 MiMo 后续普通请求头转换的独立 hook 边界。
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
