//! Process-level bootstrap configuration.
//!
//! Providers, Models, Upstream Targets, Upstream APIs, Routes, Public Models, endpoints, and
//! credential bindings are defined by the code registry; bootstrap carries only listen settings,
//! resource limits, and shared HTTP client policy.

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
pub use source::{BootstrapConfigFileError, BootstrapConfigPath};

const BOOTSTRAP_SCHEMA_VERSION: u32 = 2;

/// Bootstrap configuration parsing, version, or security-boundary validation failed.
#[derive(Debug, Error)]
pub enum BootstrapConfigError {
    /// The TOML document could not be parsed as bootstrap configuration.
    #[error("invalid bootstrap configuration")]
    Parse,
    /// The document declares a schema version unsupported by this runtime.
    #[error("unsupported bootstrap schema version {actual}")]
    UnsupportedSchema {
        /// Schema version declared by the document.
        actual: u32,
    },
    /// The listen address is not a loopback socket address.
    #[error("listen address '{listen}' must be a valid loopback socket address")]
    NonLoopbackListen {
        /// Raw address that failed loopback validation.
        listen: String,
    },
    /// A runtime limit is zero and cannot provide a valid boundary.
    #[error("runtime limit '{name}' must be greater than zero")]
    InvalidLimit {
        /// Name of the invalid limit.
        name: &'static str,
    },
    /// The replay eligibility limit exceeds the downstream request hard limit.
    #[error("replay body limit {replay} must not exceed downstream request body limit {request}")]
    ReplayLimitExceedsRequest {
        /// Configured replay eligibility limit in bytes.
        replay: usize,
        /// Configured downstream request hard limit in bytes.
        request: usize,
    },
}

/// Immutable process configuration parsed during startup.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BootstrapConfig {
    listen: SocketAddr,
    users_file: PathBuf,
    upstream_credentials_file: PathBuf,
    limits: RuntimeLimits,
    http_client: HttpClientConfig,
}

impl BootstrapConfig {
    /// Returns the loopback listen address.
    pub fn listen(&self) -> SocketAddr {
        self.listen
    }

    /// Returns the private downstream-user file read at startup.
    pub fn users_file(&self) -> &Path {
        &self.users_file
    }

    /// Returns the private upstream credential file read at startup.
    pub fn upstream_credentials_file(&self) -> &Path {
        &self.upstream_credentials_file
    }

    /// Returns independent runtime limits for request, replay, JSON-response, and SSE bodies.
    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    /// Returns the shared upstream HTTP client policy.
    pub fn http_client(&self) -> &HttpClientConfig {
        &self.http_client
    }
}

/// Independent boundaries for downstream requests, replay eligibility, JSON responses, and SSE events.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuntimeLimits {
    max_request_body_bytes: usize,
    max_json_response_body_bytes: usize,
    max_replay_body_bytes: usize,
    max_sse_event_bytes: usize,
}

impl RuntimeLimits {
    /// Returns the maximum body size allowed for one downstream request.
    pub fn max_request_body_bytes(&self) -> usize {
        self.max_request_body_bytes
    }

    /// Returns the maximum successful non-streaming JSON response size buffered before commit.
    pub fn max_json_response_body_bytes(&self) -> usize {
        self.max_json_response_body_bytes
    }

    /// Returns the largest downstream request body eligible for another upstream attempt.
    pub fn max_replay_body_bytes(&self) -> usize {
        self.max_replay_body_bytes
    }

    /// Returns the maximum size allowed for one SSE event.
    pub fn max_sse_event_bytes(&self) -> usize {
        self.max_sse_event_bytes
    }
}

/// Connection and timeout policy for the shared upstream HTTP client.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HttpClientConfig {
    connect_timeout: Duration,
    pool_idle_timeout: Duration,
    pool_max_idle_per_host: usize,
}

impl HttpClientConfig {
    /// Returns the upstream connection timeout.
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// Returns the idle connection retention time.
    pub fn pool_idle_timeout(&self) -> Duration {
        self.pool_idle_timeout
    }

    /// Returns the maximum idle connections retained per host.
    pub fn pool_max_idle_per_host(&self) -> usize {
        self.pool_max_idle_per_host
    }
}
