//! Static Alibaba Cloud Model Studio contract for OpenAI-compatible Chat/Responses/Embeddings and
//! DashScope-native Images.

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, DashScopeImagesCapabilities, DashScopePromptExtendMode,
        EmbeddingDimensionDomain, EmbeddingEncoding, EmbeddingInputForm, EmbeddingsCapabilities,
        FunctionToolCapabilities, ImagesGenerationsCapabilities, ImagesResponseFormat,
        ImagesSizeDomain, JsonSchemaSupport, OperationKind, ProviderChatCompletionsCapabilities,
        ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
        StructuredOutputProfile, ToolChoiceMode,
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

use super::media::QWEN_IMAGE_INPUT;

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

/// DashScope-native image generation ceiling confirmed for the qwen-image 3.0 family.
const IMAGES_CAPABILITIES: ImagesGenerationsCapabilities = ImagesGenerationsCapabilities {
    default_outputs: 1,
    max_outputs: 6,
    allowed_sizes: Some(ImagesSizeDomain {
        minimum_side: 512,
        maximum_side: 2_048,
        minimum_area: 512 * 512,
        maximum_area: 2_048 * 2_048,
    }),
    default_response_format: ImagesResponseFormat::Url,
    allowed_response_formats: Some(&[ImagesResponseFormat::Url]),
    supported_parameters: &["n", "output_format", "response_format", "size", "user"],
    dashscope_extensions: Some(DashScopeImagesCapabilities {
        default_prompt_extend: true,
        prompt_extend_modes: &[
            DashScopePromptExtendMode::Direct,
            DashScopePromptExtendMode::Agent,
        ],
        default_prompt_extend_mode: DashScopePromptExtendMode::Direct,
        default_enable_thinking: true,
        negative_prompt: true,
        maximum_seed: 2_147_483_647,
        default_watermark: false,
    }),
};

/// Bounded Model Studio operation surface confirmed independently of any model-specific target.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    Some(OpenAiCompatibleEndpoint::new(
        "/chat/completions",
        ProviderChatCompletionsCapabilities {
            streaming: true,
            stream_usage: true,
            function_tools: Some(FUNCTION_TOOLS),
            media: crate::core::ChatMediaProfile::new(Some(QWEN_IMAGE_INPUT), None, None),
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
            media: crate::core::ResponsesMediaProfile::new(Some(QWEN_IMAGE_INPUT), None),
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
)
.with_images(Some(OpenAiCompatibleEndpoint::new(
    "/services/aigc/multimodal-generation/generation",
    IMAGES_CAPABILITIES,
)));

/// OpenAI-compatible generation and Embeddings wire profile used by the Model Studio Beijing endpoint.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Bailian,
    API_SURFACE,
    "/models",
    transform_request_headers,
)
.with_routed_request_header_hook(transform_routed_request_headers)
.with_request_body_hook(transform_request_body)
.with_images_request_body_hook(transform_images_request_body);

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

/// Applies fixed Model Studio queueing and model-scoped Responses session-cache policy.
///
/// Sources reverified on 2026-08-27:
/// <https://help.aliyun.com/zh/model-studio/rate-limiting-best-practices> and
/// <https://help.aliyun.com/zh/model-studio/use-context-cache>.
fn transform_routed_request_headers(
    operation: OperationKind,
    upstream_model: &str,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    // Queue burst-limited requests for at most 30 seconds before returning a Provider 429.
    upstream.insert(
        HeaderName::from_static("x-dashscope-wait-timeout"),
        HeaderValue::from_static("30"),
    )?;

    // Enable automatic Responses cache only for models explicitly listed by Model Studio.
    if operation == OperationKind::Responses
        && matches!(
            upstream_model,
            "qwen3.8-max" | "qwen3.7-max" | "qwen3.7-plus"
        )
    {
        upstream.insert(
            HeaderName::from_static("x-dashscope-session-cache"),
            HeaderValue::from_static("enable"),
        )?;
    }
    Ok(())
}

