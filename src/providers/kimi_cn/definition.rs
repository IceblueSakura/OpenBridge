//! Static Moonshot Kimi China Provider contract and OpenAI-compatible Chat profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, ImageDetailPolicy, ImageInputCapabilities,
        ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonSchemaSupport, ProviderChatCompletionsCapabilities, ReasoningOutput,
        RemoteImageInputLimits, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

/// Image surface confirmed for the Kimi China Chat family.
///
/// One PNG data-URL image is proven upstream on Kimi K3 (2026-08-10); JPEG is
/// declared by OpenAI-compatible endpoint convention, no other media type was exercised.
const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    4,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: RemoteImageInputLimits::new(8_192),
        data: InlineImageInputProfile::new(
            &[ImageMediaType::Jpeg, ImageMediaType::Png],
            InlineImageInputLimits::new(
                20 * 1024 * 1024,
                15 * 1024 * 1024,
                20 * 1024 * 1024,
                15 * 1024 * 1024,
            ),
        ),
    },
    ImageDetailPolicy::OmittedOnly { default: None },
);

/// Conservative Kimi China Chat-only operation surface used by the registered Kimi K3 Target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: None,
            file_input: false,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: true,
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
