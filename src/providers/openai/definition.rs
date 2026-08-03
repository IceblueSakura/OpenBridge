//! Static OpenAI Provider contract and OpenAI-compatible wire profile.

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{ApiCapabilities, ChatCompletionsCapabilities, ReasoningOutput, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

/// Static OpenAI adapter capabilities and permitted endpoint/credential scope.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::OpenAi,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: true,
            structured_outputs: true,
            store: true,
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
            store: true,
            previous_response_id: true,
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
    &["public-api"],
    &[CredentialKind::ApiKey],
);

/// Static OpenAI-compatible wire profile used by OpenAI.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenAi,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the OpenAI contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Applies the ordinary-header transform currently required by OpenAI.
fn transform_request_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }
    Ok(())
}
