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

use super::media::IMAGE_INPUT;

const CHAT_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const RESPONSES_STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::NonStrictOnly);

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
                strict_schema: false,
            }),
            media: crate::core::ChatMediaProfile::new(Some(IMAGE_INPUT), None, None),
            structured_outputs: Some(CHAT_STRUCTURED_OUTPUTS),
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
        "/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: false,
                strict_schema: false,
            }),
            media: crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
            structured_outputs: Some(RESPONSES_STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
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
    use crate::core::{
        JsonSchemaSupport, OperationKind, ProviderOperationCapabilities, StructuredOutputProfile,
    };

    use super::*;

    #[test]
    fn ordinary_endpoint_does_not_advertise_beta_function_strictness() {
        for operation in [OperationKind::ChatCompletions, OperationKind::Responses] {
            let capabilities = DEFINITION
                .contract()
                .capabilities()
                .operation(operation)
                .unwrap();
            let tools = match capabilities {
                ProviderOperationCapabilities::ChatCompletions(capabilities) => {
                    capabilities.function_tools
                }
                ProviderOperationCapabilities::Responses(capabilities) => {
                    capabilities.function_tools
                }
                _ => unreachable!("DeepSeek generation definition has only Chat and Responses"),
            }
            .unwrap();
            assert!(!tools.strict_schema);
        }

        let responses = DEFINITION
            .contract()
            .capabilities()
            .operation(OperationKind::Responses)
            .and_then(ProviderOperationCapabilities::responses)
            .unwrap();
        assert_eq!(
            responses.structured_outputs,
            Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::NonStrictOnly
            ))
        );
    }
}
