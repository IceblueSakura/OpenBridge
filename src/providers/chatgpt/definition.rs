//! Static ChatGPT Provider contract and Codex Responses wire profile.
//!
//! This profile permits only OAuth bearer credentials and fixed Codex backend paths. It does not
//! expose Chat Completions, Embeddings, WebSocket, or a generic OpenAI-compatible endpoint.

use http::HeaderMap;

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// Static ChatGPT Codex adapter capabilities and permitted endpoint/credential scope.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::ChatGpt,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: false,
            streaming: false,
            function_calling: false,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            reasoning_output: ReasoningOutput::Unsupported,
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
            function_calling: false,
            parallel_tool_calls: false,
            image_input: false,
            structured_outputs: false,
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::Unsupported,
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
    &[CredentialKind::OAuth2BearerAccessToken],
);

/// Responses-only wire profile used by the fixed ChatGPT Codex backend.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::ChatGpt,
    &CONTRACT,
    None,
    Some("/responses"),
    None,
    "/models",
    transform_request_headers,
)
.with_openai_data_type_responses_terminal();

/// Single static descriptor for the ChatGPT contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves the dedicated hook boundary for future ChatGPT ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
