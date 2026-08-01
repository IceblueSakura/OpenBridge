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
mod source;

use document::RawBootstrap;

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

/// 解析并校验 bootstrap TOML。
///
/// 该函数只产生启动配置，不会注册 provider、model、target、upstream API 或 route。
pub fn parse_bootstrap_config(document: &str) -> Result<BootstrapConfig, BootstrapConfigError> {
    // 解析 bootstrap 文档并确认 schema 版本。
    let raw: RawBootstrap = toml::from_str(document).map_err(|_| BootstrapConfigError::Parse)?;
    if raw.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        return Err(BootstrapConfigError::UnsupportedSchema {
            actual: raw.schema_version,
        });
    }
    // 校验所有内存、超时和连接池限制均可提供有效边界。
    validate_nonzero("max_request_body_bytes", raw.max_request_body_bytes)?;
    validate_nonzero("max_sse_event_bytes", raw.max_sse_event_bytes)?;
    validate_nonzero(
        "upstream_connect_timeout_ms",
        raw.upstream_connect_timeout_ms,
    )?;
    validate_nonzero(
        "upstream_pool_idle_timeout_ms",
        raw.upstream_pool_idle_timeout_ms,
    )?;
    validate_nonzero(
        "upstream_pool_max_idle_per_host",
        raw.upstream_pool_max_idle_per_host,
    )?;
    // 解析并限制监听地址为 loopback，避免 bootstrap 直接暴露服务。
    let listen = raw
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| BootstrapConfigError::NonLoopbackListen {
            listen: raw.listen.clone(),
        })?;

    // 将原始字段转换成运行时值对象。
    Ok(BootstrapConfig {
        listen,
        users_file: raw.users_file,
        limits: RuntimeLimits {
            max_request_body_bytes: raw.max_request_body_bytes,
            max_sse_event_bytes: raw.max_sse_event_bytes,
        },
        http_client: HttpClientConfig {
            connect_timeout: Duration::from_millis(raw.upstream_connect_timeout_ms),
            pool_idle_timeout: Duration::from_millis(raw.upstream_pool_idle_timeout_ms),
            pool_max_idle_per_host: raw.upstream_pool_max_idle_per_host,
        },
    })
}

fn validate_nonzero(
    name: &'static str,
    value: impl Copy + PartialEq + From<u8>,
) -> Result<(), BootstrapConfigError> {
    if value == 0.into() {
        Err(BootstrapConfigError::InvalidLimit { name })
    } else {
        Ok(())
    }
}
