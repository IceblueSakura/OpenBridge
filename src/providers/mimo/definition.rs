//! Static Xiaomi MiMo Provider contract and dual-protocol OpenAI-compatible profile.

use http::HeaderMap;

use crate::{
    core::{
        ApiCapabilities, ChatCompletionsCapabilities, ImageInputCapabilities, ImageInputSource,
        ImageMediaType, ReasoningOutput, ResponsesCapabilities,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::OpenAiCompatibleAdapter,
};

const IMAGE_SOURCES: &[ImageInputSource] =
    &[ImageInputSource::RemoteUrl, ImageInputSource::DataUrl];
const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
    ImageMediaType::Bmp,
];
const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities {
    sources: IMAGE_SOURCES,
    media_types: IMAGE_MEDIA_TYPES,
    detail_default: None,
    allowed_details: &[],
    max_parts: 64,
    max_url_length: 8_192,
    max_inline_encoded_bytes: 50 * 1024 * 1024,
    max_inline_decoded_bytes: 38 * 1024 * 1024,
    max_total_inline_encoded_bytes: 50 * 1024 * 1024,
    max_total_inline_decoded_bytes: 38 * 1024 * 1024,
};

/// Confirmed MiMo capability ceiling for Chat Completions and Responses; readable reasoning output
/// is not yet confirmed on the wire.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::MiMo,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
            streaming: true,
            function_calling: true,
            parallel_tool_calls: true,
            image_input: Some(IMAGE_INPUT),
            structured_outputs: true,
            store: false,
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
            image_input: Some(IMAGE_INPUT),
            structured_outputs: true,
            store: false,
            previous_response_id: false,
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
        embeddings: crate::core::EmbeddingsCapabilities::disabled(),
    },
    &[CredentialKind::ApiKey],
);

/// Dual-protocol OpenAI-compatible wire profile used by MiMo.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::MiMo,
    &CONTRACT,
    Some("/v1/chat/completions"),
    Some("/v1/responses"),
    None,
    "/v1/models",
    transform_request_headers,
);

/// Single static descriptor for the MiMo contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves the dedicated hook boundary for future MiMo ordinary-header transforms.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
