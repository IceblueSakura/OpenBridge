//! Static OpenRouter Provider contract and stateless OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, ImageDetailPolicy, ImageInputCapabilities,
        ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonSchemaSupport, ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, RemoteImageInputLimits, ResponseInclude,
        StructuredOutputProfile,
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

/// Image surface confirmed for the OpenRouter Chat family.
///
/// One PNG data-URL image is proven upstream on MiniMax M3 (2026-08-10); JPEG is
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
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(STRUCTURED_OUTPUTS),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: None,
            file_input: None,
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
            image_input: None,
            structured_outputs: Some(STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: None,
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
