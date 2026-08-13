//! Errors raised while parsing and validating process bootstrap configuration.
//!
//! File-system context is owned by `source`; this module contains only document and semantic
//! validation failures.

use thiserror::Error;

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
    /// The OTLP/HTTP trace endpoint is not a credential-free absolute HTTP base URL.
    #[error(
        "OTLP/HTTP trace endpoint must be an absolute HTTP base URL without credentials, path, query, or fragment"
    )]
    InvalidOtlpHttpTraceEndpoint,
    /// The OTLP/HTTP metrics endpoint is not a credential-free absolute HTTP base URL.
    #[error(
        "OTLP/HTTP metrics endpoint must be an absolute HTTP base URL without credentials, path, query, or fragment"
    )]
    InvalidOtlpHttpMetricsEndpoint,
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
    /// An HTTP content snapshot switch is enabled but no JSONL directory was provided.
    #[error(
        "http_jsonl_directory is required in [logging] when any content snapshot switch is enabled"
    )]
    MissingHttpJsonlDirectory,
    /// The configured JSONL directory is not an absolute filesystem path.
    #[error("http_jsonl_directory must be an absolute path")]
    RelativeHttpJsonlDirectory,
}
