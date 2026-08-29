//! Shared HTTP JSON/SSE wire implementation for OpenAI-compatible Providers.
//!
//! Provider identity, capabilities, endpoint paths, and request-header hooks remain owned by each
//! Provider's compile-time definition; this module only reuses protocol mechanics and provides no
//! dynamic Provider DSL or runtime transform configuration.

mod embeddings;
mod headers;
mod registration;
mod request;
mod response;
mod surface;
#[cfg(test)]
mod tests;

use http::HeaderMap;

use crate::{
    core::{ApiProtocol, EmbeddingEncoding, EmbeddingEncodingPolicy, OperationKind},
    credential::UpstreamCredential,
    provider::{AdapterError, ProviderKind, ProviderRequestHeaders, SafeHeaders, SensitiveHeaders},
};

pub(crate) use registration::native_upstream_apis;
pub(crate) use request::take_chat_reasoning_switch;
pub(crate) use surface::{OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint};

/// Compile-time Provider hook for transforming ordinary headers according to Provider rules.
pub(crate) type RequestHeaderHook = fn(&HeaderMap, &mut SafeHeaders) -> Result<(), AdapterError>;
/// Compile-time Provider hook for one trusted routed operation and upstream model.
pub(crate) type RoutedRequestHeaderHook =
    fn(OperationKind, &str, &mut SafeHeaders) -> Result<(), AdapterError>;
