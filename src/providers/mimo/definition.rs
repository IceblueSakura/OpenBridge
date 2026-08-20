//! Static Xiaomi MiMo Provider contract and dual-protocol OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        FunctionToolCapabilities, ProviderChatCompletionsCapabilities,
        ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
        ResponseInclude, StructuredOutputProfile, ToolChoiceMode,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
        take_chat_reasoning_switch,
    },
};

use super::media::{AUDIO_CEILING, IMAGE_INPUT};

/// Confirmed MiMo Chat and Responses operation surface.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: &[ToolChoiceMode::Auto],
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ChatMediaProfile::new(Some(IMAGE_INPUT), Some(AUDIO_CEILING), None),
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
        "/v1/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: &[ToolChoiceMode::Auto],
                parallel_calls: true,
                strict_schema: true,
            }),
            media: crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
            structured_outputs: Some(StructuredOutputProfile::JsonObject),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: true,
            context_management: false,
            include: &[ResponseInclude::ReasoningEncryptedContent],
            moderation: false,
            logprobs: false,
        },
    )),
    None,
);

/// Dual-protocol OpenAI-compatible wire profile used by MiMo.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::MiMo,
    API_SURFACE,
    "/v1/models",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body);

/// Single static descriptor for the MiMo contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves the dedicated hook boundary for future MiMo ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Converts each admitted Chat level to MiMo's documented `thinking.type` switch.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Preserve requests without an explicit Chat level.
    let Some(enabled) = take_chat_reasoning_switch(protocol, document)? else {
        return Ok(());
    };

    // Write the fixed Provider extension after removing the standard downstream field.
    document.insert(
        "thinking".to_owned(),
        serde_json::json!({
            "type": if enabled { "enabled" } else { "disabled" }
        }),
    );
    Ok(())
}
