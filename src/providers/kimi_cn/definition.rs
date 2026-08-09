//! Static Moonshot Kimi China Provider contract and OpenAI-compatible Chat profile.

use http::HeaderMap;

use crate::{
    core::{ProviderChatCompletionsCapabilities, ReasoningOutput},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

/// Conservative Kimi China Chat-only operation surface used by the registered Kimi K3 Target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/chat/completions",
        ProviderChatCompletionsCapabilities {
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
    )),
    None,
    None,
);

/// OpenAI-compatible Chat wire profile used by the Kimi China endpoint.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::KimiCn,
    API_SURFACE,
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the Kimi China contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves a dedicated boundary for future Kimi China ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
