//! Operation-owned typed views over one Provider's shared adapter.
//!
//! A Provider operation is selected from the static definition before request preparation. The
//! typed views retain shared header, authentication, and status policy while preventing request
//! protocol fields from selecting another operation implicitly.

use http::{HeaderMap, StatusCode};

use crate::{
    core::{
        ApiProtocol, ApiRequest, EmbeddingRequest, ImagesRequest, OperationKind,
        ProviderOperationCapabilities,
    },
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
    /// Native Images Generations wire policy.
    ImagesGenerations(ImagesProviderAdapter),
}

impl ProviderAdapter {
    /// Selects one operation descriptor from the Provider's closed wire surface.
    pub(super) fn operation_adapter(
        self,
        operation: OperationKind,
        capabilities: ProviderOperationCapabilities,
    ) -> Option<ProviderOperationAdapter> {
        match operation {
            OperationKind::ChatCompletions => Some(ProviderOperationAdapter::Generation(
                GenerationProviderAdapter::new(
                    self,
                    ApiProtocol::ChatCompletions,
                    self.openai_compatible()
                        .generation_path(ApiProtocol::ChatCompletions)?,
                    capabilities,
                ),
            )),
            OperationKind::Responses => Some(ProviderOperationAdapter::Generation(
                GenerationProviderAdapter::new(
                    self,
                    ApiProtocol::Responses,
                    self.openai_compatible()
                        .generation_path(ApiProtocol::Responses)?,
                    capabilities,
                ),
            )),
            OperationKind::EmbeddingsCreate => Some(ProviderOperationAdapter::Embeddings(
                EmbeddingsProviderAdapter::new(
                    self,
                    self.openai_compatible().embeddings_path()?,
                    capabilities,
                ),
            )),
            OperationKind::ImagesGenerations => Some(ProviderOperationAdapter::ImagesGenerations(
                ImagesProviderAdapter::new(
                    self,
                    self.openai_compatible().images_path()?,
                    capabilities,
                ),
            )),
        }
    }

    /// Builds one routed Generation request through the selected Provider wire primitive.
    fn prepare_routed_generation_request(
        self,
        protocol: ApiProtocol,
        path: &'static str,
        request: &ApiRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.openai_compatible()
            .prepare_routed_request(protocol, path, request, upstream_api)
    }

    /// Builds one routed Embeddings request through the selected Provider wire primitive.
    fn prepare_routed_embeddings_request(
        self,
        path: &'static str,
        request: &EmbeddingRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.openai_compatible()
            .prepare_embedding_routed_request(path, request, upstream_api)
    }

    /// Builds one routed Images request through the selected Provider wire primitive.
    fn prepare_routed_images_request(
        self,
        path: &'static str,
        request: &ImagesRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.openai_compatible()
            .prepare_images_routed_request(path, request, upstream_api)
    }

    /// Classifies one fully framed Generation SSE event through the selected terminal primitive.
    fn classify_generation_sse_event(
        self,
        protocol: ApiProtocol,
        event: SseEvent,
    ) -> ClassifiedSseEvent {
        self.openai_compatible().classify_sse_event(protocol, event)
    }

    /// Applies the selected Generation SSE media primitive.
    fn recognizes_generation_sse_response(
        self,
        protocol: ApiProtocol,
        headers: &HeaderMap,
    ) -> bool {
        self.openai_compatible()
            .recognizes_sse_response(protocol, headers)
    }
}

/// Provider adapter bound to exactly one Generation protocol.
#[derive(Clone, Copy)]
pub struct GenerationProviderAdapter {
    provider: ProviderAdapter,
    protocol: ApiProtocol,
    path: &'static str,
    capabilities: ProviderOperationCapabilities,
}

impl GenerationProviderAdapter {
    pub(super) const fn new(
        provider: ProviderAdapter,
        protocol: ApiProtocol,
        path: &'static str,
        capabilities: ProviderOperationCapabilities,
    ) -> Self {
        Self {
            provider,
            protocol,
            path,
            capabilities,
        }
    }

    /// Returns the fixed protocol selected from the Provider definition.
    pub const fn protocol(self) -> ApiProtocol {
        self.protocol
    }

    /// Returns the capability ceiling co-selected with this operation descriptor.
    pub const fn capabilities(self) -> ProviderOperationCapabilities {
        self.capabilities
    }

    /// Builds a routed request only when the request and Upstream API match this operation.
    pub fn prepare_routed_request(
        self,
        request: &ApiRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.require_request_protocol(request)?;
        if upstream_api.operation() != self.protocol.operation() {
            return Err(AdapterError::UnsupportedProtocol);
        }
        self.provider.prepare_routed_generation_request(
            self.protocol,
            self.path,
            request,
            upstream_api,
        )
    }

