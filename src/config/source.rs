//! bootstrap 配置文件定位与加载。

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{BootstrapConfig, BootstrapConfigError, parse_bootstrap_config};

/// 默认 bootstrap 配置路径。
pub const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";

/// 可选加载当前目录或其父目录中的 `.env` 文件。
///
/// `dotenvy` 不覆盖已经存在的进程环境变量；未找到文件不是错误，但文件存在且无法解析时
/// 会返回错误，避免服务在部分 credential 被意外加载的状态下启动。
pub fn load_optional_dotenv() -> Result<Option<PathBuf>, dotenvy::Error> {
    match dotenvy::dotenv() {
        Ok(path) => Ok(Some(path)),
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 用于定位 bootstrap 配置文件的值对象。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapConfigPath(PathBuf);

impl BootstrapConfigPath {
    /// 创建一个由调用方指定路径的配置定位器。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// 环境变量只选择启动配置文件位置，不能注册或修改 Provider。
    pub fn from_environment() -> Self {
        Self::new(
            env::var("OPENBRIDGE_CONFIG").unwrap_or_else(|_| DEFAULT_BOOTSTRAP_PATH.to_owned()),
        )
    }

    /// 返回配置文件路径。
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// 读取并解析 bootstrap 文件。
    pub fn load(&self) -> Result<BootstrapConfig, BootstrapConfigFileError> {
        let document =
            fs::read_to_string(&self.0).map_err(|source| BootstrapConfigFileError::Read {
                path: self.0.clone(),
                source,
            })?;
        parse_bootstrap_config(&document).map_err(BootstrapConfigFileError::Invalid)
    }
}

impl Default for BootstrapConfigPath {
    fn default() -> Self {
        Self::new(DEFAULT_BOOTSTRAP_PATH)
    }
}

/// bootstrap 文件读取或内容校验失败。
#[derive(Debug, Error)]
pub enum BootstrapConfigFileError {
    #[error("failed to read bootstrap configuration '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("bootstrap configuration validation failed")]
    Invalid(#[source] BootstrapConfigError),
}
