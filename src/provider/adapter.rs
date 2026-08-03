//! Closed request and response dispatch for compiled Provider adapters.

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest},
    credential::UpstreamCredential,
    providers::openai_compatible::OpenAiCompatibleAdapter,
    transport::sse::SseEvent,
};

use super::{
    ClassifiedSseEvent, ProviderContract, ProviderKind, SafeHeaders, SensitiveHeaders,
    StatusClassification,
};

/// Failure reported by a Provider adapter during request, authentication, or response handling.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// The request protocol is outside the adapter's supported scope.
    #[error("request protocol is not supported by this provider adapter")]
    UnsupportedProtocol,
    /// The credential Provider does not match the adapter.
    #[error("credential provider does not match the provider adapter")]
    CredentialProviderMismatch,
    /// The credential kind is outside the Provider's static contract.
    #[error("credential kind is not supported by the provider adapter")]
    CredentialKindMismatch,
    /// A sensitive header was incorrectly placed in the ordinary-header set.
    #[error("sensitive header cannot be emitted as a regular provider header")]
    SensitiveHeaderInSafeSet,
    /// The request body cannot be parsed or rewritten as a valid JSON object.
    #[error("request body could not be transformed by the provider adapter")]
    InvalidRequestBody,
    /// The credential cannot be encoded as a valid HTTP header.
    #[error("provider authentication material cannot be encoded as an HTTP header")]
    InvalidAuthenticationHeader,
}

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
}

impl PreparedUpstreamRequest {
    /// Creates an adapter request without a bound endpoint origin.
    pub(crate) fn new(method: Method, relative_uri: Uri, body: Bytes) -> Self {
        Self {
            method,
            relative_uri,
            body,
        }
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

    /// Selects an adapter from the Provider kind in the registry.
    pub fn for_kind(kind: ProviderKind) -> Self {
        kind.definition().adapter()
    }

    /// Returns the OpenAI-compatible profile held by this closed dispatch.
    fn openai_compatible(&self) -> OpenAiCompatibleAdapter {
        match self.implementation {
            ProviderAdapterImplementation::OpenAiCompatible(adapter) => adapter,
        }
    }

    /// Returns the adapter's static Provider contract.
    pub fn contract(&self) -> &'static ProviderContract {
        self.openai_compatible().contract()
    }

    /// Merges the ordinary-header hook with the final Provider-sensitive authentication header.
    pub(crate) fn build_outbound_headers(
        &self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
    ) -> Result<HeaderMap, AdapterError> {
        // Build Provider base headers and run the request hook, which may write only SafeHeaders.
        let mut safe_headers = self.prepare_headers()?;
        self.apply_request_header_hook(downstream_headers, &mut safe_headers)?;
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

    /// Upstream model-discovery request fixed by the compile-time adapter.
    ///
    /// This request is for explicit administrative probes only; it does not implement downstream
    /// `/v1/models`, which always exposes OpenBridge Public Models.
    pub(crate) fn prepare_model_list_request(&self) -> PreparedUpstreamRequest {
        self.openai_compatible().prepare_model_list_request()
    }

    /// Builds a relative upstream request from the execution protocol and upstream model ID in RoutePlan.
    pub fn prepare_request(
        &self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.openai_compatible()
            .prepare_request(request, upstream_model)
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

    /// Returns whether a fully framed SSE event is terminal or failed.
    pub fn classify_sse_event(
        &self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> Result<ClassifiedSseEvent, AdapterError> {
        Ok(self.openai_compatible().classify_sse_event(protocol, event))
    }

    /// Maps an upstream HTTP status to a coarse error and retry boundary.
    pub fn classify_status(&self, status: StatusCode) -> StatusClassification {
        self.openai_compatible().classify_status(status)
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderValue, header::AUTHORIZATION};
    use secrecy::SecretString;

    use super::*;

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
}
