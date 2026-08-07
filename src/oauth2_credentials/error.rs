//! Shared value-free errors for the managed OAuth2 credential lifecycle.

use thiserror::Error;

/// Value-free failure returned while validating or loading an OAuth2 credential document.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OAuth2CredentialManagerError {
    /// The configured Provider has no managed auth-file adapter.
    #[error("OAuth2 credential Provider is unsupported")]
    UnsupportedProvider,
    /// The same Provider was added more than once.
    #[error("OAuth2 credential Provider is configured more than once")]
    DuplicateProvider,
    /// The configured auth file could not be read.
    #[error("OAuth2 auth file could not be read")]
    Read,
    /// The file is not a valid ChatGPT OAuth JSON document.
    #[error("OAuth2 auth file is invalid")]
    InvalidDocument,
    /// The file does not explicitly select managed ChatGPT authentication.
    #[error("OAuth2 auth mode is unsupported")]
    UnsupportedAuthMode,
    /// Another credential type appears beside the ChatGPT OAuth bundle.
    #[error("OAuth2 auth file contains conflicting credential material")]
    ConflictingAuthMaterial,
    /// The document contains no OAuth2 token bundle.
    #[error("OAuth2 token bundle is missing")]
    MissingTokens,
    /// The ID token is blank or cannot be decoded.
    #[error("OAuth2 identity token is invalid")]
    InvalidIdToken,
    /// The access token is blank or cannot be decoded.
    #[error("OAuth2 access token is invalid")]
    InvalidAccessToken,
    /// The access-token JWT has no absolute expiry.
    #[error("OAuth2 access token expiry is missing")]
    MissingAccessTokenExpiry,
    /// The access token is expired where a newly exchanged bundle requires future validity.
    #[error("OAuth2 access token is expired")]
    ExpiredAccessToken,
    /// The refresh token required by the lifecycle is blank.
    #[error("OAuth2 refresh token is invalid")]
    InvalidRefreshToken,
    /// The token bundle has no selected account/workspace binding.
    #[error("OAuth2 account binding is missing")]
    MissingAccountBinding,
    /// The selected account conflicts with the ID-token account claim.
    #[error("OAuth2 account binding is inconsistent")]
    AccountBindingMismatch,
    /// The optional refresh timestamp is present but blank or cannot be produced.
    #[error("OAuth2 last refresh timestamp is invalid")]
    InvalidLastRefresh,
}
