//! OpenRouter Provider 的静态契约与无状态 OpenAI-compatible profile。

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// OpenRouter Chat Completions 的保守能力上界。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::OpenRouter,
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
    &["openrouter-chat", "openrouter-responses"],
    &[CredentialKind::ApiKey],
);

/// OpenRouter 使用的无状态 Chat/Responses OpenAI-compatible wire profile。
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenRouter,
    &CONTRACT,
    Some("/chat/completions"),
    Some("/responses"),
    "/models",
    transform_request_headers,
)
.with_openai_data_type_responses_terminal();

/// OpenRouter contract 与 adapter 的唯一静态描述符。
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// 保持 OpenRouter 可选归因和路由 header 由编译期策略显式拥有。
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
