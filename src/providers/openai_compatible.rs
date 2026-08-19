//! Shared HTTP JSON/SSE wire implementation for OpenAI-compatible Providers.
//!
//! Provider identity, capabilities, endpoint paths, and request-header hooks remain owned by each
//! Provider's compile-time definition; this module only reuses protocol mechanics and provides no
//! dynamic Provider DSL or runtime transform configuration.

use bytes::Bytes;
use http::{
    HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use zeroize::Zeroizing;

use crate::{
    core::{
        ApiCapabilities, ApiProtocol, ApiRequest, ChatCompletionsCapabilities, EmbeddingRequest,
        EmbeddingsCapabilities, OperationKind, ProviderChatCompletionsCapabilities,
        ProviderOperationCapabilities, ProviderResponsesCapabilities, ResponsesCapabilities,
    },
    credential::CredentialType,
    provider::{
        AdapterError, ClassifiedSseEvent, PreparedUpstreamRequest, ProviderKind,
        ProviderRequestHeaders, RetryHint, SafeHeaders, SensitiveHeaders, StatusClassification,
        StreamEventStatus, UpstreamErrorKind,
    },
    registry::{
        CanonicalTaskKind, ReasoningLevel, ReasoningLevelMapping, UpstreamApi,
        UpstreamApiCapabilities, UpstreamApiConfig, UpstreamApiKey, UpstreamApiModelRules,
    },
    transport::sse::SseEvent,
};

/// Compile-time Provider hook for transforming ordinary headers according to Provider rules.
pub(crate) type RequestHeaderHook = fn(&HeaderMap, &mut SafeHeaders) -> Result<(), AdapterError>;
/// Compile-time Provider hook for narrowing one parsed protocol request to its fixed wire contract.
pub(crate) type RequestBodyHook =
    fn(ApiProtocol, &mut serde_json::Map<String, serde_json::Value>) -> Result<(), AdapterError>;
/// Compile-time Provider hook for extracting model identifiers from a model-list response.
pub(crate) type ModelListParser = fn(&serde_json::Value) -> Option<Vec<String>>;

/// One fixed OpenAI-compatible operation endpoint paired with its capability ceiling.
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleEndpoint<T> {
    relative_path: &'static str,
    capabilities: T,
}

impl<T> OpenAiCompatibleEndpoint<T> {
    /// Pairs one trusted relative endpoint path with the capabilities implemented there.
    pub(crate) const fn new(relative_path: &'static str, capabilities: T) -> Self {
        Self {
            relative_path,
            capabilities,
        }
    }
}

/// Closed operation surface shared by one Provider contract and its wire adapter.
#[derive(Clone, Copy)]
pub(crate) struct OpenAiCompatibleApiSurface {
    chat_completions: Option<OpenAiCompatibleEndpoint<ProviderChatCompletionsCapabilities>>,
    responses: Option<OpenAiCompatibleEndpoint<ProviderResponsesCapabilities>>,
    embeddings: Option<OpenAiCompatibleEndpoint<EmbeddingsCapabilities>>,
}

impl OpenAiCompatibleApiSurface {
    /// Creates one operation surface; an absent endpoint is an unsupported operation.
    pub(crate) const fn new(
        chat_completions: Option<OpenAiCompatibleEndpoint<ProviderChatCompletionsCapabilities>>,
        responses: Option<OpenAiCompatibleEndpoint<ProviderResponsesCapabilities>>,
        embeddings: Option<OpenAiCompatibleEndpoint<EmbeddingsCapabilities>>,
    ) -> Self {
        Self {
            chat_completions,
            responses,
            embeddings,
        }
    }

    /// Projects the Provider capability contract from the same typed endpoint descriptors.
    pub(crate) const fn capabilities(&'static self) -> ApiCapabilities {
        ApiCapabilities::from_indexed_operations([
            match &self.chat_completions {
                Some(endpoint) => Some(ProviderOperationCapabilities::ChatCompletions(
                    &endpoint.capabilities,
                )),
                None => None,
            },
            match &self.responses {
                Some(endpoint) => Some(ProviderOperationCapabilities::Responses(
                    &endpoint.capabilities,
                )),
                None => None,
            },
            match &self.embeddings {
                Some(endpoint) => Some(ProviderOperationCapabilities::Embeddings(
                    &endpoint.capabilities,
                )),
                None => None,
            },
        ])
    }

    /// Returns the trusted Chat Completions path when that operation is present.
    const fn chat_path(self) -> Option<&'static str> {
        match self.chat_completions {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }

    /// Returns the trusted Responses path when that operation is present.
    const fn responses_path(self) -> Option<&'static str> {
        match self.responses {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }

    /// Returns the trusted Embeddings path when that operation is present.
    const fn embeddings_path(self) -> Option<&'static str> {
        match self.embeddings {
            Some(endpoint) => Some(endpoint.relative_path),
            None => None,
        }
    }
}

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
    model_list_path: &'static str,
    model_list_parser: ModelListParser,
    request_header_hook: RequestHeaderHook,
    request_body_hook: RequestBodyHook,
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
            model_list_path,
            model_list_parser: parse_openai_model_list_ids,
            request_header_hook,
            request_body_hook: preserve_request_body,
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

    /// Returns the Provider kind that owns this closed wire profile.
    pub(crate) const fn kind(self) -> ProviderKind {
        self.kind
    }

    /// Builds the fixed model-list request used by the administrative probe.
    pub(crate) fn prepare_model_list_request(
        self,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Bind the static OpenAI-compatible path without accepting a URL or query override.
        let relative_uri = Uri::from_static(self.model_list_path);
        Ok(PreparedUpstreamRequest::new(
            Method::GET,
            relative_uri,
            Bytes::new(),
        ))
    }

    /// Extracts model identifiers through the selected static response-envelope profile.
    pub(crate) fn model_list_ids(self, response: &serde_json::Value) -> Option<Vec<String>> {
        (self.model_list_parser)(response)
    }

    /// Replaces the upstream model and binds the profile's declared relative endpoint.
    pub(crate) fn prepare_request(
        self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.prepare_request_with_api(request, upstream_model, None)
    }

    /// Replaces target-specific wire values and binds the selected Upstream API endpoint.
    pub(crate) fn prepare_routed_request(
        self,
        request: &ApiRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.prepare_request_with_api(request, upstream_api.upstream_model(), Some(upstream_api))
    }

    /// Replaces the Public Model and binds the fixed Native Embeddings endpoint.
    pub(crate) fn prepare_embedding_routed_request(
        self,
        request: &EmbeddingRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Require an Embeddings API and a Provider profile with one trusted relative path.
        if upstream_api.operation() != OperationKind::EmbeddingsCreate {
            return Err(AdapterError::UnsupportedProtocol);
        }
        let path = self
            .embeddings_path
            .ok_or(AdapterError::UnsupportedProtocol)?;

        // Parse the preflighted object and replace only the registry-owned model field.
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_api.upstream_model().to_owned()),
            );

        // Re-serialize once without converting input, encoding, dimensions, or user fields.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(PreparedUpstreamRequest::new(
            Method::POST,
            Uri::from_static(path),
            body,
        ))
    }

    /// Builds one JSON request and optionally applies mappings from the selected Upstream API.
    fn prepare_request_with_api(
        self,
        request: &ApiRequest,
        upstream_model: &str,
        routed: Option<&UpstreamApi>,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Select the static relative endpoint for the request protocol.
        let path = match request.protocol() {
            ApiProtocol::ChatCompletions => self.chat_path,
            ApiProtocol::Responses => self.responses_path,
        }
        .ok_or(AdapterError::UnsupportedProtocol)?;
        let relative_uri = Uri::from_static(path);

        // Parse and replace the upstream model field controlled only by the adapter.
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_model.to_owned()),
            );

        // Apply only the concrete Provider's compile-time request-shape narrowing.
        (self.request_body_hook)(
            request.protocol(),
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
        )?;

        // Remove ordinary fields that the selected API is configured to accept only downstream.
        if let Some(upstream_api) = routed {
            discard_ignored_generation_parameters(
                document
                    .as_object_mut()
                    .ok_or(AdapterError::InvalidRequestBody)?,
                upstream_api,
            );
        }

        // Apply only the selected Upstream API's explicit reasoning wire mapping.
        let reasoning_level_mapping = routed.and_then(|upstream_api| {
            apply_reasoning_level_mapping(
                request.protocol(),
                document.as_object_mut()?,
                upstream_api,
            )
        });

        // Re-serialize once after all trusted Provider wire transformations.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(
            PreparedUpstreamRequest::new(Method::POST, relative_uri, body)
                .with_reasoning_level_mapping(reasoning_level_mapping),
        )
    }

    /// Builds the base ordinary headers for an OpenAI-compatible JSON request.
    pub(crate) fn prepare_headers(self) -> Result<SafeHeaders, AdapterError> {
        let mut headers = SafeHeaders::default();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"))?;
        Ok(headers)
    }

    /// Applies the ordinary-header transform defined by the concrete Provider.
    pub(crate) fn apply_request_header_hook(
        self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        (self.request_header_hook)(downstream_headers, headers)
    }

    /// Applies fixed Provider request headers after the downstream-header hook.
    pub(crate) fn apply_configured_request_headers(
        self,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        self.request_headers.apply_to(headers)
    }

    /// Builds a Bearer authentication header bound to the Provider identity.
    pub(crate) fn prepare_auth_headers(
        self,
        credential: &crate::credential::UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        // Verify credential ownership to prevent cross-Provider secret reuse.
        if credential.provider() != self.kind {
            return Err(AdapterError::CredentialProviderMismatch);
        }
        let CredentialType::Upstream(kind) = credential.metadata().credential_type() else {
            return Err(AdapterError::CredentialKindMismatch);
        };
        if !self.kind.contract().credential_kinds().contains(&kind) {
            return Err(AdapterError::CredentialKindMismatch);
        }

        // Assemble the sensitive Bearer header inside a zeroizing string.
        let mut bearer = Zeroizing::new("Bearer ".to_owned());
        bearer.push_str(credential.expose_secret());
        let mut headers = SensitiveHeaders::default();
        headers.insert(AUTHORIZATION, bearer);

        // Bind ChatGPT subscription requests to the selected account and conditional FedRAMP edge.
        if self.kind == ProviderKind::ChatGpt {
            let account_id = credential
                .expose_chatgpt_account_id()
                .ok_or(AdapterError::IncompleteAuthenticationContext)?;
            headers.insert(
                HeaderName::from_static("chatgpt-account-id"),
                Zeroizing::new(account_id.to_owned()),
            );
            let is_fedramp_account = credential
                .is_fedramp_account()
                .ok_or(AdapterError::IncompleteAuthenticationContext)?;
            if is_fedramp_account {
                headers.insert(
                    HeaderName::from_static("x-openai-fedramp"),
                    Zeroizing::new("true".to_owned()),
                );
            }
        }
        Ok(headers)
    }

    /// Identifies OpenAI-compatible Chat/Responses SSE terminal or failure events.
    pub(crate) fn classify_sse_event(
        self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> ClassifiedSseEvent {
        let status = match protocol {
            ApiProtocol::ChatCompletions if event.data() == "[DONE]" => {
                StreamEventStatus::Completed
            }
            ApiProtocol::Responses => self.classify_responses_sse_event(&event),
            _ => StreamEventStatus::Continue,
        };
        ClassifiedSseEvent::new(event, status)
    }

    /// Returns whether response headers satisfy this Provider's trusted SSE media profile.
    pub(crate) fn recognizes_sse_response(
        self,
        protocol: ApiProtocol,
        headers: &HeaderMap,
    ) -> bool {
        // Reject duplicate values before interpreting the media type.
        let mut values = headers.get_all(CONTENT_TYPE).iter();
        let Some(value) = values.next() else {
            return protocol == ApiProtocol::Responses
                && self.streaming_response_media_type_policy
                    == StreamingResponseMediaTypePolicy::AllowMissingForResponses;
        };
        if values.next().is_some() {
            return false;
        }

        // Compare only the media type token while allowing case and parameters.
        value
            .to_str()
            .ok()
            .and_then(|value| value.split(';').next())
            .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/event-stream"))
    }

    /// Identifies Responses SSE terminal states using the concrete OpenAI-compatible profile.
    fn classify_responses_sse_event(self, event: &SseEvent) -> StreamEventStatus {
        classify_openai_responses_terminal(event, self.responses_terminal_discriminator)
    }

    /// Maps an OpenAI-compatible HTTP status to an error and retry classification.
    pub(crate) fn classify_status(self, status: StatusCode) -> StatusClassification {
        // Select the error class from the OpenAI-compatible status family, then decide whether pre-output retry is allowed.
        let (kind, retry_hint) = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                (UpstreamErrorKind::InvalidRequest, RetryHint::Never)
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                (UpstreamErrorKind::Authentication, RetryHint::Never)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                (UpstreamErrorKind::RateLimited, RetryHint::BeforeFirstEvent)
            }
            status if status.is_server_error() => (
                UpstreamErrorKind::UpstreamUnavailable,
                RetryHint::BeforeFirstEvent,
            ),
            _ => (UpstreamErrorKind::UpstreamFailure, RetryHint::Never),
        };
        StatusClassification::new(kind, retry_hint)
    }
}

