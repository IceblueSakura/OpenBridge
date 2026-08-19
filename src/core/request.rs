//! Downstream native protocols and request value objects that passed basic HTTP checks.
//!
//! `ApiRequest` stores JSON bytes for the protocol fixed by RoutePlan: a Native Route preserves the
//! canonical downstream body, while a Generation Bridge Route stores the target-protocol body produced by
//! `BridgePlan`. The Provider adapter then supplies the real model, applies target wire mappings,
//! and binds the upstream relative request.

use std::fmt;

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
    /// Number of variants in the closed operation kernel.
    pub(crate) const COUNT: usize = 3;

    /// Returns the deterministic slot used by operation-indexed internal sets.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::ChatCompletions => 0,
            Self::Responses => 1,
            Self::EmbeddingsCreate => 2,
        }
    }

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

impl fmt::Display for OperationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
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

/// Closed conversion direction supported by the Generation Protocol Bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationBridgeDirection {
    /// Converts a downstream Chat Completions request into an upstream Responses request.
    ChatToResponses,
    /// Converts a downstream Responses request into an upstream Chat Completions request.
    ResponsesToChat,
}

impl GenerationBridgeDirection {
    /// Returns the downstream protocol accepted by this direction.
    pub const fn downstream_protocol(self) -> ApiProtocol {
        match self {
            Self::ChatToResponses => ApiProtocol::ChatCompletions,
            Self::ResponsesToChat => ApiProtocol::Responses,
        }
    }

    /// Returns the upstream protocol produced by this direction.
    pub const fn upstream_protocol(self) -> ApiProtocol {
        match self {
            Self::ChatToResponses => ApiProtocol::Responses,
            Self::ResponsesToChat => ApiProtocol::ChatCompletions,
        }
    }

    /// Resolves one supported direction from distinct generation protocols.
    pub const fn from_protocols(downstream: ApiProtocol, upstream: ApiProtocol) -> Option<Self> {
        match (downstream, upstream) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => Some(Self::ChatToResponses),
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => Some(Self::ResponsesToChat),
            _ => None,
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
