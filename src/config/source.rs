//! 配置文件定位与加载。
//!
//! 服务进程和管理 CLI 必须使用同一来源规则，避免它们对默认路径、环境覆盖或错误边界
//! 产生分歧。它只读取 bootstrap 与 routes 这一对 owner-controlled 文档；当前 CLI 不接受
//! 单次调用提供的路径参数。

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{ConfigError, RegistrySnapshot, load_registry};

pub const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";
pub const DEFAULT_ROUTES_PATH: &str = "config/routes.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPaths {
    bootstrap: PathBuf,
    routes: PathBuf,
}

impl ConfigPaths {
    pub fn new(bootstrap: impl Into<PathBuf>, routes: impl Into<PathBuf>) -> Self {
        Self {
            bootstrap: bootstrap.into(),
            routes: routes.into(),
        }
    }

    /// 读取进程启动前设置的路径覆盖；环境变量只影响文件定位，不可改变解析语义。
    pub fn from_environment() -> Self {
        Self::new(
            env::var("OPENBRIDGE_BOOTSTRAP_CONFIG")
                .unwrap_or_else(|_| DEFAULT_BOOTSTRAP_PATH.to_owned()),
            env::var("OPENBRIDGE_ROUTES_CONFIG").unwrap_or_else(|_| DEFAULT_ROUTES_PATH.to_owned()),
        )
    }

    pub fn bootstrap(&self) -> &Path {
        &self.bootstrap
    }

    pub fn routes(&self) -> &Path {
        &self.routes
    }

    pub fn load(&self) -> Result<RegistrySnapshot, ConfigFileError> {
        let bootstrap =
            fs::read_to_string(&self.bootstrap).map_err(|source| ConfigFileError::Read {
                document: "bootstrap",
                path: self.bootstrap.clone(),
                source,
            })?;
        let routes = fs::read_to_string(&self.routes).map_err(|source| ConfigFileError::Read {
            document: "routes",
            path: self.routes.clone(),
            source,
        })?;
        load_registry(&bootstrap, &routes).map_err(ConfigFileError::Invalid)
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new(DEFAULT_BOOTSTRAP_PATH, DEFAULT_ROUTES_PATH)
    }
}

#[derive(Debug, Error)]
pub enum ConfigFileError {
    #[error("failed to read {document} configuration '{path}'")]
    Read {
        document: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("configuration validation failed")]
    Invalid(#[source] ConfigError),
}
