//! Static Alibaba Cloud Model Studio Provider contract and OpenAI-compatible Chat/Embeddings profile.

use http::HeaderMap;

use crate::{
    core::{
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        SafeHeaders,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
        take_chat_reasoning_switch,
    },
};

const EMBEDDING_INPUT_FORMS: &[EmbeddingInputForm] =
    &[EmbeddingInputForm::String, EmbeddingInputForm::StringArray];
const EMBEDDING_ENCODINGS: &[EmbeddingEncoding] = &[EmbeddingEncoding::Float];
const EMBEDDING_DIMENSIONS: &[u32] = &[256, 512, 768, 1_024, 1_536, 2_048, 2_560];
const EMBEDDING_PARAMETERS: &[&str] = &["dimensions", "encoding_format"];
const CHAT_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;

/// Bounded Model Studio operation surface confirmed independently of any model-specific target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            function_tools: None,
            image_input: None,
            structured_outputs: Some(CHAT_STRUCTURED_OUTPUTS),
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
    Some(OpenAiCompatibleEndpoint::new(
        "/responses",
        ProviderResponsesCapabilities {
            streaming: true,
            function_tools: None,
            image_input: None,
            structured_outputs: None,
            state: ProviderResponsesStateCeiling::Stateless,
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
    )),
    Some(OpenAiCompatibleEndpoint::new(
        "/embeddings",
        EmbeddingsCapabilities {
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
    )),
);

/// OpenAI-compatible generation and Embeddings wire profile used by the Model Studio Beijing endpoint.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Bailian,
    API_SURFACE,
    "/models",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body);

/// Single static descriptor for the Model Studio contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::ApiKey],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Preserves a dedicated boundary for future Model Studio ordinary-header requirements.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Converts each admitted hybrid Qwen Chat level to Model Studio's official thinking switch.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Preserve model-specific effort semantics for targets without the Qwen boolean wire contract.
    let qwen_hybrid_thinking = matches!(
        document.get("model").and_then(serde_json::Value::as_str),
        Some("qwen3.8-max" | "qwen3.7-max" | "qwen3.7-plus")
    );
    if !qwen_hybrid_thinking {
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
