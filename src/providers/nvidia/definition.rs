//! Static NVIDIA API Catalog Provider contract and OpenAI-compatible Chat profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
        EmbeddingsCapabilities, FunctionToolCapabilities, ImageDetailPolicy,
        ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits,
        InlineImageInputProfile, JsonSchemaSupport, ProviderChatCompletionsCapabilities,
        ReasoningOutput, RemoteImageInputLimits, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

const EMBEDDING_INPUT_FORMS: &[EmbeddingInputForm] =
    &[EmbeddingInputForm::String, EmbeddingInputForm::StringArray];
const EMBEDDING_ENCODINGS: &[EmbeddingEncoding] = &[EmbeddingEncoding::Float];
const EMBEDDING_DIMENSIONS: &[u32] = &[2_048];
const EMBEDDING_PARAMETERS: &[&str] = &["dimensions", "encoding_format"];

/// Image surface confirmed for the NVIDIA API Catalog generation family.
///
/// One PNG data-URL image is proven upstream (2026-08-10); JPEG is declared by
/// OpenAI-compatible endpoint convention, no other media type was exercised.
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

/// Basic NVIDIA Chat-only operation surface confirmed independently of any model-specific target.
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
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
            audio: None,
            file_input: None,
            predicted_outputs: false,
            web_search: false,
            prompt_cache_key: false,
            moderation: false,
            logprobs: false,
            multiple_choices: false,
        },
    )),
    None,
    Some(OpenAiCompatibleEndpoint::new(
        "/embeddings",
        EmbeddingsCapabilities {
            input_forms: EMBEDDING_INPUT_FORMS,
            default_encoding: EmbeddingEncoding::Float,
            allowed_encodings: Some(EMBEDDING_ENCODINGS),
            default_dimensions: 2_048,
            allowed_dimensions: Some(EmbeddingDimensionDomain::Values {
                values: EMBEDDING_DIMENSIONS,
            }),
            max_inputs: 20,
            max_tokens_per_input: None,
            max_total_tokens: None,
            locally_counted_input_forms: &[],
            supported_parameters: EMBEDDING_PARAMETERS,
        },
    )),
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
