//! 启动时加载的私有上游 credential pool 配置。
//!
//! 文件只保存编译期 pool id 对应的 API key，不允许配置 Provider、credential kind、endpoint 或路由。
//! 配置在网络监听或 probe 请求前读取一次，并把 secret 移交给不可变 [`CredentialStore`](crate::credential::CredentialStore)。

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
};

use secrecy::SecretString;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{
    credential::{
        CredentialMetadata, CredentialSource, CredentialStoreBuilder, CredentialStoreError,
    },
    registry::RuntimeRegistry,
};

const UPSTREAM_CREDENTIALS_SCHEMA_VERSION: u32 = 1;

/// 已解析并完成文档内校验的上游 credential 配置。
pub struct UpstreamCredentialConfiguration {
    pools: BTreeMap<String, Vec<String>>,
}

impl UpstreamCredentialConfiguration {
    /// 解析并校验 upstream credential TOML。
    pub fn from_toml(document: &str) -> Result<Self, UpstreamCredentialConfigError> {
        // 解析文档并确认 schema 版本。
        let raw: RawUpstreamCredentials =
            toml::from_str(document).map_err(|_| UpstreamCredentialConfigError::Parse)?;
        if raw.schema_version != UPSTREAM_CREDENTIALS_SCHEMA_VERSION {
            return Err(UpstreamCredentialConfigError::UnsupportedSchema {
                actual: raw.schema_version,
            });
        }

        // 校验 pool 标识和 API key 集合，再按稳定 pool id 建立索引。
        let mut pools = BTreeMap::new();
        for raw_pool in raw.credential_pools {
            let id = raw_pool.id.trim();
            if id.is_empty() {
                return Err(UpstreamCredentialConfigError::BlankPoolId);
            }
            if raw_pool.api_keys.is_empty() {
                return Err(UpstreamCredentialConfigError::EmptyPool { id: id.to_owned() });
            }
            if raw_pool.api_keys.iter().any(|key| key.trim().is_empty()) {
                return Err(UpstreamCredentialConfigError::BlankApiKey { id: id.to_owned() });
            }
            if contains_duplicate_secret(&raw_pool.api_keys) {
                return Err(UpstreamCredentialConfigError::DuplicateApiKey { id: id.to_owned() });
            }
            if pools.insert(id.to_owned(), raw_pool.api_keys).is_some() {
                return Err(UpstreamCredentialConfigError::DuplicatePoolId { id: id.to_owned() });
            }
        }
        Ok(Self { pools })
    }

    /// 只把调用方要求的 pool 加入新的 credential builder。
    pub fn into_builder_for<'a>(
        self,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<CredentialStoreBuilder, UpstreamCredentialConfigError> {
        let mut builder = CredentialStoreBuilder::new();
        self.load_into_for(&mut builder, registry, required_pool_ids)?;
        Ok(builder)
    }

    /// 校验配置与编译期注册表一致，并把指定 pool 的 secret 移交给现有 builder。
    pub fn load_into_for<'a>(
        mut self,
        builder: &mut CredentialStoreBuilder,
        registry: &RuntimeRegistry,
        required_pool_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), UpstreamCredentialConfigError> {
        // 拒绝未由代码注册的 pool，避免拼写错误或陈旧 secret 被静默忽略。
        for configured_pool_id in self.pools.keys() {
            if registry.credential_pool(configured_pool_id).is_none() {
                return Err(UpstreamCredentialConfigError::UnknownPool {
                    id: configured_pool_id.clone(),
                });
            }
        }

        // 去重并解析调用方实际需要的编译期 pool。
        let required = required_pool_ids
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        for pool_id in &required {
            if registry.credential_pool(pool_id).is_none() {
                return Err(UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                });
            }
            if !self.pools.contains_key(pool_id) {
                return Err(UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                });
            }
        }

        // 完整校验成功后才移动 secret，避免错误返回时留下部分上游 pool。
        for pool_id in required {
            let pool = registry.credential_pool(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::UnknownPool {
                    id: pool_id.clone(),
                }
            })?;
            let api_keys = self.pools.remove(&pool_id).ok_or_else(|| {
                UpstreamCredentialConfigError::MissingPool {
                    id: pool_id.clone(),
                }
            })?;

            // 按 TOML 数组顺序生成稳定成员 ID，并把 secret 移入唯一运行时 Store builder。
            for (index, api_key) in api_keys.into_iter().enumerate() {
                builder
                    .insert_upstream_member(
                        pool.provider(),
                        pool.id(),
                        format!("{}#{}", pool.id(), index + 1),
                        SecretString::from(api_key),
                        CredentialMetadata::upstream(
                            pool.kind(),
                            CredentialSource::UpstreamConfiguration,
                        ),
                    )
                    .map_err(UpstreamCredentialConfigError::Credential)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for UpstreamCredentialConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamCredentialConfiguration")
            .field("credential_pools", &self.pools.len())
            .finish()
    }
}

