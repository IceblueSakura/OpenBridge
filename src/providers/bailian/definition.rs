//! Static Alibaba Cloud Model Studio Provider contract and OpenAI-compatible Chat/Embeddings profile.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm,
        EmbeddingsCapabilities, FunctionToolCapabilities, JsonSchemaSupport,
        ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
        ProviderResponsesStateCeiling, ReasoningOutput, StructuredOutputProfile, ToolChoiceMode,
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
const CHAT_STRUCTURED_OUTPUTS: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);
const RESPONSES_STRUCTURED_OUTPUTS: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const RESPONSES_TOOL_CHOICE_MODES: &[ToolChoiceMode] =
    &[ToolChoiceMode::None, ToolChoiceMode::Auto];
const FUNCTION_TOOLS: FunctionToolCapabilities = FunctionToolCapabilities {
    choice_modes: ALL_TOOL_CHOICE_MODES,
    parallel_calls: true,
    strict_schema: false,
};

/// Bounded Model Studio operation surface confirmed independently of any model-specific target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FUNCTION_TOOLS),
            media: crate::core::ChatMediaProfile::new(None, None, None),
            structured_outputs: Some(CHAT_STRUCTURED_OUTPUTS),
            store: false,
            reasoning_output: ReasoningOutput::PlainText,
            custom_tool_calling: false,
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
                choice_modes: RESPONSES_TOOL_CHOICE_MODES,
                parallel_calls: true,
                strict_schema: false,
            }),
            media: crate::core::ResponsesMediaProfile::new(None, None),
            structured_outputs: Some(RESPONSES_STRUCTURED_OUTPUTS),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::Summary,
            custom_tool_calling: false,
            hosted_tools: &[],
            conversation: false,
            prompt_templates: false,
            prompt_cache_key: true,
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

/// Converts confirmed Chat off controls and Qwen reasoning levels to Model Studio switches.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Convert every admitted Qwen reasoning level to its confirmed boolean Chat switch.
    let qwen_boolean_thinking = matches!(
        document.get("model").and_then(serde_json::Value::as_str),
        Some("qwen3.8-max" | "qwen3.7-max" | "qwen3.7-plus" | "qwen3.6-27b")
    );
    if qwen_boolean_thinking && let Some(enabled) = take_chat_reasoning_switch(protocol, document)?
    {
        document.insert(
            "enable_thinking".to_owned(),
            serde_json::Value::Bool(enabled),
        );
    }

    // Convert only DeepSeek's off level while preserving its multi-level effort vocabulary.
    let bailian_deepseek = matches!(
        document.get("model").and_then(serde_json::Value::as_str),
        Some("deepseek-v4-pro" | "deepseek-v4-flash-0731")
    );
    if protocol == crate::core::ApiProtocol::ChatCompletions
        && bailian_deepseek
        && document
            .get("reasoning_effort")
            .and_then(serde_json::Value::as_str)
            == Some("none")
    {
        document.remove("reasoning_effort");
        document.insert("enable_thinking".to_owned(), serde_json::Value::Bool(false));
    }
    Ok(())
}
