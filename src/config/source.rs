//! bootstrap 配置文件定位与加载。

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{BootstrapError, BootstrapPolicy, load_bootstrap};

/// 默认 bootstrap 配置路径。
pub const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";

/// 用于定位 bootstrap 配置文件的值对象。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapPath(PathBuf);

impl BootstrapPath {
    /// 创建一个由调用方指定路径的配置定位器。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// 环境变量只选择 bootstrap 文件位置，不能注册或修改 Provider。
    pub fn from_environment() -> Self {
        Self::new(
            env::var("OPENBRIDGE_BOOTSTRAP_CONFIG")
                .unwrap_or_else(|_| DEFAULT_BOOTSTRAP_PATH.to_owned()),
        )
    }

    /// 返回配置文件路径。
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// 读取并解析 bootstrap 文件。
    pub fn load(&self) -> Result<BootstrapPolicy, BootstrapFileError> {
        let document = fs::read_to_string(&self.0).map_err(|source| BootstrapFileError::Read {
            path: self.0.clone(),
            source,
        })?;
        load_bootstrap(&document).map_err(BootstrapFileError::Invalid)
    }
}

impl Default for BootstrapPath {
    fn default() -> Self {
        Self::new(DEFAULT_BOOTSTRAP_PATH)
    }
}

/// bootstrap 文件读取或内容校验失败。
#[derive(Debug, Error)]
pub enum BootstrapFileError {
    #[error("failed to read bootstrap configuration '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("bootstrap configuration validation failed")]
    Invalid(#[source] BootstrapError),
}
