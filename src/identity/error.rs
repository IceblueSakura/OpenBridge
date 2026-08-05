//! Errors raised while loading and validating downstream-user configuration.
//!
//! User-registry errors describe TOML and semantic validation, while file errors preserve the
//! private path and source boundary for startup diagnostics.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Downstream-user TOML parsing or validation failed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum UserRegistryError {
    /// The TOML document could not be parsed as user configuration.
    #[error("invalid user configuration")]
    Parse,
    /// The document declares a schema version unsupported by this runtime.
    #[error("unsupported user configuration schema version {actual}")]
    UnsupportedSchema {
        /// Schema version declared by the document.
        actual: u32,
    },
    /// The user ID is blank.
    #[error("user id must not be blank")]
    BlankUserId,
    /// The user ID is duplicated.
    #[error("user id '{id}' is configured more than once")]
    DuplicateUserId {
        /// Duplicated user ID.
        id: String,
    },
    /// The user name is blank.
    #[error("user '{id}' name must not be blank")]
    BlankUserName {
        /// User ID whose name is blank.
        id: String,
    },
    /// The API key is shorter than the security minimum.
    #[error("user '{id}' API key must contain at least 32 bytes")]
    ApiKeyTooShort {
        /// User ID whose API key is invalid.
        id: String,
    },
    /// The same API key is reused by multiple users.
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateApiKey,
    /// The configuration has no enabled user.
    #[error("at least one downstream user must be enabled")]
    NoEnabledUsers,
}

/// User configuration file reading or content validation failed.
#[derive(Debug, Error)]
pub enum UserConfigFileError {
    /// The user configuration file could not be read.
    #[error("failed to read user configuration '{path}'")]
    Read {
        /// Path of the file that could not be read.
        path: PathBuf,
        #[source]
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// The file was read, but its content failed user-registry validation.
    #[error("user configuration validation failed")]
    Invalid(#[source] UserRegistryError),
}
