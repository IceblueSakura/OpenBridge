//! Static DeepSeek Provider contract and model-gated OpenAI-compatible generation profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, JsonSchemaSupport,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

const CHAT_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const RESPONSES_STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);

/// Single DeepSeek operation surface shared by the Provider contract and wire adapter.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: true,
            }),
            image_input: None,
            structured_outputs: Some(CHAT_STRUCTURED_OUTPUTS),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: None,
            file_input: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: false,
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
                parallel_calls: false,
                strict_schema: true,
            }),
            image_input: None,
            structured_outputs: Some(RESPONSES_STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: false,
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

/// OpenAI-compatible Chat and Responses wire profile used by registered DeepSeek models.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::DeepSeek,
    API_SURFACE,
    "/models",
    transform_request_headers,
);

/// Single static descriptor for the DeepSeek contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves the dedicated hook boundary for future DeepSeek ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{JsonSchemaSupport, StructuredOutputMode};

    #[test]
    fn direct_generation_contract_matches_confirmed_tool_and_request_option_boundaries() {
        let capabilities = DEFINITION.contract().capabilities();
        let chat = capabilities
            .chat_completions
            .expect("DeepSeek contract must expose Chat Completions");
        let responses = capabilities
            .responses
            .expect("DeepSeek contract must expose Responses");

        // Both APIs execute strict function schemas and every standard function choice mode.
        for tools in [chat.function_tools, responses.function_tools] {
            let tools = tools.expect("DeepSeek generation APIs must expose function tools");
            assert_eq!(tools.choice_modes, ALL_TOOL_CHOICE_MODES);
            assert!(tools.strict_schema);
            assert!(!tools.parallel_calls);
        }

        // DeepSeek manages caching automatically and does not implement Responses include values.
        assert!(!chat.prompt_cache_key);
        assert!(!responses.prompt_cache_key);
        assert!(responses.include.is_empty());
    }

    #[test]
    fn chat_endpoint_keeps_json_object_only_structured_outputs() {
        let chat = DEFINITION
            .contract()
            .capabilities()
            .chat_completions
            .expect("DeepSeek contract must expose Chat Completions");
        let profile = chat
            .structured_outputs
            .expect("DeepSeek Chat must expose structured outputs");
        assert_eq!(profile, StructuredOutputProfile::JsonObject);
        assert_eq!(
            profile.modes(),
            &[StructuredOutputMode::JsonObject],
            "DeepSeek Chat rejects json_schema with HTTP 400"
        );
        assert!(!profile.supports_strict_schema());
    }

    #[test]
    fn responses_endpoint_exposes_strict_json_schema() {
        let responses = DEFINITION
            .contract()
            .capabilities()
            .responses
            .expect("DeepSeek contract must expose Responses");
        let profile = responses
            .structured_outputs
            .expect("DeepSeek Responses must expose structured outputs");
        assert_eq!(
            profile,
            StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported)
        );
        assert!(profile.supports(StructuredOutputMode::JsonObject));
        assert!(profile.supports(StructuredOutputMode::JsonSchema));
        assert!(profile.supports_strict_schema());
    }
}
