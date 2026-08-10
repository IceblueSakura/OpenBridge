//! Process-level bootstrap configuration.
//!
//! Providers, Models, Upstream Targets, Upstream APIs, Routes, Public Models, endpoints, and
//! credential bindings are defined by the code registry; bootstrap carries listen settings,
//! ChatGPT startup instructions, resource limits, and shared HTTP client policy.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;

mod document;
mod error;
mod parser;
mod source;

pub use error::BootstrapConfigError;
pub use parser::parse_bootstrap_config;
pub use source::{BootstrapConfigFileError, BootstrapConfigPath};

const BOOTSTRAP_SCHEMA_VERSION: u32 = 2;

/// Immutable process configuration parsed during startup.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BootstrapConfig {
    listen: SocketAddr,
    users_file: PathBuf,
    upstream_credentials_file: PathBuf,
    chatgpt_instructions: Option<String>,
    limits: RuntimeLimits,
    http_client: HttpClientConfig,
    otlp_http_trace_export: Option<OtlpHttpExportConfig>,
    otlp_http_metrics_export: Option<OtlpHttpExportConfig>,
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

    /// Returns the optional startup-owned instructions used only by active ChatGPT Targets.
    pub fn chatgpt_instructions(&self) -> Option<&str> {
        self.chatgpt_instructions.as_deref()
    }

    /// Returns independent runtime limits for request, replay, JSON-response, and SSE bodies.
    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    /// Returns the shared upstream HTTP client policy.
    pub fn http_client(&self) -> &HttpClientConfig {
        &self.http_client
    }

    /// Returns the optional startup-only OTLP/HTTP trace exporter policy.
    pub fn otlp_http_trace_export(&self) -> Option<&OtlpHttpExportConfig> {
        self.otlp_http_trace_export.as_ref()
    }

    /// Returns the optional startup-only OTLP/HTTP metrics exporter policy.
    pub fn otlp_http_metrics_export(&self) -> Option<&OtlpHttpExportConfig> {
        self.otlp_http_metrics_export.as_ref()
    }
}

/// Validated process policy for one startup-owned OTLP/HTTP signal exporter.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OtlpHttpExportConfig {
    endpoint: Url,
}

impl OtlpHttpExportConfig {
    /// Returns the collector base URL to which the exporter appends its fixed signal path.
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
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
