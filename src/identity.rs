//! Startup-loaded registry of downstream users and API keys.
//!
//! The user file is read once before listening starts. The registry remains immutable during
//! runtime; adding users, disabling users, or replacing API keys requires a file change and restart.

use std::{
    collections::BTreeSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Deserialize;
use thiserror::Error;

use crate::credential::{CredentialStore, CredentialStoreBuilder, CredentialStoreError};

const USERS_SCHEMA_VERSION: u32 = 1;
const MIN_API_KEY_BYTES: usize = 32;

/// Stable downstream-user identity after successful authentication.
#[derive(Debug, Eq, PartialEq)]
pub struct User {
    id: String,
    name: String,
}

impl User {
    /// Returns the stable downstream-user ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the user name used for display or audit.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Read-only downstream-user registry for the runtime.
pub struct UserRegistry {
    users: Vec<Arc<User>>,
}

impl UserRegistry {
    /// Authenticates an API key through the shared CredentialStore and returns its stable user identity.
    pub fn authenticate(
        &self,
        credentials: &CredentialStore,
        candidate: &str,
    ) -> Option<Arc<User>> {
        // Let the Store enforce purpose isolation and constant-time key matching, then look up the identity by non-sensitive user ID.
        let user_id = credentials.authenticate_downstream(candidate)?;
        self.users.iter().find(|user| user.id() == user_id).cloned()
    }

    /// Enumerates all enabled users without exposing any API key.
    pub fn users(&self) -> impl Iterator<Item=&User> {
        self.users.iter().map(Arc::as_ref)
    }
}

impl fmt::Debug for UserRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserRegistry")
            .field("enabled_users", &self.users.len())
            .finish()
    }
}

/// Parsed downstream-user metadata and a credential builder awaiting merge.
///
/// The caller must add upstream credentials during startup before building the single runtime Store.
pub struct UserConfiguration {
    users: UserRegistry,
    credentials: CredentialStoreBuilder,
}

impl UserConfiguration {
    /// Parses and validates user TOML, separating identity metadata and secrets into their owners.
    pub fn from_toml(document: &str) -> Result<Self, UserRegistryError> {
        // Parse the document and verify the user-configuration schema.
        let raw: RawUsers = toml::from_str(document).map_err(|_| UserRegistryError::Parse)?;
        if raw.schema_version != USERS_SCHEMA_VERSION {
            return Err(UserRegistryError::UnsupportedSchema {
                actual: raw.schema_version,
            });
        }

        // Validate user metadata and let the Store builder check all key uniqueness.
        let mut ids = BTreeSet::new();
        let mut users = Vec::new();
        let mut credentials = CredentialStoreBuilder::new();
        for raw_user in raw.users {
            let id = raw_user.id.trim();
            if id.is_empty() {
                return Err(UserRegistryError::BlankUserId);
            }
            if !ids.insert(id.to_owned()) {
                return Err(UserRegistryError::DuplicateUserId { id: id.to_owned() });
            }
            if raw_user.name.trim().is_empty() {
                return Err(UserRegistryError::BlankUserName { id: id.to_owned() });
            }
            if raw_user.api_key.len() < MIN_API_KEY_BYTES {
                return Err(UserRegistryError::ApiKeyTooShort { id: id.to_owned() });
            }
            credentials
                .insert_downstream(
                    id,
                    secrecy::SecretString::from(raw_user.api_key),
                    raw_user.enabled,
                )
                .map_err(map_credential_error)?;
            if raw_user.enabled {
                users.push(Arc::new(User {
                    id: id.to_owned(),
                    name: raw_user.name.trim().to_owned(),
                }));
            }
        }
        // Reject a registry with no user available for authentication.
        if users.is_empty() {
            return Err(UserRegistryError::NoEnabledUsers);
        }
        Ok(Self {
            users: UserRegistry { users },
            credentials,
        })
    }

    /// Returns the enabled non-sensitive user registry.
    pub fn users(&self) -> &UserRegistry {
        &self.users
    }

    /// Splits the user registry and credential builder so the composition root can complete the startup snapshot.
    pub fn into_parts(self) -> (UserRegistry, CredentialStoreBuilder) {
        (self.users, self.credentials)
    }
}

impl fmt::Debug for UserConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserConfiguration")
            .field("users", &self.users)
            .field("credentials", &self.credentials)
            .finish()
    }
}

/// Collapses credential-builder errors into stable user-configuration errors.
fn map_credential_error(error: CredentialStoreError) -> UserRegistryError {
    // Collapse detailed credential-builder errors into user-configuration errors that reveal no secrets.
    match error {
        CredentialStoreError::DuplicateDownstreamSecret => UserRegistryError::DuplicateApiKey,
        CredentialStoreError::DuplicateId => UserRegistryError::DuplicateApiKey,
        CredentialStoreError::DuplicateUpstreamSecret
        | CredentialStoreError::InvalidPoolIdentity
        | CredentialStoreError::StatefulPoolHasMultipleMembers => UserRegistryError::Parse,
        CredentialStoreError::InvalidMetadata | CredentialStoreError::InvalidOAuthContext => {
            UserRegistryError::Parse
        }
        CredentialStoreError::Unavailable => UserRegistryError::Parse,
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
/// Downstream-user TOML parsing or validation failed.
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUsers {
    schema_version: u32,
    users: Vec<RawUser>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUser {
    id: String,
    name: String,
    api_key: String,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

const fn enabled_by_default() -> bool {
    true
}

/// Path to the user configuration file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserConfigPath(PathBuf);

impl UserConfigPath {
    /// Creates a user-configuration locator for a caller-supplied path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the user configuration file path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Reads and parses the user configuration file.
    pub fn load(&self) -> Result<UserConfiguration, UserConfigFileError> {
        // Read the configuration file while preserving path context.
        let document = fs::read_to_string(&self.0).map_err(|source| UserConfigFileError::Read {
            path: self.0.clone(),
            source,
        })?;
        // Validate the content and convert it into an immutable user registry.
        UserConfiguration::from_toml(&document).map_err(UserConfigFileError::Invalid)
    }
}

#[derive(Debug, Error)]
/// User configuration file reading or content validation failed.
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
