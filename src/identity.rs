//! 启动时加载的下游用户与 API Key 注册表。
//!
//! 用户文件只在监听开始前读取一次。运行期间注册表保持不可变；新增用户、停用用户或更换
//! API Key 都需要修改文件并重启进程。

use std::{
    collections::BTreeSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use thiserror::Error;

const USERS_SCHEMA_VERSION: u32 = 1;
const MIN_API_KEY_BYTES: usize = 32;

/// 认证成功后的稳定下游用户身份。
#[derive(Debug, Eq, PartialEq)]
pub struct User {
    id: String,
    name: String,
}

impl User {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

struct UserEntry {
    user: Arc<User>,
    api_key: SecretString,
}

/// 运行期间只读的下游用户注册表。
pub struct UserRegistry {
    entries: Vec<UserEntry>,
}

impl UserRegistry {
    /// 解析并校验用户 TOML，生成不可变注册表。
    pub fn from_toml(document: &str) -> Result<Self, UserRegistryError> {
        let raw: RawUsers = toml::from_str(document).map_err(|_| UserRegistryError::Parse)?;
        if raw.schema_version != USERS_SCHEMA_VERSION {
            return Err(UserRegistryError::UnsupportedSchema {
                actual: raw.schema_version,
            });
        }

        let mut ids = BTreeSet::new();
        let mut api_keys = BTreeSet::new();
        let mut entries = Vec::new();
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
            if !api_keys.insert(raw_user.api_key.clone()) {
                return Err(UserRegistryError::DuplicateApiKey);
            }
            if raw_user.enabled {
                entries.push(UserEntry {
                    user: Arc::new(User {
                        id: id.to_owned(),
                        name: raw_user.name.trim().to_owned(),
                    }),
                    api_key: SecretString::from(raw_user.api_key),
                });
            }
        }
        if entries.is_empty() {
            return Err(UserRegistryError::NoEnabledUsers);
        }
        Ok(Self { entries })
    }

    /// 使用 constant-time equality 匹配 API Key，并返回对应用户。
    pub fn authenticate(&self, candidate: &str) -> Option<Arc<User>> {
        let candidate = candidate.as_bytes();
        let mut matched = None;
        for entry in &self.entries {
            let expected = entry.api_key.expose_secret().as_bytes();
            if candidate.len() == expected.len() && bool::from(candidate.ct_eq(expected)) {
                matched = Some(entry.user.clone());
            }
        }
        matched
    }

    pub fn users(&self) -> impl Iterator<Item = &User> {
        self.entries.iter().map(|entry| entry.user.as_ref())
    }
}

impl fmt::Debug for UserRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserRegistry")
            .field("enabled_users", &self.entries.len())
            .finish()
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum UserRegistryError {
    #[error("invalid user configuration")]
    Parse,
    #[error("unsupported user configuration schema version {actual}")]
    UnsupportedSchema { actual: u32 },
    #[error("user id must not be blank")]
    BlankUserId,
    #[error("user id '{id}' is configured more than once")]
    DuplicateUserId { id: String },
    #[error("user '{id}' name must not be blank")]
    BlankUserName { id: String },
    #[error("user '{id}' API key must contain at least 32 bytes")]
    ApiKeyTooShort { id: String },
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateApiKey,
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

/// 用户配置文件路径。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserConfigPath(PathBuf);

impl UserConfigPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn load(&self) -> Result<UserRegistry, UserConfigFileError> {
        let document = fs::read_to_string(&self.0).map_err(|source| UserConfigFileError::Read {
            path: self.0.clone(),
            source,
        })?;
        UserRegistry::from_toml(&document).map_err(UserConfigFileError::Invalid)
    }
}

#[derive(Debug, Error)]
pub enum UserConfigFileError {
    #[error("failed to read user configuration '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("user configuration validation failed")]
    Invalid(#[source] UserRegistryError),
}