/// 使用 constant-time equality 检查同一 pool 内是否存在重复 secret。
fn contains_duplicate_secret(secrets: &[String]) -> bool {
    secrets.iter().enumerate().any(|(index, candidate)| {
        secrets[..index].iter().any(|expected| {
            candidate.len() == expected.len()
                && bool::from(candidate.as_bytes().ct_eq(expected.as_bytes()))
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUpstreamCredentials {
    schema_version: u32,
    credential_pools: Vec<RawCredentialPool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCredentialPool {
    id: String,
    api_keys: Vec<String>,
}

/// 上游 credential TOML 解析、校验或 registry 绑定失败。
#[derive(Debug, Error, Eq, PartialEq)]
pub enum UpstreamCredentialConfigError {
    /// TOML 文档无法解析为上游 credential 配置。
    #[error("invalid upstream credential configuration")]
    Parse,
    /// 文档声明了当前运行时不支持的 schema 版本。
    #[error("unsupported upstream credential configuration schema version {actual}")]
    UnsupportedSchema {
        /// 文档中声明的 schema 版本。
        actual: u32,
    },
    /// credential pool id 为空。
    #[error("upstream credential pool id must not be blank")]
    BlankPoolId,
    /// 同一个 pool id 出现多次。
    #[error("upstream credential pool '{id}' is configured more than once")]
    DuplicatePoolId {
        /// 重复的 pool id。
        id: String,
    },
    /// pool 没有任何 API key。
    #[error("upstream credential pool '{id}' must contain at least one API key")]
    EmptyPool {
        /// 空 pool 的 id。
        id: String,
    },
    /// pool 包含空白 API key。
    #[error("upstream credential pool '{id}' contains a blank API key")]
    BlankApiKey {
        /// 包含空白 API key 的 pool id。
        id: String,
    },
    /// pool 重复配置相同 API key。
    #[error("upstream credential pool '{id}' contains a duplicate API key")]
    DuplicateApiKey {
        /// 包含重复 API key 的 pool id。
        id: String,
    },
    /// TOML 声明了编译期注册表中不存在的 pool。
    #[error("upstream credential configuration contains unknown pool '{id}'")]
    UnknownPool {
        /// 未注册的 pool id。
        id: String,
    },
    /// 调用方要求的编译期 pool 没有配置 API key。
    #[error("upstream credential configuration is missing required pool '{id}'")]
    MissingPool {
        /// 缺失的 pool id。
        id: String,
    },
    /// API key 无法加入用途受限的 credential Store。
    #[error("upstream credential configuration could not populate the credential store")]
    Credential(#[source] CredentialStoreError),
}

/// 上游 credential 配置文件路径。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialConfigPath(PathBuf);

impl UpstreamCredentialConfigPath {
    /// 创建一个由 bootstrap 指定路径的 upstream credential 配置定位器。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// 返回 upstream credential 配置文件路径。
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// 读取并解析 upstream credential 配置文件。
    pub fn load(
        &self,
    ) -> Result<UpstreamCredentialConfiguration, UpstreamCredentialConfigFileError> {
        // 读取私有配置文件并保留路径上下文。
        let document = fs::read_to_string(&self.0).map_err(|source| {
            UpstreamCredentialConfigFileError::Read {
                path: self.0.clone(),
                source,
            }
        })?;
        // 校验内容并返回不公开 secret 的配置对象。
        UpstreamCredentialConfiguration::from_toml(&document)
            .map_err(UpstreamCredentialConfigFileError::Invalid)
    }
}

/// 上游 credential 配置文件读取或内容校验失败。
#[derive(Debug, Error)]
pub enum UpstreamCredentialConfigFileError {
    /// 无法读取 upstream credential 配置文件。
    #[error("failed to read upstream credential configuration '{path}'")]
    Read {
        /// 读取失败的文件路径。
        path: PathBuf,
        #[source]
        /// 底层文件系统错误。
        source: io::Error,
    },
    /// 文件已读取，但内容未通过 upstream credential 校验。
    #[error("upstream credential configuration validation failed")]
    Invalid(#[source] UpstreamCredentialConfigError),
}
