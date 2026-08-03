//! Xiaomi MiMo Provider 的静态契约与双协议 OpenAI-compatible profile。

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// MiMo Chat Completions 与 Responses 的已确认能力上界；reasoning 输出仍未确认可读 wire。
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::MiMo,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio_input: false,
            file_input: false,
            audio_output: false,
            predicted_outputs: false,
            web_search: false,
            prompt_caching: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
            conversation: false,
            prompt_templates: false,
            prompt_caching: false,
            context_management: false,
            include: &[],
            moderation: false,
            logprobs: false,
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
