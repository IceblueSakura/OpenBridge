//! Closed request and response dispatch for compiled Provider adapters.

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};

use crate::{
    credential::UpstreamCredential, providers::openai_compatible::OpenAiCompatibleAdapter,
    registry::ReasoningLevelMapping,
};

use super::{ProviderContract, ProviderKind, SafeHeaders, SensitiveHeaders, StatusClassification};

mod error;

pub use error::AdapterError;

/// Upstream request with a selected protocol but no Upstream Target origin bound yet.
///
/// Adapters can produce only relative URIs; transport joins them to an allowlisted configured
/// origin. This is the second boundary preventing a Provider adapter or downstream request from
/// bypassing the egress allowlist.
#[derive(Clone)]
pub struct PreparedUpstreamRequest {
    method: Method,
    relative_uri: Uri,
    body: Bytes,
    reasoning_level_mapping: Option<ReasoningLevelMapping>,
}

impl PreparedUpstreamRequest {
    /// Creates an adapter request without a bound endpoint origin.
    pub(crate) fn new(method: Method, relative_uri: Uri, body: Bytes) -> Self {
        Self {
            method,
            relative_uri,
            body,
            reasoning_level_mapping: None,
        }
    }

    /// Attaches the reasoning-level mapping applied while preparing the Provider wire request.
    pub(crate) fn with_reasoning_level_mapping(
        mut self,
        mapping: Option<ReasoningLevelMapping>,
    ) -> Self {
        self.reasoning_level_mapping = mapping;
        self
    }

    /// Returns the HTTP method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Returns the relative URI without an authority.
    pub fn relative_uri(&self) -> &Uri {
        &self.relative_uri
    }

    /// Returns the rewritten request body.
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// Returns the reasoning-level mapping applied to this prepared wire request.
    pub(crate) fn reasoning_level_mapping(&self) -> Option<&ReasoningLevelMapping> {
        self.reasoning_level_mapping.as_ref()
    }
}

#[derive(Clone, Copy)]
enum ProviderAdapterImplementation {
    OpenAiCompatible(OpenAiCompatibleAdapter),
}

/// Closed dispatch entry point for compiled Provider adapters.
#[derive(Clone, Copy)]
pub struct ProviderAdapter {
    implementation: ProviderAdapterImplementation,
}

impl ProviderAdapter {
    /// Wraps an OpenAI-compatible wire profile as a closed Provider adapter.
    pub(crate) const fn from_openai_compatible(adapter: OpenAiCompatibleAdapter) -> Self {
        Self {
            implementation: ProviderAdapterImplementation::OpenAiCompatible(adapter),
        }
    }

    /// Returns the Provider kind that owns this closed adapter implementation.
    pub(crate) const fn kind(&self) -> ProviderKind {
        match self.implementation {
            ProviderAdapterImplementation::OpenAiCompatible(adapter) => adapter.kind(),
        }
    }

    /// Selects an adapter from the Provider kind in the registry.
    pub fn for_kind(kind: ProviderKind) -> Self {
        kind.definition().adapter()
    }

    /// Returns the OpenAI-compatible profile held by this closed dispatch.
    pub(super) fn openai_compatible(&self) -> OpenAiCompatibleAdapter {
        match self.implementation {
            ProviderAdapterImplementation::OpenAiCompatible(adapter) => adapter,
        }
    }

