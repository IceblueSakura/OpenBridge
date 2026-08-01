//! 进程级 bootstrap 配置。
//!
//! Provider、Model、Upstream Target、Upstream API、Route、Public Model、endpoint 和 credential binding 均由代码注册表
//! 定义；bootstrap 只承载监听、资源限制和共享 HTTP client 策略。

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

mod document;
mod parser;
mod source;

pub use parser::parse_bootstrap_config;
pub use source::{BootstrapConfigFileError, BootstrapConfigPath, load_optional_dotenv};

const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;

/// bootstrap 配置解析、版本或安全边界校验失败。
#[derive(Debug, Error)]
pub enum BootstrapConfigError {
    /// TOML 文档无法解析为 bootstrap 配置。
    #[error("invalid bootstrap configuration")]
    Parse,
    /// 文档声明了当前运行时不支持的 schema 版本。
    #[error("unsupported bootstrap schema version {actual}")]
    UnsupportedSchema {
        /// 文档中声明的 schema 版本。
        actual: u32,
    },
    /// 监听地址不是 loopback socket 地址。
    #[error("listen address '{listen}' must be a valid loopback socket address")]
    NonLoopbackListen {
        /// 未通过 loopback 校验的原始地址。
        listen: String,
    },
    /// 某个运行时限制为零，无法提供有效边界。
    #[error("runtime limit '{name}' must be greater than zero")]
    InvalidLimit {
        /// 失败的限制项名称。
        name: &'static str,
    },
}

/// 启动阶段解析出的不可变进程配置。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BootstrapConfig {
    listen: SocketAddr,
    users_file: PathBuf,
    limits: RuntimeLimits,
    http_client: HttpClientConfig,
}

impl BootstrapConfig {
    /// 返回 loopback 监听地址。
    pub fn listen(&self) -> SocketAddr {
        self.listen
    }

    /// 返回启动时读取的私有下游用户文件。
    pub fn users_file(&self) -> &Path {
        &self.users_file
    }

    /// 返回请求体与 SSE event 的运行时限制。
    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    /// 返回共享上游 HTTP client 策略。
    pub fn http_client(&self) -> &HttpClientConfig {
        &self.http_client
    }
}

/// 下游请求和 SSE 事件的内存边界。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuntimeLimits {
    max_request_body_bytes: usize,
    max_sse_event_bytes: usize,
}

impl RuntimeLimits {
    /// 返回单个下游请求允许的最大 body 大小。
    pub fn max_request_body_bytes(&self) -> usize {
        self.max_request_body_bytes
    }

    /// 返回单个 SSE event 允许的最大大小。
    pub fn max_sse_event_bytes(&self) -> usize {
        self.max_sse_event_bytes
    }
}

/// 共享上游 HTTP client 的连接与超时策略。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HttpClientConfig {
    connect_timeout: Duration,
    pool_idle_timeout: Duration,
    pool_max_idle_per_host: usize,
}

impl HttpClientConfig {
    /// 返回建立上游连接的超时时间。
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// 返回连接池中空闲连接的保留时间。
    pub fn pool_idle_timeout(&self) -> Duration {
        self.pool_idle_timeout
    }

    /// 返回每个 host 允许保留的最大空闲连接数。
    pub fn pool_max_idle_per_host(&self) -> usize {
        self.pool_max_idle_per_host
    }
}
