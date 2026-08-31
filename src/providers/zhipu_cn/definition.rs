//! Static Zhipu AI China Provider contract and OpenAI-compatible generation profile.
//!
//! Chat follows Zhipu's OpenAI compatibility guide. GLM-5.3 Responses follows the model page
//! reverified on 2026-08-31 and remains narrowed to the text-only protocol subset until broader
//! endpoint-specific probes exist.

use http::HeaderMap;

use crate::{
    core::{
        FunctionToolCapabilities, ProviderChatCompletionsCapabilities,
        ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
        StructuredOutputProfile, ToolChoiceMode,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

use super::media::IMAGE_INPUT;

/// Provider ceiling shared by the registered GLM Chat targets and the GLM-5.3 Responses target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/api/paas/v4/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: &[ToolChoiceMode::Auto],
                parallel_calls: false,
                strict_schema: false,
            }),
            media: crate::core::ChatMediaProfile::new(Some(IMAGE_INPUT), None, None),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
    )),
    Some(OpenAiCompatibleEndpoint::new(
        "/api/v1/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: None,
            media: crate::core::ResponsesMediaProfile::new(None, None),
            structured_outputs: None,
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: false,
            context_management: false,
            include: &[],
            moderation: false,
            logprobs: false,
        },
    )),
    None,
);

/// OpenAI-compatible generation wire profile used by the fixed Zhipu China paths.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::ZhipuCn,
    API_SURFACE,
    "/api/paas/v4/models",
    transform_request_headers,
);

/// Single static descriptor for the Zhipu China contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves a dedicated boundary for future Zhipu China ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
