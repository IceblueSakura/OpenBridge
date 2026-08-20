//! Static OpenAI Provider contract and OpenAI-compatible wire profile.

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
        EmbeddingsCapabilities, FunctionToolCapabilities, ImageDetail, ImageDetailPolicy,
        ImageDetailProfile, ImageInputCapabilities, ImageMediaType, ImageSourceCapabilities,
        InlineImageInputLimits, InlineImageInputProfile, JsonSchemaSupport,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
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

const EMBEDDING_INPUT_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::String,
    EmbeddingInputForm::StringArray,
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const EMBEDDING_ENCODINGS: &[EmbeddingEncoding] =
    &[EmbeddingEncoding::Float, EmbeddingEncoding::Base64];
const LOCALLY_COUNTED_EMBEDDING_FORMS: &[EmbeddingInputForm] = &[
    EmbeddingInputForm::TokenArray,
    EmbeddingInputForm::TokenArrayArray,
];
const EMBEDDING_PARAMETERS: &[&str] = &["dimensions", "encoding_format", "user"];
const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
];
const IMAGE_DETAILS: &[ImageDetail] = &[
    ImageDetail::Auto,
    ImageDetail::Low,
    ImageDetail::High,
    ImageDetail::Original,
];
const IMAGE_REMOTE_LIMITS: RemoteImageInputLimits = RemoteImageInputLimits::new(8_192);
const IMAGE_INLINE_LIMITS: InlineImageInputLimits = InlineImageInputLimits::new(
    20 * 1024 * 1024,
    15 * 1024 * 1024,
    50 * 1024 * 1024,
    38 * 1024 * 1024,
);
const IMAGE_INLINE_PROFILE: InlineImageInputProfile =
    InlineImageInputProfile::new(IMAGE_MEDIA_TYPES, IMAGE_INLINE_LIMITS);
const IMAGE_DETAIL_PROFILE: ImageDetailProfile =
    ImageDetailProfile::new(Some(ImageDetail::Auto), IMAGE_DETAILS);
const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    500,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: IMAGE_REMOTE_LIMITS,
        data: IMAGE_INLINE_PROFILE,
    },
    ImageDetailPolicy::Explicit(IMAGE_DETAIL_PROFILE),
);

/// Single OpenAI operation surface shared by the Provider contract and wire adapter.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/chat/completions",
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
            store: true,
            reasoning_output: ReasoningOutput::Unknown,
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
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            terminal_usage: true,
            function_tools: Some(FunctionToolCapabilities {
                choice_modes: ALL_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: true,
            }),
            image_input: Some(IMAGE_INPUT),
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            state: ProviderResponsesStateCeiling::StorageAndContinuation,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            hosted_tools: &[],
            file_input: None,
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: false,
            context_management: false,
            include: &[ResponseInclude::ReasoningEncryptedContent],
            moderation: false,
            logprobs: false,
        },
    )),
    Some(OpenAiCompatibleEndpoint::new(
        "/v1/embeddings",
        EmbeddingsCapabilities {
            input_forms: EMBEDDING_INPUT_FORMS,
            default_encoding: EmbeddingEncoding::Float,
            allowed_encodings: Some(EMBEDDING_ENCODINGS),
            default_dimensions: 1,
            allowed_dimensions: Some(EmbeddingDimensionDomain::Range {
                minimum: 1,
                maximum: u32::MAX,
            }),
            max_inputs: u32::MAX,
            max_tokens_per_input: None,
            max_total_tokens: None,
            locally_counted_input_forms: LOCALLY_COUNTED_EMBEDDING_FORMS,
            supported_parameters: EMBEDDING_PARAMETERS,
        },
    )),
);

/// Static OpenAI-compatible wire profile used by OpenAI.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenAi,
    API_SURFACE,
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the OpenAI contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Applies the ordinary-header transform currently required by OpenAI.
fn transform_request_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }
    Ok(())
}
