//! Static ChatGPT Provider contract and Codex Responses wire profile.
//!
//! This profile permits only OAuth bearer credentials and fixed Codex backend paths. It does not
//! expose Chat Completions, Embeddings, WebSocket, or a generic OpenAI-compatible endpoint.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, ImageDetailPolicy, ImageInputCapabilities,
        ImageMediaType, ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonSchemaSupport, ProviderResponsesCapabilities, ProviderResponsesStateCeiling,
        ReasoningOutput, ResponseInclude, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        ProviderRequestHeaders, SafeHeaders, StaticRequestHeader,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

const CHATGPT_IDENTITY_HEADERS: &[StaticRequestHeader] = &[
    StaticRequestHeader::new("accept", "text/event-stream"),
    StaticRequestHeader::new("originator", "codex_cli_rs"),
];
/// Fixed headless Linux profile derived from the Codex CLI `rust-v0.146.0` User-Agent format.
const CODEX_CLI_LINUX_USER_AGENT: &str = "codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown";
const CHATGPT_REQUEST_HEADERS: ProviderRequestHeaders = ProviderRequestHeaders::new()
    .with_user_agent(CODEX_CLI_LINUX_USER_AGENT)
    .with_headers(CHATGPT_IDENTITY_HEADERS);
const RESPONSES_INCLUDES: &[ResponseInclude] = &[ResponseInclude::ReasoningEncryptedContent];
const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[
    ImageMediaType::Jpeg,
    ImageMediaType::Png,
    ImageMediaType::Gif,
    ImageMediaType::Webp,
];
/// Conservative Codex Responses profile for one inline image without explicit detail controls.
pub(super) const IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    1,
    ImageSourceCapabilities::DataUrl(InlineImageInputProfile::new(
        IMAGE_MEDIA_TYPES,
        InlineImageInputLimits::new(
            20 * 1024 * 1024,
            15 * 1024 * 1024,
            20 * 1024 * 1024,
            15 * 1024 * 1024,
        ),
    )),
    ImageDetailPolicy::OmittedOnly { default: None },
);

/// Single ChatGPT operation surface shared by the Provider contract and wire adapter.
const API_SURFACE: OpenAiCompatibleApiSurface = OpenAiCompatibleApiSurface::new(
    None,
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
            media: crate::core::ResponsesMediaProfile::new(Some(IMAGE_INPUT), None),
            structured_outputs: Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
                JsonSchemaSupport::StrictSupported,
            )),
            state: ProviderResponsesStateCeiling::Stateless,
            background: false,
            reasoning_output: ReasoningOutput::Summary,
            custom_tool_calling: false,
            hosted_tools: &[],
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

/// Responses-only wire profile used by the fixed ChatGPT Codex backend.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::ChatGpt,
    API_SURFACE,
    "/models?client_version=0.146.0",
    transform_request_headers,
)
.with_request_body_hook(transform_request_body)
.with_request_headers(CHATGPT_REQUEST_HEADERS)
.with_openai_data_type_responses_terminal()
.with_missing_responses_content_type_as_sse()
.with_model_list_parser(parse_model_list_ids);

/// Single static descriptor for the ChatGPT contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::OAuth2BearerAccessToken],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Keeps downstream headers out of the fixed ChatGPT request identity.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Extracts ChatGPT Codex model slugs from its manifest response envelope.
fn parse_model_list_ids(response: &serde_json::Value) -> Option<Vec<String>> {
    Some(
        response
            .get("models")?
            .as_array()?
            .iter()
            .filter_map(|entry| entry.get("slug").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect(),
    )
}

/// Narrows standard Responses input to the current Codex backend's streaming request envelope.
fn transform_request_body(
    protocol: crate::core::ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Require the SSE-only Responses shape and reject fields the private backend does not accept.
    if protocol != crate::core::ApiProtocol::Responses
        || document.get("stream").and_then(serde_json::Value::as_bool) != Some(true)
        || ["max_output_tokens", "max_completion_tokens", "max_tokens"]
            .iter()
            .any(|field| document.contains_key(*field))
    {
        return Err(AdapterError::InvalidRequestBody);
    }

    // Convert the standard Responses string shorthand into an equivalent user input message.
    if let Some(text) = document
        .get("input")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    {
        document.insert(
            "input".to_owned(),
            serde_json::json!([{
                "role": "user",
                "content": [{"type": "input_text", "text": text}],
            }]),
        );
    }
    if !document
        .get("input")
        .is_some_and(serde_json::Value::is_array)
    {
        return Err(AdapterError::InvalidRequestBody);
    }

    // The ChatGPT Codex backend requires stateless storage semantics on every request.
    match document.get("store") {
        None | Some(serde_json::Value::Bool(false)) => {
            document.insert("store".to_owned(), serde_json::Value::Bool(false));
        }
        Some(_) => return Err(AdapterError::InvalidRequestBody),
    }
    Ok(())
}
