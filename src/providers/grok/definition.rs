//! Static Grok Provider contract and subscription CLI-proxy Responses wire profile.
//!
//! This profile permits only OAuth bearer credentials and the fixed Grok Build CLI proxy path. It
//! does not expose Chat Completions, Embeddings, WebSocket, media generation, or the open
//! `api.x.ai` endpoint.

use http::HeaderMap;

use crate::{
    core::{
        ALL_TOOL_CHOICE_MODES, FunctionToolCapabilities, JsonSchemaSupport,
        ProviderResponsesCapabilities, ProviderResponsesStateCeiling, ReasoningOutput,
        ResponseInclude, StructuredOutputProfile,
    },
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderDefinition, ProviderKind,
        ProviderRequestHeaders, SafeHeaders, StaticRequestHeader,
    },
    providers::openai_compatible::{
        OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint,
    },
};

const GROK_IDENTITY_HEADERS: &[StaticRequestHeader] = &[
    StaticRequestHeader::new("accept", "text/event-stream"),
    // Fixed Grok Build CLI identity required by the subscription proxy host.
    StaticRequestHeader::new("x-xai-token-auth", "xai-grok-cli"),
    StaticRequestHeader::new("x-grok-client-version", GROK_CLI_CLIENT_VERSION),
    StaticRequestHeader::new("x-grok-client-identifier", "grok-shell"),
];
/// Pinned Grok CLI client version; bump via commit when the upstream client drifts.
const GROK_CLI_CLIENT_VERSION: &str = "0.2.120";
const GROK_REQUEST_HEADERS: ProviderRequestHeaders = ProviderRequestHeaders::new()
    .with_user_agent("xai-grok-workspace/0.2.120")
    .with_headers(GROK_IDENTITY_HEADERS);
const RESPONSES_INCLUDES: &[ResponseInclude] = &[ResponseInclude::ReasoningEncryptedContent];

/// Single Grok operation surface shared by the Provider contract and wire adapter.
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
            media: crate::core::ResponsesMediaProfile::new(None, None),
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

/// Responses-only wire profile used by the fixed Grok subscription CLI proxy.
///
/// The CLI proxy emits standard `event:`-framed Responses SSE, so the default SSE event-field
/// terminal discriminator applies; the strict media-type policy is retained because the proxy's
/// Content-Type behavior has no local evidence.
const ADAPTER: OpenAiCompatibleAdapter = OpenAiCompatibleAdapter::new(
    ProviderKind::Grok,
    API_SURFACE,
    "/models",
    transform_request_headers,
)
.with_request_headers(GROK_REQUEST_HEADERS);

/// Single static descriptor for the Grok contract and adapter.
pub(crate) static DEFINITION: ProviderDefinition = ProviderDefinition::new(
    API_SURFACE.capabilities(),
    &[CredentialKind::OAuth2BearerAccessToken],
    ProviderAdapter::from_openai_compatible(ADAPTER),
);

/// Keeps downstream headers out of the fixed Grok request identity.
fn transform_request_headers(
    _downstream: &HeaderMap,
    _upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    Ok(())
}
