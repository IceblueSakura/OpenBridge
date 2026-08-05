//! Downstream native protocols and request value objects that passed basic HTTP checks.
//!
//! `ApiRequest` stores JSON bytes for the protocol fixed by RoutePlan: a Native Route preserves the
//! canonical downstream body, while a Bridged Route stores the target-protocol body produced by
//! `BridgePlan`. The Provider adapter then supplies the real model, applies target wire mappings,
//! and binds the upstream relative request.

use bytes::Bytes;

/// Stable identity for one client-visible API operation.
///
/// Chat Completions and Responses retain [`ApiProtocol`] because they can participate in the
/// generation Protocol Bridge. Embeddings is an independent JSON operation and never becomes an
/// `ApiProtocol` variant.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OperationKind {
    /// Creates a Chat Completions response.
    ChatCompletions,
    /// Creates a Responses response.
    Responses,
    /// Creates one or more embedding vectors.
    EmbeddingsCreate,
}

impl OperationKind {
    /// Returns the generation protocol for operations that can participate in the Protocol Bridge.
    pub const fn api_protocol(self) -> Option<ApiProtocol> {
        match self {
            Self::ChatCompletions => Some(ApiProtocol::ChatCompletions),
            Self::Responses => Some(ApiProtocol::Responses),
            Self::EmbeddingsCreate => None,
        }
    }

    /// Returns the stable low-cardinality operation name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
            Self::EmbeddingsCreate => "embeddings_create",
        }
    }
}

/// Native protocol used by an OpenAI-compatible downstream request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiProtocol {
    /// OpenAI Chat Completions protocol.
    ChatCompletions,
    /// OpenAI Responses protocol.
    Responses,
}

impl ApiProtocol {
    /// Returns the API operation represented by this generation protocol.
    pub const fn operation(self) -> OperationKind {
        match self {
            Self::ChatCompletions => OperationKind::ChatCompletions,
            Self::Responses => OperationKind::Responses,
        }
    }
}

/// Request that passed basic HTTP checks and can be handed to a Provider adapter.
#[derive(Clone, Debug)]
pub struct ApiRequest {
    protocol: ApiProtocol,
    body: Bytes,
}

/// Embeddings Create request that passed endpoint-specific analysis and fixed-interface preflight.
///
/// This type remains separate from [`ApiRequest`] because Embeddings cannot enter the generation
/// Protocol Bridge or inherit Chat/Responses semantics.
#[derive(Clone, Debug)]
pub struct EmbeddingRequest {
    body: Bytes,
}

impl EmbeddingRequest {
    /// Creates a preflighted Native Embeddings request from its preserved JSON bytes.
    pub(crate) fn new(body: Bytes) -> Self {
        Self { body }
    }

    /// Returns the original downstream JSON bytes before trusted model rewriting.
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}

impl ApiRequest {
    /// Creates a request view with a protocol identifier.
    pub fn new(protocol: ApiProtocol, body: Bytes) -> Self {
        Self { protocol, body }
    }

    /// Returns the request's native protocol.
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// Returns request JSON bytes for the current execution protocol.
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
