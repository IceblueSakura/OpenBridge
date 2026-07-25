//! 进程级 bootstrap 配置。
//!
//! Provider、Model、Deployment、Alias、endpoint 和 credential binding 均由代码注册表
//! 定义；bootstrap 只承载监听、资源限制和共享 HTTP client 策略。

use std::{net::SocketAddr, time::Duration};

use thiserror::Error;

mod document;
mod source;

use document::RawBootstrap;

pub use source::{BootstrapFileError, BootstrapPath};

const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("invalid bootstrap configuration")]
    Parse,
    #[error("unsupported bootstrap schema version {actual}")]
    UnsupportedSchema { actual: u32 },
    #[error("listen address '{listen}' must be a valid loopback socket address")]
    NonLoopbackListen { listen: String },
    #[error("runtime limit '{name}' must be greater than zero")]
    InvalidLimit { name: &'static str },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BootstrapPolicy {
    listen: SocketAddr,
    limits: RuntimeLimits,
    upstream_policy: UpstreamPolicy,
}

impl BootstrapPolicy {
    pub fn listen(&self) -> SocketAddr {
        self.listen
    }

    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    pub fn upstream_policy(&self) -> &UpstreamPolicy {
        &self.upstream_policy
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuntimeLimits {
    max_request_body_bytes: usize,
    max_sse_event_bytes: usize,
}

impl RuntimeLimits {
    pub fn max_request_body_bytes(&self) -> usize {
        self.max_request_body_bytes
    }

    pub fn max_sse_event_bytes(&self) -> usize {
        self.max_sse_event_bytes
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UpstreamPolicy {
    connect_timeout: Duration,
    pool_idle_timeout: Duration,
    pool_max_idle_per_host: usize,
}

impl UpstreamPolicy {
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub fn pool_idle_timeout(&self) -> Duration {
        self.pool_idle_timeout
    }

    pub fn pool_max_idle_per_host(&self) -> usize {
        self.pool_max_idle_per_host
    }
}

pub fn load_bootstrap(document: &str) -> Result<BootstrapPolicy, BootstrapError> {
    let raw: RawBootstrap = toml::from_str(document).map_err(|_| BootstrapError::Parse)?;
    if raw.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        return Err(BootstrapError::UnsupportedSchema {
            actual: raw.schema_version,
        });
    }
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
    let listen = raw
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| BootstrapError::NonLoopbackListen {
            listen: raw.listen.clone(),
        })?;

    Ok(BootstrapPolicy {
        listen,
        limits: RuntimeLimits {
            max_request_body_bytes: raw.max_request_body_bytes,
            max_sse_event_bytes: raw.max_sse_event_bytes,
        },
        upstream_policy: UpstreamPolicy {
            connect_timeout: Duration::from_millis(raw.upstream_connect_timeout_ms),
            pool_idle_timeout: Duration::from_millis(raw.upstream_pool_idle_timeout_ms),
            pool_max_idle_per_host: raw.upstream_pool_max_idle_per_host,
        },
    })
}

fn validate_nonzero(
    name: &'static str,
    value: impl Copy + PartialEq + From<u8>,
) -> Result<(), BootstrapError> {
    if value == 0.into() {
        Err(BootstrapError::InvalidLimit { name })
    } else {
        Ok(())
    }
}
