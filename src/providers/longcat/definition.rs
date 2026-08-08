//! Static LongCat Provider contract and OpenAI-compatible wire profile.

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, ApiCapabilities, ChatCompletionsCapabilities,
        FunctionToolCapabilities, ReasoningOutput, ResponsesCapabilities,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::{OpenAiCompatibleAdapter, take_chat_reasoning_switch},
};

/// LongCat OpenAI-compatible capability ceiling based on direct checks and the OpenRouter catalog.
pub(crate) static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::LongCat,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: false,
            }),
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
            enabled: true,
            streaming: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: false,
            }),
            image_input: None,
            structured_outputs: None,
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
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

/// Static OpenAI-compatible wire profile used by LongCat.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::LongCat,
    &CONTRACT,
    Some("/openai/v1/chat/completions"),
    Some("/openai/v1/responses"),
    None,
    "/openai/v1/models",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body)
.with_openai_data_type_responses_terminal();

/// Single static descriptor for the LongCat contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Applies the ordinary-header transform currently required by LongCat.
fn transform_request_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }
    Ok(())
}

/// Converts each LongCat model level to its documented Chat `thinking.type` switch.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Preserve requests without an explicit LongCat Chat level.
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