/// Compile-time Provider hook for account-bound sensitive authentication context.
pub(crate) type AuthenticationContextHook =
    fn(&UpstreamCredential<'_>) -> Result<SensitiveHeaders, AdapterError>;
/// Compile-time Provider hook for narrowing one parsed protocol request to its fixed wire contract.
pub(crate) type RequestBodyHook =
    fn(ApiProtocol, &mut serde_json::Map<String, serde_json::Value>) -> Result<(), AdapterError>;
/// Compile-time Provider hook for converting one preflighted Images request to its native wire.
pub(crate) type ImagesRequestBodyHook =
    fn(&mut serde_json::Map<String, serde_json::Value>) -> Result<(), AdapterError>;
/// Compile-time Provider hook for extracting model identifiers from a model-list response.
pub(crate) type ModelListParser = fn(&serde_json::Value) -> Option<Vec<String>>;

#[derive(Clone, Copy)]
/// Source used to identify OpenAI terminal event names in SSE events.
enum OpenAiTerminalDiscriminator {
    /// Reads the terminal name from the SSE `event:` field.
    SseEventField,
    /// Reads the terminal name from the top-level `type` field in the data JSON.
    DataJsonType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Static policy for recognizing successful SSE response media types.
enum StreamingResponseMediaTypePolicy {
    /// Requires exactly one explicit `text/event-stream` Content-Type value.
    RequireEventStream,
    /// Allows a Responses stream to omit Content-Type while rejecting present non-SSE values.
    AllowMissingForResponses,
}

/// A static OpenAI-compatible wire profile.
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleAdapter {
    kind: ProviderKind,
    chat_path: Option<&'static str>,
    responses_path: Option<&'static str>,
    embeddings_path: Option<&'static str>,
    images_path: Option<&'static str>,
    model_list_path: &'static str,
    model_list_parser: ModelListParser,
    request_header_hook: RequestHeaderHook,
    routed_request_header_hook: RoutedRequestHeaderHook,
    authentication_context_hook: AuthenticationContextHook,
    request_body_hook: RequestBodyHook,
    images_request_body_hook: ImagesRequestBodyHook,
    request_headers: ProviderRequestHeaders,
    responses_terminal_discriminator: OpenAiTerminalDiscriminator,
    streaming_response_media_type_policy: StreamingResponseMediaTypePolicy,
}

impl OpenAiCompatibleAdapter {
    /// Builds the static wire profile owned by the concrete Provider.
    pub(crate) const fn new(
        kind: ProviderKind,
        api_surface: OpenAiCompatibleApiSurface,
        model_list_path: &'static str,
        request_header_hook: RequestHeaderHook,
    ) -> Self {
        Self {
            kind,
            chat_path: api_surface.chat_path(),
            responses_path: api_surface.responses_path(),
            embeddings_path: api_surface.embeddings_path(),
            images_path: api_surface.images_path(),
            model_list_path,
            model_list_parser: request::parse_openai_model_list_ids,
            request_header_hook,
            routed_request_header_hook: headers::preserve_routed_request_headers,
            authentication_context_hook: headers::empty_authentication_context,
            request_body_hook: request::preserve_request_body,
            images_request_body_hook: request::convert_images_to_openai_compatible_shape,
            request_headers: ProviderRequestHeaders::new(),
            responses_terminal_discriminator: OpenAiTerminalDiscriminator::SseEventField,
            streaming_response_media_type_policy:
                StreamingResponseMediaTypePolicy::RequireEventStream,
        }
    }

    /// Attaches the concrete Provider's bounded request-body transformation.
    pub(crate) const fn with_request_body_hook(
        mut self,
        request_body_hook: RequestBodyHook,
    ) -> Self {
        self.request_body_hook = request_body_hook;
        self
    }

    /// Normalizes one bounded Embeddings response to the downstream requested encoding.
    pub(crate) fn normalize_embedding_response_body(
        self,
        body: &[u8],
        requested_encoding: EmbeddingEncoding,
        policy: EmbeddingEncodingPolicy,
    ) -> Result<Vec<u8>, AdapterError> {
        embeddings::normalize_response_body(body, requested_encoding, policy)
    }

    /// Attaches headers selected only from a trusted routed operation and upstream model.
    pub(crate) const fn with_routed_request_header_hook(
        mut self,
        routed_request_header_hook: RoutedRequestHeaderHook,
    ) -> Self {
        self.routed_request_header_hook = routed_request_header_hook;
        self
    }

    /// Attaches the concrete Provider's account-bound sensitive authentication context.
    pub(crate) const fn with_authentication_context_hook(
        mut self,
        authentication_context_hook: AuthenticationContextHook,
    ) -> Self {
        self.authentication_context_hook = authentication_context_hook;
        self
    }

    /// Attaches the concrete Provider's Images request-body wire conversion.
    pub(crate) const fn with_images_request_body_hook(
        mut self,
        images_request_body_hook: ImagesRequestBodyHook,
    ) -> Self {
        self.images_request_body_hook = images_request_body_hook;
        self
    }

    /// Attaches the concrete Provider's bounded model-list response parser.
    pub(crate) const fn with_model_list_parser(mut self, parser: ModelListParser) -> Self {
        self.model_list_parser = parser;
        self
    }

    /// Attaches fixed non-sensitive request headers owned by the concrete Provider definition.
    pub(crate) const fn with_request_headers(
        mut self,
        request_headers: ProviderRequestHeaders,
    ) -> Self {
        self.request_headers = request_headers;
        self
    }

    /// Reads the OpenAI Responses terminal name from the top-level `type` field in data JSON.
    pub(crate) const fn with_openai_data_type_responses_terminal(mut self) -> Self {
        self.responses_terminal_discriminator = OpenAiTerminalDiscriminator::DataJsonType;
        self
    }

    /// Allows the concrete Provider's Responses endpoint to omit SSE Content-Type on success.
    pub(crate) const fn with_missing_responses_content_type_as_sse(mut self) -> Self {
        self.streaming_response_media_type_policy =
            StreamingResponseMediaTypePolicy::AllowMissingForResponses;
        self
    }

    /// Returns the trusted Images Generations path when that operation is present.
    pub(crate) const fn images_path(self) -> Option<&'static str> {
        self.images_path
    }

    /// Returns the Provider kind that owns this closed wire profile.
    pub(crate) const fn kind(self) -> ProviderKind {
        self.kind
    }
}