/// Converts one preflighted OpenAI Images request to the DashScope native multimodal-generation shape.
fn transform_images_request_body(
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Take the trusted non-blank prompt captured by strict request analysis.
    let prompt = document
        .remove("prompt")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(AdapterError::InvalidRequestBody)?;

    // Collect optional parameters that the fixed interface already validated.
    let mut parameters = serde_json::Map::new();
    if let Some(n) = document.remove("n").filter(|value| !value.is_null()) {
        parameters.insert("n".to_owned(), n);
    }
    if let Some(size) = document
        .remove("size")
        .and_then(|value| value.as_str().map(str::to_owned))
        && size != "auto"
    {
        // DashScope uses `*` between width and height instead of the OpenAI `x` separator.
        parameters.insert(
            "size".to_owned(),
            serde_json::Value::String(size.replace('x', "*")),
        );
    }
    // Resolve DashScope extension defaults explicitly so upstream default drift cannot change behavior.
    let prompt_extend = document
        .remove("prompt_extend")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    parameters.insert(
        "prompt_extend".to_owned(),
        serde_json::Value::Bool(prompt_extend),
    );
    if prompt_extend {
        let prompt_extend_mode = document
            .remove("prompt_extend_mode")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "direct".to_owned());
        parameters.insert(
            "prompt_extend_mode".to_owned(),
            serde_json::Value::String(prompt_extend_mode),
        );
        let enable_thinking = document
            .remove("enable_thinking")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        parameters.insert(
            "enable_thinking".to_owned(),
            serde_json::Value::Bool(enable_thinking),
        );
    } else {
        document.remove("prompt_extend_mode");
        document.remove("enable_thinking");
    }
    for extension in ["negative_prompt", "seed"] {
        if let Some(value) = document.remove(extension).filter(|value| !value.is_null()) {
            parameters.insert(extension.to_owned(), value);
        }
    }
    let watermark = document
        .remove("watermark")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    parameters.insert("watermark".to_owned(), serde_json::Value::Bool(watermark));

    // Downstream-only OpenAI fields never enter the DashScope native wire.
    for field in [
        "background",
        "moderation",
        "output_compression",
        "output_format",
        "partial_images",
        "quality",
        "response_format",
        "stream",
        "style",
        "user",
    ] {
        document.remove(field);
    }

    // Rebuild the trusted DashScope envelope; `user` and `response_format` never leave the gateway.
    document.insert(
        "input".to_owned(),
        serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{ "text": prompt }],
            }]
        }),
    );
    document.insert(
        "parameters".to_owned(),
        serde_json::Value::Object(parameters),
    );
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
        Some("qwen3.8-max" | "qwen3.8-27b" | "qwen3.7-max" | "qwen3.7-plus")
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
        Some("deepseek-v4-pro-0813" | "deepseek-v4-flash-0731")
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

#[cfg(test)]
mod tests {
    use http::HeaderName;

    use super::*;
    use crate::core::OperationKind;

    const WAIT_HEADER: HeaderName = HeaderName::from_static("x-dashscope-wait-timeout");
    const CACHE_HEADER: HeaderName = HeaderName::from_static("x-dashscope-session-cache");

    #[test]
    fn routed_headers_enable_waiting_and_scope_responses_cache_to_supported_models() {
        // Apply server-side burst waiting to every routed Bailian operation.
        let mut chat = SafeHeaders::default();
        transform_routed_request_headers(OperationKind::ChatCompletions, "qwen3.8-max", &mut chat)
            .unwrap();
        assert_eq!(chat.get(WAIT_HEADER).unwrap(), "30");
        assert!(chat.get(CACHE_HEADER).is_none());

        // Enable session cache only for the officially listed Responses models.
        for model in ["qwen3.8-max", "qwen3.7-max", "qwen3.7-plus"] {
            let mut responses = SafeHeaders::default();
            transform_routed_request_headers(OperationKind::Responses, model, &mut responses)
                .unwrap();
            assert_eq!(responses.get(WAIT_HEADER).unwrap(), "30");
            assert_eq!(responses.get(CACHE_HEADER).unwrap(), "enable");
        }

        let mut unsupported = SafeHeaders::default();
        transform_routed_request_headers(OperationKind::Responses, "qwen3.8-27b", &mut unsupported)
            .unwrap();
        assert_eq!(unsupported.get(WAIT_HEADER).unwrap(), "30");
        assert!(unsupported.get(CACHE_HEADER).is_none());
    }
}
