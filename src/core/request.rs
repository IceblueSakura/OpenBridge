//! Downstream native protocols and request value objects that passed basic HTTP checks.
//!
//! `ApiRequest` stores JSON bytes for the protocol fixed by RoutePlan: a Native Route preserves the
//! canonical downstream body, while a Bridged Route stores the target-protocol body produced by
//! `BridgePlan`. The Provider adapter then supplies the real model, applies target wire mappings,
//! and binds the upstream relative request.

use bytes::Bytes;

/// Native protocol used by an OpenAI-compatible downstream request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiProtocol {
    /// OpenAI Chat Completions protocol.
    ChatCompletions,
    /// OpenAI Responses protocol.
    Responses,
}

/// Request that passed basic HTTP checks and can be handed to a Provider adapter.
#[derive(Clone, Debug)]
pub struct ApiRequest {
    protocol: ApiProtocol,
    body: Bytes,
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
