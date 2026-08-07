//! Credential-store failure contract.
//!
//! Errors describe only coarse validation or availability outcomes. They never retain credential
//! values, file locators, or Provider authentication material.

use thiserror::Error;

/// Credential snapshot construction or purpose-restricted lookup failed.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CredentialStoreError {
    /// A credential ID is duplicated for the same purpose.
    #[error("credential id is configured more than once")]
    DuplicateId,
    /// The same downstream API key is reused by multiple users.
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateDownstreamSecret,
    /// The same upstream secret is configured more than once in a pool.
    #[error("the same upstream secret is configured more than once in a credential pool")]
    DuplicateUpstreamSecret,
    /// A TargetBound API with continuation enabled references a multi-member pool.
    #[error("state-bound upstream APIs require a single-member credential pool")]
    StatefulPoolHasMultipleMembers,
    /// A pool or member non-sensitive ID is blank.
    #[error("credential pool and member ids must not be blank")]
    InvalidPoolIdentity,
    /// Credential metadata does not match its purpose or has an invalid generation.
    #[error("credential metadata is invalid")]
    InvalidMetadata,
    /// OAuth material is missing its account binding or known expiration.
    #[error("OAuth credential context is invalid")]
    InvalidOAuthContext,
    /// The secret is missing, empty, or does not match the requested purpose/binding.
    #[error("credential is unavailable")]
    Unavailable,
}
