//! Operation-owned typed views over one Provider's shared adapter.
//!
//! A Provider operation is selected from the static definition before request preparation. The
//! typed views retain shared header, authentication, and status policy while preventing request
//! protocol fields from selecting another operation implicitly.

use http::{HeaderMap, StatusCode};

use crate::{
    core::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind},
    credential::UpstreamCredential,
    registry::UpstreamApi,
    transport::sse::SseEvent,
};

use super::{
    AdapterError, ClassifiedSseEvent, PreparedUpstreamRequest, ProviderAdapter,
    StatusClassification,
};

/// Closed operation adapter selected from a Provider definition.
#[derive(Clone, Copy)]
pub enum ProviderOperationAdapter {
    /// Chat Completions or Responses wire policy with one fixed protocol.
    Generation(GenerationProviderAdapter),
    /// Native Embeddings Create wire policy.
    Embeddings(EmbeddingsProviderAdapter),
}

/// Provider adapter bound to exactly one Generation protocol.
#[derive(Clone, Copy)]
pub struct GenerationProviderAdapter {
    provider: ProviderAdapter,
    protocol: ApiProtocol,
}

impl GenerationProviderAdapter {
    pub(super) const fn new(provider: ProviderAdapter, protocol: ApiProtocol) -> Self {
        Self { provider, protocol }
    }

    /// Returns the fixed protocol selected from the Provider definition.
    pub const fn protocol(self) -> ApiProtocol {
        self.protocol
    }

    /// Builds a relative request only when its protocol matches this selected operation.
    pub fn prepare_request(
        self,
        request: &ApiRequest,
        upstream_model: &str,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.require_request_protocol(request)?;
        self.provider.prepare_request(request, upstream_model)
    }

    /// Builds a routed request only when the request and Upstream API match this operation.
    pub(crate) fn prepare_routed_request(
        self,
        request: &ApiRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.require_request_protocol(request)?;
        if upstream_api.operation() != self.protocol.operation() {
            return Err(AdapterError::UnsupportedProtocol);
        }
        self.provider.prepare_routed_request(request, upstream_api)
    }

    /// Assembles shared Provider headers and authentication for this operation.
    pub(crate) fn build_outbound_headers(
        self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
    ) -> Result<HeaderMap, AdapterError> {
        self.provider
            .build_outbound_headers(credential, downstream_headers)
    }

    /// Maps an upstream status through the shared Provider policy.
    pub fn classify_status(self, status: StatusCode) -> StatusClassification {
        self.provider.classify_status(status)
    }

    /// Returns whether response headers satisfy this operation's SSE media profile.
    pub(crate) fn recognizes_sse_response(self, headers: &HeaderMap) -> bool {
        self.provider
            .recognizes_sse_response(self.protocol, headers)
    }

    /// Classifies one framed SSE event through this operation's terminal profile.
    pub fn classify_sse_event(self, event: SseEvent) -> Result<ClassifiedSseEvent, AdapterError> {
        self.provider.classify_sse_event(self.protocol, event)
    }

    pub(crate) const fn provider(self) -> ProviderAdapter {
        self.provider
    }

    fn require_request_protocol(self, request: &ApiRequest) -> Result<(), AdapterError> {
        if request.protocol() == self.protocol {
            Ok(())
        } else {
            Err(AdapterError::UnsupportedProtocol)
        }
    }
}

/// Provider adapter bound to the Native Embeddings Create operation.
#[derive(Clone, Copy)]
pub struct EmbeddingsProviderAdapter {
    provider: ProviderAdapter,
}

impl EmbeddingsProviderAdapter {
    pub(super) const fn new(provider: ProviderAdapter) -> Self {
        Self { provider }
    }

    /// Builds a routed Embeddings request only for the selected operation.
    pub(crate) fn prepare_routed_request(
        self,
        request: &EmbeddingRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        if upstream_api.operation() != OperationKind::EmbeddingsCreate {
            return Err(AdapterError::UnsupportedProtocol);
        }
        self.provider
            .prepare_embedding_routed_request(request, upstream_api)
    }

    /// Assembles shared Provider headers and authentication for this operation.
    pub(crate) fn build_outbound_headers(
        self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
    ) -> Result<HeaderMap, AdapterError> {
        self.provider
            .build_outbound_headers(credential, downstream_headers)
    }

    /// Maps an upstream status through the shared Provider policy.
    pub fn classify_status(self, status: StatusCode) -> StatusClassification {
        self.provider.classify_status(status)
    }

    pub(crate) const fn provider(self) -> ProviderAdapter {
        self.provider
    }
}
