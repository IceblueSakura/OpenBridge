//! DeepSeek Provider 的静态契约与 Chat-only OpenAI-compatible profile。

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities},
    provider::{AdapterError, CredentialKind, ProviderContract, ProviderKind, SafeHeaders},
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// DeepSeek Chat Completions 能力上界。
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
        },
    },
    &["deepseek-openai"],
    &[CredentialKind::ApiKey],
);

/// DeepSeek 使用的 Chat-only OpenAI-compatible wire profile。
pub(crate) static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::DeepSeek,
    &CONTRACT,
    Some("/chat/completions"),
    None,
    "/models",
    transform_request_headers,
);

/// 保留 DeepSeek 后续普通请求头转换的独立 hook 边界。
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
