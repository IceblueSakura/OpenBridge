//! Errors raised while validating and loading upstream credential configuration.
//!
//! Document errors remain separate from file-location failures, while nested credential-store
//! and OAuth2 errors preserve their source context without exposing secret values.

use std::{io, path::PathBuf};

use thiserror::Error;

use crate::{
    credential::CredentialStoreError, oauth2_credentials::OAuth2CredentialManagerError,
    provider::ProviderKind,
};

/// Upstream credential TOML parsing, validation, or registry binding failed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum UpstreamCredentialConfigError {
    /// The TOML document cannot be parsed as upstream credential configuration.
    #[error("invalid upstream credential configuration")]
    Parse,
    /// The document declares a schema version unsupported by this runtime.
    #[error("unsupported upstream credential configuration schema version {actual}")]
    UnsupportedSchema {
        /// Schema version declared by the document.
        actual: u32,
    },
    /// The credential pool ID is blank.
    #[error("upstream credential pool id must not be blank")]
    BlankPoolId,
    /// The same pool ID appears more than once.
    #[error("upstream credential pool '{id}' is configured more than once")]
    DuplicatePoolId {
        /// Duplicated pool ID.
        id: String,
    },
    /// A binding selects both supported credential source fields.
    #[error("upstream credential pool '{id}' selects more than one credential source")]
    ConflictingCredentialSources {
        /// ID of the ambiguous binding.
        id: String,
    },
    /// An OAuth2 auth-file locator is blank.
    #[error("upstream credential pool '{id}' has a blank OAuth2 auth-file locator")]
    BlankAuthJsonFile {
        /// ID of the binding containing the blank locator.
        id: String,
    },
    /// The pool contains a blank API key.
    #[error("upstream credential pool '{id}' contains a blank API key")]
    BlankApiKey {
        /// ID of the pool containing a blank API key.
        id: String,
    },
    /// The pool contains a duplicate API key.
    #[error("upstream credential pool '{id}' contains a duplicate API key")]
    DuplicateApiKey {
        /// ID of the pool containing a duplicate API key.
        id: String,
    },
    /// The TOML declares a pool absent from the compile-time registry.
    #[error("upstream credential configuration contains unknown pool '{id}'")]
    UnknownPool {
        /// Unregistered pool ID.
        id: String,
    },
    /// The configured source variant does not match the compile-time credential kind.
    #[error(
        "upstream credential pool '{id}' uses a credential source incompatible with its registered kind"
    )]
    CredentialSourceKindMismatch {
        /// ID of the mismatched binding.
        id: String,
    },
    /// An API-key-only builder was asked to load an OAuth2 binding.
    #[error("upstream credential pool '{id}' requires OAuth2 manager loading")]
    OAuth2ManagerRequired {
        /// OAuth2 binding ID that cannot enter the API-key-only builder.
        id: String,
    },
    /// More than one auth file is bound to the same OAuth2 Provider.
    #[error("OAuth2 Provider {provider:?} is configured more than once")]
    DuplicateOAuth2Provider {
        /// Provider that received multiple auth-file bindings.
        provider: ProviderKind,
    },
    /// The requested Provider has no configured OAuth2 auth-file binding.
    #[error("upstream credential configuration has no OAuth2 binding for Provider {provider:?}")]
    MissingOAuth2Provider {
        /// Provider selected by the explicit administrative operation.
        provider: ProviderKind,
    },
    /// A compile-time pool required by the caller has no configured API key.
    #[error("upstream credential configuration is missing required pool '{id}'")]
    MissingPool {
        /// Missing pool ID.
        id: String,
    },
    /// An API key cannot be added to the purpose-bound credential store.
    #[error("upstream credential configuration could not populate the credential store")]
    Credential(#[source] CredentialStoreError),
    /// An OAuth2 auth file could not populate the immutable manager.
    #[error("upstream credential configuration could not populate the OAuth2 credential manager")]
    OAuth2Credential(#[source] OAuth2CredentialManagerError),
}

/// Reading or validating the upstream credential configuration file failed.
#[derive(Debug, Error)]
pub enum UpstreamCredentialConfigFileError {
    /// The upstream credential configuration file cannot be read.
    #[error("failed to read upstream credential configuration '{path}'")]
    Read {
        /// Path of the file that could not be read.
        path: PathBuf,
        #[source]
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// The file was read, but its contents failed upstream credential validation.
    #[error("upstream credential configuration validation failed")]
    Invalid(#[source] UpstreamCredentialConfigError),
}