    /// Builds one fixed administrative probe without borrowing a registered model's API rules.
    pub(crate) fn prepare_probe_request(
        self,
        request: &ApiRequest,
        upstream_model: &str,
        streaming: bool,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        self.require_request_protocol(request)?;
        self.provider.openai_compatible().prepare_probe_request(
            self.protocol,
            self.path,
            request,
            upstream_model,
            streaming,
        )
    }

    /// Assembles trusted routed headers and authentication for this operation.
    pub(crate) fn build_outbound_headers(
        self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
        upstream_api: &UpstreamApi,
    ) -> Result<HeaderMap, AdapterError> {
        self.provider.build_routed_outbound_headers(
            credential,
            downstream_headers,
            self.protocol.operation(),
            upstream_api.upstream_model(),
        )
    }

    /// Maps an upstream status through the shared Provider policy.
    pub fn classify_status(self, status: StatusCode) -> StatusClassification {
        self.provider.classify_status(status)
    }

    /// Returns whether response headers satisfy this operation's SSE media profile.
    pub(crate) fn recognizes_sse_response(self, headers: &HeaderMap) -> bool {
        self.provider
            .recognizes_generation_sse_response(self.protocol, headers)
    }

    /// Classifies one framed SSE event through this operation's terminal profile.
    pub fn classify_sse_event(self, event: SseEvent) -> Result<ClassifiedSseEvent, AdapterError> {
        Ok(self
            .provider
            .classify_generation_sse_event(self.protocol, event))
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
    path: &'static str,
    capabilities: ProviderOperationCapabilities,
}

impl EmbeddingsProviderAdapter {
    pub(super) const fn new(
        provider: ProviderAdapter,
        path: &'static str,
        capabilities: ProviderOperationCapabilities,
    ) -> Self {
        Self {
            provider,
            path,
            capabilities,
        }
    }

    /// Returns the capability ceiling co-selected with this operation descriptor.
    pub const fn capabilities(self) -> ProviderOperationCapabilities {
        self.capabilities
    }

    /// Builds a routed Embeddings request only for the selected operation.
    pub fn prepare_routed_request(
        self,
        request: &EmbeddingRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        if upstream_api.operation() != OperationKind::EmbeddingsCreate {
            return Err(AdapterError::UnsupportedProtocol);
        }
        self.provider
            .prepare_routed_embeddings_request(self.path, request, upstream_api)
    }

    /// Assembles trusted routed headers and authentication for this operation.
    pub(crate) fn build_outbound_headers(
        self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
        upstream_api: &UpstreamApi,
    ) -> Result<HeaderMap, AdapterError> {
        self.provider.build_routed_outbound_headers(
            credential,
            downstream_headers,
            OperationKind::EmbeddingsCreate,
            upstream_api.upstream_model(),
        )
    }

    /// Maps an upstream status through the shared Provider policy.
    pub fn classify_status(self, status: StatusCode) -> StatusClassification {
        self.provider.classify_status(status)
    }

    pub(crate) const fn provider(self) -> ProviderAdapter {
        self.provider
    }
}

/// Provider adapter bound to the Native Images Generations operation.
#[derive(Clone, Copy)]
pub struct ImagesProviderAdapter {
    provider: ProviderAdapter,
    path: &'static str,
    capabilities: ProviderOperationCapabilities,
}

impl ImagesProviderAdapter {
    pub(super) const fn new(
        provider: ProviderAdapter,
        path: &'static str,
        capabilities: ProviderOperationCapabilities,
    ) -> Self {
        Self {
            provider,
            path,
            capabilities,
        }
    }

    /// Returns the capability ceiling co-selected with this operation descriptor.
    pub const fn capabilities(self) -> ProviderOperationCapabilities {
        self.capabilities
    }

    /// Builds a routed Images request only for the selected operation.
    pub fn prepare_routed_request(
        self,
        request: &ImagesRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        if upstream_api.operation() != OperationKind::ImagesGenerations {
            return Err(AdapterError::UnsupportedProtocol);
        }
        self.provider
            .prepare_routed_images_request(self.path, request, upstream_api)
    }

    /// Assembles trusted routed headers and authentication for this operation.
    pub(crate) fn build_outbound_headers(
        self,
        credential: &UpstreamCredential<'_>,
        downstream_headers: &HeaderMap,
        upstream_api: &UpstreamApi,
    ) -> Result<HeaderMap, AdapterError> {
        self.provider.build_routed_outbound_headers(
            credential,
            downstream_headers,
            OperationKind::ImagesGenerations,
            upstream_api.upstream_model(),
        )
    }

    /// Maps an upstream status through the shared Provider policy.
    pub fn classify_status(self, status: StatusCode) -> StatusClassification {
        self.provider.classify_status(status)
    }
}
