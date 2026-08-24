//! Static OpenRouter Provider contract and stateless OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, JsonSchemaSupport,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, ResponseInclude, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

const STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);
const RESPONSES_INCLUDES: &[ResponseInclude] = &[ResponseInclude::ReasoningEncryptedContent];

use super::media::IMAGE_INPUT;

/// Conservative stateless Chat and Responses operation surface for OpenRouter.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ChatMediaProfile::new(Some(IMAGE_INPUT), None, None),
            structured_outputs: Some(STRUCTURED_OUTPUTS),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: true,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
    )),
    Some(OpenAiCompatibleEndpoint::new(
        "/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
            structured_outputs: Some(STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: true,
            context_management: false,
            include: RESPONSES_INCLUDES,
            moderation: false,
            logprobs: false,
        },
    )),
    None,
);

/// Stateless Chat/Responses OpenAI-compatible wire profile used by OpenRouter.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenRouter,
    API_SURFACE,
    "/models",
    transform_request_headers,
)
.with_openai_data_type_responses_terminal();

/// Single static descriptor for the OpenRouter contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Keeps optional OpenRouter attribution and routing headers under explicit compile-time policy.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
