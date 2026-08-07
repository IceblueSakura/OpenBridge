//! Static OpenAI Provider contract and OpenAI-compatible wire profile.

use http::{HeaderMap, header::USER_AGENT};

use crate::{
    core::{
        ApiCapabilities, ChatCompletionsCapabilities, EmbeddingDimensionDomain, EmbeddingEncoding,
        EmbeddingInputForm, EmbeddingsCapabilities, ImageDetail, ImageInputCapabilities,
        ImageInputSource, ImageMediaType, ReasoningOutput, ResponsesCapabilities,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
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
const CHAT_IMAGE_SOURCES: &[ImageInputSource] =
    &[ImageInputSource::RemoteUrl, ImageInputSource::DataUrl];
const RESPONSES_IMAGE_SOURCES: &[ImageInputSource] = &[
    ImageInputSource::RemoteUrl,
    ImageInputSource::DataUrl,
    ImageInputSource::FileId,
];
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
const CHAT_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities {
    sources: CHAT_IMAGE_SOURCES,
    media_types: IMAGE_MEDIA_TYPES,
    detail_default: Some(ImageDetail::Auto),
    allowed_details: IMAGE_DETAILS,
    max_parts: 500,
    max_url_length: 8_192,
    max_inline_encoded_bytes: 20 * 1024 * 1024,
    max_inline_decoded_bytes: 15 * 1024 * 1024,
    max_total_inline_encoded_bytes: 50 * 1024 * 1024,
    max_total_inline_decoded_bytes: 38 * 1024 * 1024,
};
const RESPONSES_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities {
    sources: RESPONSES_IMAGE_SOURCES,
    ..CHAT_IMAGE_INPUT
};

/// Static OpenAI adapter capabilities and permitted endpoint/credential scope.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::OpenAi,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: Some(CHAT_IMAGE_INPUT),
            structured_outputs: true,
            store: true,
            reasoning_output: ReasoningOutput::Unknown,
            custom_tool_calling: false,
            audio_input: false,
            file_input: false,
            audio_output: false,
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
            function_calling: true,
            parallel_tool_calls: true,
            image_input: Some(RESPONSES_IMAGE_INPUT),
            structured_outputs: true,
            store: true,
            previous_response_id: true,
            background: false,
            reasoning_output: ReasoningOutput::Unknown,
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
        embeddings: EmbeddingsCapabilities {
            enabled: true,
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
    },
    &[CredentialKind::ApiKey],
);

/// Static OpenAI-compatible wire profile used by OpenAI.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::OpenAi,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    Some("/v1/embeddings"),
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the OpenAI contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

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
