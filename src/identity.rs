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

use serde::Deserialize;
use thiserror::Error;

use crate::credential::{CredentialStore, CredentialStoreBuilder, CredentialStoreError};

const USERS_SCHEMA_VERSION: u32 = 1;
const MIN_API_KEY_BYTES: usize = 32;

/// 认证成功后的稳定下游用户身份。
#[derive(Debug, Eq, PartialEq)]
pub struct User {
    id: String,
    name: String,
}

impl User {
    /// 返回稳定的下游用户 id。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回用于展示或审计的用户名称。
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// 运行期间只读的下游用户注册表。
pub struct UserRegistry {
    users: Vec<Arc<User>>,
}

impl UserRegistry {
    /// 通过统一 CredentialStore 认证 API Key，并返回对应的稳定用户身份。
    pub fn authenticate(
        &self,
        credentials: &CredentialStore,
        candidate: &str,
    ) -> Option<Arc<User>> {
        // 让 Store 完成用途隔离和 constant-time Key 匹配，再按非敏感用户 ID 查询身份。
        let user_id = credentials.authenticate_downstream(candidate)?;
        self.users.iter().find(|user| user.id() == user_id).cloned()
    }

    /// 枚举所有已启用用户，但不暴露任何 API key。
    pub fn users(&self) -> impl Iterator<Item = &User> {
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

/// 已解析的下游用户元数据与待合并 credential 构造器。
///
/// 调用方必须在启动阶段继续加入上游 credential，再构造唯一的运行时 Store。
pub struct UserConfiguration {
    users: UserRegistry,
    credentials: CredentialStoreBuilder,
}

impl UserConfiguration {
    /// 解析并校验用户 TOML，把身份元数据与 secret 分离到各自所有者。
    pub fn from_toml(document: &str) -> Result<Self, UserRegistryError> {
        // 解析文档并确认用户配置 schema。
        let raw: RawUsers = toml::from_str(document).map_err(|_| UserRegistryError::Parse)?;
        if raw.schema_version != USERS_SCHEMA_VERSION {
            return Err(UserRegistryError::UnsupportedSchema {
                actual: raw.schema_version,
            });
        }

        // 校验用户元数据，并把全部 Key 交给 Store builder 检查唯一性。
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
        // 拒绝没有任何可用于认证的用户的注册表。
        if users.is_empty() {
            return Err(UserRegistryError::NoEnabledUsers);
        }
        Ok(Self {
            users: UserRegistry { users },
            credentials,
        })
    }

    /// 返回已启用的非敏感用户注册表。
    pub fn users(&self) -> &UserRegistry {
        &self.users
    }

    /// 拆分用户注册表与 credential 构造器，供 composition root 完成启动快照。
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

/// 将 credential builder 错误收敛为用户配置层的稳定错误。
fn map_credential_error(error: CredentialStoreError) -> UserRegistryError {
    // 将 credential builder 的细粒度错误收敛为不泄露 secret 的用户配置错误。
    match error {
        CredentialStoreError::DuplicateDownstreamSecret => UserRegistryError::DuplicateApiKey,
        CredentialStoreError::DuplicateId => UserRegistryError::DuplicateApiKey,
        CredentialStoreError::DuplicateUpstreamSecret
        | CredentialStoreError::InvalidPoolIdentity
        | CredentialStoreError::StatefulPoolHasMultipleMembers => UserRegistryError::Parse,
        CredentialStoreError::InvalidMetadata => UserRegistryError::Parse,
        CredentialStoreError::Unavailable => UserRegistryError::Parse,
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
/// 下游用户 TOML 解析或校验失败。
pub enum UserRegistryError {
    /// TOML 文档无法解析为用户配置。
    #[error("invalid user configuration")]
    Parse,
    /// 文档声明了当前运行时不支持的 schema 版本。
    #[error("unsupported user configuration schema version {actual}")]
    UnsupportedSchema {
        /// 文档中声明的 schema 版本。
        actual: u32,
    },
    /// 用户 id 为空。
    #[error("user id must not be blank")]
    BlankUserId,
    /// 用户 id 重复。
    #[error("user id '{id}' is configured more than once")]
    DuplicateUserId {
        /// 重复的用户 id。
        id: String,
    },
    /// 用户名称为空。
    #[error("user '{id}' name must not be blank")]
    BlankUserName {
        /// 名称为空的用户 id。
        id: String,
    },
    /// API key 长度不足安全下限。
    #[error("user '{id}' API key must contain at least 32 bytes")]
    ApiKeyTooShort {
        /// API key 不合规的用户 id。
        id: String,
    },
    /// 同一个 API key 被多个用户复用。
    #[error("the same downstream API key is configured for more than one user")]
    DuplicateApiKey,
    /// 配置中没有任何已启用用户。
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
    /// 创建一个由调用方指定路径的用户配置定位器。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// 返回用户配置文件路径。
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// 读取并解析用户配置文件。
    pub fn load(&self) -> Result<UserConfiguration, UserConfigFileError> {
        // 读取配置文件并保留路径上下文。
        let document = fs::read_to_string(&self.0).map_err(|source| UserConfigFileError::Read {
            path: self.0.clone(),
            source,
        })?;
        // 校验内容并转换为不可变用户注册表。
        UserConfiguration::from_toml(&document).map_err(UserConfigFileError::Invalid)
    }
}

#[derive(Debug, Error)]
/// 用户配置文件读取或内容校验失败。
pub enum UserConfigFileError {
    /// 无法读取用户配置文件。
    #[error("failed to read user configuration '{path}'")]
    Read {
        /// 读取失败的文件路径。
        path: PathBuf,
        #[source]
        /// 底层文件系统错误。
        source: io::Error,
    },
    /// 文件已读取，但内容未通过用户注册表校验。
    #[error("user configuration validation failed")]
    Invalid(#[source] UserRegistryError),
}