/// Extracts model identifiers from the common OpenAI-compatible `data[].id` envelope.
fn parse_openai_model_list_ids(response: &serde_json::Value) -> Option<Vec<String>> {
    Some(
        response
            .get("data")?
            .as_array()?
            .iter()
            .filter_map(|entry| entry.get("id").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect(),
    )
}

/// Removes only the closed ordinary-generation fields configured for the selected Upstream API.
fn discard_ignored_generation_parameters(
    document: &mut serde_json::Map<String, serde_json::Value>,
    upstream_api: &UpstreamApi,
) {
    for parameter in upstream_api.ignored_generation_parameters() {
        document.remove(parameter.as_wire_name());
    }
}

/// Keeps ordinary OpenAI-compatible request bodies unchanged after model replacement.
fn preserve_request_body(
    _protocol: ApiProtocol,
    _document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Removes a standard Chat reasoning level and returns its equivalent thinking-switch state.
pub(crate) fn take_chat_reasoning_switch(
    protocol: ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Option<bool>, AdapterError> {
    // Leave non-Chat requests and requests without an explicit standard level unchanged.
    if protocol != ApiProtocol::ChatCompletions {
        return Ok(None);
    }
    let Some(level) = document.remove("reasoning_effort") else {
        return Ok(None);
    };

    // Treat `none` as disabled and every other recognized model level as enabled.
    let level = level
        .as_str()
        .and_then(ReasoningLevel::from_wire)
        .ok_or(AdapterError::InvalidRequestBody)?;
    Ok(Some(level != ReasoningLevel::None))
}

/// Applies one canonical reasoning level mapping at the protocol-defined wire location.
fn apply_reasoning_level_mapping(
    protocol: ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
    upstream_api: &UpstreamApi,
) -> Option<ReasoningLevelMapping> {
    // Locate only the standard reasoning field for the prepared request protocol.
    let value = match protocol {
        ApiProtocol::ChatCompletions => document.get_mut("reasoning_effort"),
        ApiProtocol::Responses => document
            .get_mut("reasoning")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|reasoning| reasoning.get_mut("effort")),
    }?;

    // Resolve and write only an explicitly configured canonical-to-Provider mapping.
    let downstream = value.as_str().and_then(ReasoningLevel::from_wire)?;
    let upstream = upstream_api.reasoning_level_mapping(downstream)?.to_owned();
    *value = serde_json::Value::String(upstream.clone());
    Some(ReasoningLevelMapping {
        downstream,
        upstream,
    })
}

/// Reads an OpenAI Responses terminal using the compile-time discriminator and rejects conflicting sources.
fn classify_openai_responses_terminal(
    event: &SseEvent,
    discriminator: OpenAiTerminalDiscriminator,
) -> StreamEventStatus {
    // Read the terminal source selected by the profile; remain non-terminal when it is absent.
    let (selected, corroborating) = match discriminator {
        OpenAiTerminalDiscriminator::SseEventField => {
            let selected = classify_openai_terminal_name(event.event());
            let corroborating = selected.and_then(|_| classify_data_json_openai_terminal(event));
            (selected, corroborating)
        }
        OpenAiTerminalDiscriminator::DataJsonType => (
            classify_data_json_openai_terminal(event),
            classify_openai_terminal_name(event.event()),
        ),
    };
    let Some(selected) = selected else {
        return StreamEventStatus::Continue;
    };

    // Fail closed when both explicit terminal sources conflict in one event.
    if corroborating.is_some_and(|status| status != selected) {
        StreamEventStatus::Failed
    } else {
        selected
    }
}

/// Maps an OpenAI Responses terminal name to the unified stream state.
fn classify_openai_terminal_name(name: Option<&str>) -> Option<StreamEventStatus> {
    // Convert only protocol-defined terminal names into unified lifecycle states.
    match name {
        Some("response.completed") => Some(StreamEventStatus::Completed),
        Some("response.failed" | "response.incomplete") => Some(StreamEventStatus::Failed),
        _ => None,
    }
}

/// Extracts an OpenAI Responses terminal name from the top-level `type` in data JSON.
fn classify_data_json_openai_terminal(event: &SseEvent) -> Option<StreamEventStatus> {
    // Parse the minimal event envelope without retaining or logging business content.
    let document = serde_json::from_str::<serde_json::Value>(event.data()).ok()?;
    classify_openai_terminal_name(document.get("type").and_then(serde_json::Value::as_str))
}

/// Builds Chat followed by an optional Responses HTTP JSON/SSE Upstream API.
pub(crate) fn native_upstream_apis(
    upstream_model: &str,
    task: CanonicalTaskKind,
    chat_capabilities: ChatCompletionsCapabilities,
    responses_capabilities: Option<ResponsesCapabilities>,
) -> Vec<UpstreamApiConfig> {
    // Build the required stateless Chat API as the first operation.
    let mut upstream_apis = vec![UpstreamApiConfig {
        key: UpstreamApiKey::new(OperationKind::ChatCompletions, task),
        upstream_model: upstream_model.to_owned(),
        model_rules: UpstreamApiModelRules::default(),
        capabilities: UpstreamApiCapabilities::ChatCompletions(chat_capabilities),
        streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
    }];

    // Append the target-bound Responses API only when the target exposes that operation.
    if let Some(responses_capabilities) = responses_capabilities {
        upstream_apis.push(UpstreamApiConfig {
            key: UpstreamApiKey::new(OperationKind::Responses, task),
            upstream_model: upstream_model.to_owned(),
            model_rules: UpstreamApiModelRules::default(),
            capabilities: UpstreamApiCapabilities::Responses(responses_capabilities),
            streaming_policy: crate::registry::UpstreamStreamingPolicy::Optional,
        });
    }
    upstream_apis
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue, header::USER_AGENT};
    use serde_json::json;

    use super::*;

    fn transform_headers(
        downstream: &HeaderMap,
        upstream: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        // Forward the selected downstream metadata for the synthetic Provider policy.
        let source = HeaderName::from_static("x-source-name");
        let target = HeaderName::from_static("x-target-name");
        if let Some(value) = downstream.get(source) {
            upstream.insert(target, value.clone())?;
        }
        if let Some(value) = downstream.get(USER_AGENT) {
            upstream.insert(USER_AGENT, value.clone())?;
        }

        // Remove the shared JSON header to exercise hook deletion independently.
        upstream.remove(CONTENT_TYPE);
        Ok(())
    }

    #[test]
    fn provider_hook_and_fixed_headers_apply_in_deterministic_order() {
        const FIXED_HEADERS: &[crate::provider::StaticRequestHeader] =
            &[crate::provider::StaticRequestHeader::new(
                "x-provider-fixed",
                "fixed-value",
            )];
        const REQUEST_HEADERS: ProviderRequestHeaders = ProviderRequestHeaders::new()
            .with_user_agent("fixed-provider-client/1.0")
            .with_headers(FIXED_HEADERS);

        // Configure one synthetic adapter and conflicting downstream identity.
        let adapter = OpenAiCompatibleAdapter::new(
            ProviderKind::OpenAi,
            OpenAiCompatibleApiSurface::new(
                Some(OpenAiCompatibleEndpoint::new(
                    "/chat",
                    ProviderChatCompletionsCapabilities::default(),
                )),
                Some(OpenAiCompatibleEndpoint::new(
                    "/responses",
                    ProviderResponsesCapabilities::default(),
                )),
                None,
            ),
            "/models",
            transform_headers,
        )
        .with_request_headers(REQUEST_HEADERS);
        let mut downstream = HeaderMap::new();
        downstream.insert(
            HeaderName::from_static("x-source-name"),
            HeaderValue::from_static("transformed-value"),
        );
        downstream.insert(
            USER_AGENT,
            HeaderValue::from_static("downstream-client/1.0"),
        );

        // Run the hook before the fixed profile, matching production request assembly.
        let mut upstream = adapter.prepare_headers().unwrap();
        adapter
            .apply_request_header_hook(&downstream, &mut upstream)
            .unwrap();
        adapter
            .apply_configured_request_headers(&mut upstream)
            .unwrap();

        // Verify deletion, fixed-header precedence, and downstream transformation together.
        assert!(upstream.get(CONTENT_TYPE).is_none());
        assert_eq!(
            upstream.get(USER_AGENT).unwrap(),
            "fixed-provider-client/1.0"
        );
        assert_eq!(
            upstream
                .get(HeaderName::from_static("x-provider-fixed"))
                .unwrap(),
            "fixed-value"
        );
        assert_eq!(
            upstream
                .get(HeaderName::from_static("x-target-name"))
                .unwrap(),
            "transformed-value"
        );
    }

    #[test]
    fn provider_model_list_profiles_bind_paths_and_response_envelopes() {
        // Keep the generic OpenAI-compatible path and data envelope as the default profile.
        let openai = crate::provider::ProviderAdapter::for_kind(ProviderKind::OpenAi);
        assert_eq!(
            openai
                .prepare_model_list_request()
                .unwrap()
                .relative_uri()
                .to_string(),
            "/v1/models"
        );
        assert_eq!(
            openai.model_list_ids(&json!({"data": [{"id": "gpt-5.6-sol"}]})),
            Some(vec!["gpt-5.6-sol".to_owned()])
        );

        // Bind LongCat's OpenAI-compatible model-list endpoint under its /openai/v1 prefix.
        let longcat = crate::provider::ProviderAdapter::for_kind(ProviderKind::LongCat);
        assert_eq!(
            longcat
                .prepare_model_list_request()
                .unwrap()
                .relative_uri()
                .to_string(),
            "/openai/v1/models"
        );

        // Bind ChatGPT's client-version query and parse its Codex manifest envelope.
        let chatgpt = crate::provider::ProviderAdapter::for_kind(ProviderKind::ChatGpt);
        assert_eq!(
            chatgpt
                .prepare_model_list_request()
                .unwrap()
                .relative_uri()
                .to_string(),
            "/models?client_version=0.146.0"
        );
        assert_eq!(
            chatgpt.model_list_ids(&json!({
                "models": [
                    {"slug": "gpt-5.6-sol"},
                    {"slug": "gpt-5.6-luna"}
                ]
            })),
            Some(vec!["gpt-5.6-sol".to_owned(), "gpt-5.6-luna".to_owned()])
        );
        assert!(
            chatgpt
                .model_list_ids(&json!({"data": [{"id": "gpt-5.6-sol"}]}))
                .is_none()
        );
    }
}
