//! DeepSeek Provider 的静态契约与 Chat-only OpenAI-compatible profile。

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// DeepSeek Chat Completions 能力上界；Chat reasoning 以 `reasoning_content` 明文输出。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::DeepSeek,
    ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
        },
        responses: ResponsesCapabilities {
            enabled: false,
            streaming: false,
            function_calling: false,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::Unsupported,
        },
    },
    &["deepseek-openai"],
    &[CredentialKind::ApiKey],
);

/// DeepSeek 使用的 Chat-only OpenAI-compatible wire profile。
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::DeepSeek,
    &CONTRACT,
    Some("/chat/completions"),
    None,
    "/models",
    transform_request_headers,
);

/// DeepSeek contract 与 adapter 的唯一静态描述符。
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// 保留 DeepSeek 后续普通请求头转换的独立 hook 边界。
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
