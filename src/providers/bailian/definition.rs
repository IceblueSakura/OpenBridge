//! Static Alibaba Cloud Model Studio Provider contract and OpenAI-compatible Chat/Embeddings profile.

use http::HeaderMap;

use crate::{
    core::{
        ApiCapabilities, ChatCompletionsCapabilities, EmbeddingDimensionDomain, EmbeddingEncoding,
        EmbeddingInputForm, EmbeddingsCapabilities, ReasoningOutput, ResponsesCapabilities,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderContract, ProviderDefinition,
        ProviderKind, SafeHeaders,
    },
    providers::openai_compatible::{OpenAiCompatibleAdapter, take_chat_reasoning_switch},
};

const EMBEDDING_INPUT_FORMS: &[EmbeddingInputForm] =
    &[EmbeddingInputForm::String, EmbeddingInputForm::StringArray];
const EMBEDDING_ENCODINGS: &[EmbeddingEncoding] = &[EmbeddingEncoding::Float];
const EMBEDDING_DIMENSIONS: &[u32] = &[64, 128, 256, 512, 768, 1_024, 2_560];
const EMBEDDING_PARAMETERS: &[&str] = &["dimensions", "encoding_format"];

/// Bounded Model Studio Chat, Responses, and Embeddings ceilings confirmed independently of any model-specific target.
pub static CONTRACT: ProviderContract = ProviderContract::new(
    ProviderKind::Bailian,
    ApiCapabilities {
        chat_completions: ChatCompletionsCapabilities {
            enabled: true,
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
        responses: ResponsesCapabilities {
            enabled: true,
            streaming: true,
            function_tools: None,
            image_input: None,
            structured_outputs: None,
            store: false,
            previous_response_id: false,
            background: false,
            reasoning_output: ReasoningOutput::Summary,
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
            default_dimensions: 1_024,
            allowed_dimensions: Some(EmbeddingDimensionDomain::Values {
                values: EMBEDDING_DIMENSIONS,
            }),
            max_inputs: 20,
            max_tokens_per_input: Some(128_000),
            max_total_tokens: None,
            locally_counted_input_forms: &[],
            supported_parameters: EMBEDDING_PARAMETERS,
        },
    },
    &[CredentialKind::ApiKey],
);

/// OpenAI-compatible generation and Embeddings wire profile used by the Model Studio Beijing endpoint.
static ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Bailian,
    &CONTRACT,
    Some("/chat/completions"),
    Some("/responses"),
    Some("/embeddings"),
    "/models",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body);

/// Single static descriptor for the Model Studio contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition =
    ProviderDefinition::new(&CONTRACT, ProviderAdapter::from_openai_compatible(ADAPTER));

/// Preserves a dedicated boundary for future Model Studio ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Converts each admitted Qwen3.7 Chat level to Model Studio's official thinking switch.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Preserve model-specific effort semantics for non-Qwen3.7 targets sharing this adapter.
    let qwen3_7 = matches!(
        document.get("model").and_then(serde_json::Value::as_str),
        Some("qwen3.7-max" | "qwen3.7-plus")
    );
    if !qwen3_7 {
        return Ok(());
    }

    // Replace the downstream Chat level with the official boolean extension.
    if let Some(enabled) = take_chat_reasoning_switch(protocol, document)? {
        document.insert(
            "enable_thinking".to_owned(),
            serde_json::Value::Bool(enabled),
        );
    }
    Ok(())
}
