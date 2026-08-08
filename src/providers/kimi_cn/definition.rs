//! Static Moonshot Kimi China Provider contract and OpenAI-compatible Chat profile.

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// Conservative Kimi China Chat Completions ceiling used by the registered Kimi K3 Target.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::KimiCn,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_tools: None,
            image_input: None,
            structured_outputs: None,
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: None,
            file_input: false,
            predicted_outputs: false,
            web_search: false,
            prompt_caching: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
        responses: ResponsesCapabilities {
            enabled: false,
            streaming: false,
            function_tools: None,
            image_input: None,
            structured_outputs: None,
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
        embeddings: crate::core::EmbeddingsCapabilities::disabled(),
    },
    &[CredentialKind::ApiKey],
);

/// OpenAI-compatible Chat wire profile used by the Kimi China endpoint.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::KimiCn,
    &CONTRACT,
    Some("/v1/chat/completions"),
    None,
    None,
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the Kimi China contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves a dedicated boundary for future Kimi China ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
