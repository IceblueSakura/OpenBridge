//! Static NVIDIA API Catalog Provider contract and OpenAI-compatible Chat profile.

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// Basic NVIDIA Chat Completions ceiling confirmed independently of any model-specific target.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::Nvidia,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_tools: None,
            image_input: None,
            structured_outputs: None,
            store: false,
            reasoning_output: ReasoningOutput::Unknown,
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

/// OpenAI-compatible Chat wire profile used by the NVIDIA API Catalog endpoint.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Nvidia,
    &CONTRACT,
    Some("/chat/completions"),
    None,
    None,
    "/models",
    transform_request_headers,
);

/// Single static descriptor for the NVIDIA contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves a dedicated boundary for future NVIDIA ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
