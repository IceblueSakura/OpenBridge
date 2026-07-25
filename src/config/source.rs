//! bootstrap 配置文件定位与加载。

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{BootstrapError, BootstrapPolicy, load_bootstrap};

pub const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapPath(PathBuf);

impl BootstrapPath {
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

    pub fn path(&self) -> &Path {
        &self.0
    }

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
