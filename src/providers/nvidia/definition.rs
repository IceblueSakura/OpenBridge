//! Static NVIDIA API Catalog Provider contract and OpenAI-compatible Chat profile.

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

/// Basic NVIDIA Chat-only operation surface confirmed independently of any model-specific target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
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
    )),
    None,
    None,
);

/// OpenAI-compatible Chat wire profile used by the NVIDIA API Catalog endpoint.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Nvidia,
    API_SURFACE,
    "/models",
    transform_request_headers,
);

/// Single static descriptor for the NVIDIA contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves a dedicated boundary for future NVIDIA ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
