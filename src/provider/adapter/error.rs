//! Errors raised by the closed Provider adapter boundary.
//!
//! Adapter errors cover request transformation, credential matching, safe-header construction,
//! and response classification without exposing provider-specific implementation details.

use thiserror::Error;

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
    /// The credential omits Provider-specific account or routing context.
    #[error("provider authentication context is incomplete")]
    IncompleteAuthenticationContext,
}