    /// Returns the adapter's static Provider contract.
    pub fn contract(&self) -> &'static ProviderContract {
        self.kind().contract()
    }

    /// Assembles hook output, fixed Provider headers, and sensitive authentication headers.
    pub(crate) fn build_outbound_headers(
        &self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
    ) -> Result<HeaderMap, AdapterError> {
        // Build Provider base headers and run the request hook, which may write only SafeHeaders.
        let mut safe_headers = self.prepare_headers()?;
        self.apply_request_header_hook(downstream_headers, &mut safe_headers)?;

        // Apply fixed Provider identity after the hook so downstream values cannot override it.
        self.apply_configured_request_headers(&mut safe_headers)?;
        let mut headers = safe_headers.into_inner();

        // Append the credential adapter's sensitive header last so a downstream hook cannot overwrite authentication material.
        self.prepare_auth_headers(credential)?
            .append_to(&mut headers)?;
        Ok(headers)
    }

    /// Runs the Provider's trusted request-header hook.
    ///
    /// The hook may add, replace, transform, or remove ordinary headers; `SafeHeaders` still
    /// rejects authentication, cookie, Host, and proxy-authentication headers.
    pub fn apply_request_header_hook(
        &self,
        downstream_headers: &HeaderMap,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        self.openai_compatible()
            .apply_request_header_hook(downstream_headers, headers)
    }

    /// Applies fixed non-sensitive headers compiled into the Provider definition.
    fn apply_configured_request_headers(
        &self,
        headers: &mut SafeHeaders,
    ) -> Result<(), AdapterError> {
        self.openai_compatible()
            .apply_configured_request_headers(headers)
    }

    /// Upstream model-discovery request fixed by the compile-time adapter.
    ///
    /// This request is for explicit administrative probes only; it does not implement downstream
    /// `/v1/models`, which always exposes OpenBridge Public Models.
    pub(crate) fn prepare_model_list_request(
        &self,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.openai_compatible().prepare_model_list_request()
    }

    /// Extracts model identifiers from the Provider-specific model-list envelope.
    pub(crate) fn model_list_ids(&self, response: &serde_json::Value) -> Option<Vec<String>> {
        self.openai_compatible().model_list_ids(response)
    }

    /// Builds safe request headers without authentication material.
    pub fn prepare_headers(&self) -> Result<SafeHeaders, AdapterError> {
        self.openai_compatible().prepare_headers()
    }

    /// Builds sensitive authentication headers added only before egress.
    pub fn prepare_auth_headers(
        &self,
        credential: &UpstreamCredential<'_>,
    ) -> Result<SensitiveHeaders, AdapterError> {
        self.openai_compatible().prepare_auth_headers(credential)
    }

    /// Maps an upstream HTTP status to a coarse error and retry boundary.
    pub fn classify_status(&self, status: StatusCode) -> StatusClassification {
        self.openai_compatible().classify_status(status)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use http::{
        HeaderName, HeaderValue,
        header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
    };
    use secrecy::SecretString;

    use super::*;
    use crate::{
        core::{ApiProtocol, OperationKind},
        provider::{GenerationProviderAdapter, ProviderOperationAdapter},
    };

    fn generation_adapter(
        provider: ProviderKind,
        protocol: ApiProtocol,
    ) -> GenerationProviderAdapter {
        match provider
            .definition()
            .operation_adapter(protocol.operation())
            .expect("test Provider declares the Generation operation")
        {
            ProviderOperationAdapter::Generation(adapter) => adapter,
            ProviderOperationAdapter::Embeddings(_)
            | ProviderOperationAdapter::ImagesGenerations(_) => {
                panic!("Generation operation selected a non-Generation adapter")
            }
        }
    }

    #[test]
    fn safe_header_debug_output_omits_values() {
        let mut headers = SafeHeaders::default();
        headers
            .insert(
                http::HeaderName::from_static("x-provider-metadata"),
                HeaderValue::from_static("metadata-test-value"),
            )
            .unwrap();

        assert!(!format!("{headers:?}").contains("metadata-test-value"));
    }

    #[test]
    fn safe_headers_reject_authentication_material() {
        let mut headers = SafeHeaders::default();

        let error = headers
            .insert(AUTHORIZATION, HeaderValue::from_static("forbidden"))
            .unwrap_err();

        assert!(matches!(error, AdapterError::SensitiveHeaderInSafeSet));
    }

    #[test]
    fn provider_sse_media_profiles_are_typed_and_fail_closed() {
        let standard = generation_adapter(ProviderKind::OpenAi, ApiProtocol::Responses);
        let chatgpt = generation_adapter(ProviderKind::ChatGpt, ApiProtocol::Responses);

        // Require an explicit SSE media type for ordinary OpenAI-compatible Providers.
        assert!(!standard.recognizes_sse_response(&HeaderMap::new()));

        // Accept ChatGPT's observed omission only for its declared Responses operation.
        assert!(chatgpt.recognizes_sse_response(&HeaderMap::new()));
        assert!(
            ProviderKind::ChatGpt
                .definition()
                .operation_adapter(OperationKind::ChatCompletions)
                .is_none()
        );

        // Apply normal media-type semantics to explicit values for every Provider profile.
        for value in [
            "text/event-stream",
            "Text/Event-Stream",
            "text/event-stream; charset=utf-8",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_str(value).unwrap());
            assert!(
                standard.recognizes_sse_response(&headers),
                "rejected {value}"
            );
        }

        // Reject present wrong, prefixed, or duplicate media types even for ChatGPT.
        for value in ["application/json", "text/event-streaming"] {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_str(value).unwrap());
            assert!(
                !chatgpt.recognizes_sse_response(&headers),
                "accepted {value}"
            );
        }
        let mut duplicate = HeaderMap::new();
        duplicate.append(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        duplicate.append(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        assert!(!chatgpt.recognizes_sse_response(&duplicate));
    }

    #[test]
    fn openai_auth_adapter_builds_the_expected_bearer_value_inside_the_crate_boundary() {
        let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
        let mut credentials = crate::credential::CredentialStoreBuilder::new();
        credentials
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool",
                "pool#1",
                SecretString::from("credential-test-value".to_owned()),
                crate::credential::CredentialMetadata::upstream(
                    crate::provider::CredentialKind::ApiKey,
                    crate::credential::CredentialSource::Programmatic,
                ),
            )
            .unwrap();
        let credentials = credentials.build();
        let credential = credentials
            .upstream_pool(
                ProviderKind::OpenAi,
                "pool",
                crate::provider::CredentialKind::ApiKey,
            )
            .unwrap()
            .remove(0);

        let headers = adapter.prepare_auth_headers(&credential).unwrap();

        assert_eq!(
            headers.expose(AUTHORIZATION),
            Some("Bearer credential-test-value")
        );
    }

    #[test]
    fn chatgpt_auth_adapter_builds_account_bound_sensitive_headers() {
        // Build one complete synthetic ChatGPT OAuth credential with conditional FedRAMP routing.
        let adapter = ProviderAdapter::for_kind(ProviderKind::ChatGpt);
        let mut credentials = crate::credential::CredentialStoreBuilder::new();
        credentials
            .insert_chatgpt_oauth_member(
                "chatgpt-codex",
                "chatgpt-codex#1",
                SecretString::from("access-token-sensitive".to_owned()),
                SecretString::from("account-sensitive".to_owned()),
                true,
                crate::credential::CredentialMetadata::upstream(
                    crate::provider::CredentialKind::OAuth2BearerAccessToken,
                    crate::credential::CredentialSource::Programmatic,
                )
                .with_expires_at(SystemTime::now() + Duration::from_secs(3_600)),
            )
            .unwrap();
        let credentials = credentials.build();
        let credential = credentials
            .upstream_pool(
                ProviderKind::ChatGpt,
                "chatgpt-codex",
                crate::provider::CredentialKind::OAuth2BearerAccessToken,
            )
            .unwrap()
            .remove(0);

        // Verify every authentication and account-routing value stays in the sensitive set.
        let headers = adapter.prepare_auth_headers(&credential).unwrap();
        assert_eq!(
            headers.expose(AUTHORIZATION),
            Some("Bearer access-token-sensitive")
        );
        assert_eq!(
            headers.expose(HeaderName::from_static("chatgpt-account-id")),
            Some("account-sensitive")
        );
        assert_eq!(
            headers.expose(HeaderName::from_static("x-openai-fedramp")),
            Some("true")
        );
        let debug = format!("{headers:?} {credential:?}");
        assert!(!debug.contains("access-token-sensitive"));
        assert!(!debug.contains("account-sensitive"));

        // Assemble the complete request with conflicting downstream identity headers.
        let originator = HeaderName::from_static("originator");
        let mut downstream_headers = HeaderMap::new();
        downstream_headers.insert(
            USER_AGENT,
            HeaderValue::from_static("untrusted-downstream-client/1.0"),
        );
        downstream_headers.insert(
            originator.clone(),
            HeaderValue::from_static("untrusted-downstream-originator"),
        );
        let outbound = adapter
            .build_outbound_headers(&credential, &downstream_headers)
            .unwrap();

        // Require the compile-time ChatGPT identity to override downstream values.
        assert_eq!(
            outbound
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some("codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown")
        );
        assert_eq!(
            outbound
                .get(originator)
                .and_then(|value| value.to_str().ok()),
            Some("codex_cli_rs")
        );
    }
}
