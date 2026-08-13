//! Parses bootstrap TOML and validates runtime boundaries.

use std::{net::SocketAddr, time::Duration};
use url::Url;

use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapConfig, BootstrapConfigError, HttpClientConfig,
    HttpLoggingConfig, OtlpHttpExportConfig, RuntimeLimits, document::RawBootstrap,
};

/// Parses and validates bootstrap TOML.
///
/// This function produces startup configuration only; it does not register providers, models,
/// targets, Upstream APIs, or Routes.
pub fn parse_bootstrap_config(document: &str) -> Result<BootstrapConfig, BootstrapConfigError> {
    // Parse the bootstrap document and verify its schema version.
    let raw: RawBootstrap = toml::from_str(document).map_err(|_| BootstrapConfigError::Parse)?;
    if raw.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        return Err(BootstrapConfigError::UnsupportedSchema {
            actual: raw.schema_version,
        });
    }
    // Validate that all memory, timeout, and connection-pool limits are usable.
    validate_nonzero("max_request_body_bytes", raw.max_request_body_bytes)?;
    validate_nonzero(
        "max_json_response_body_bytes",
        raw.max_json_response_body_bytes,
    )?;
    validate_nonzero("max_replay_body_bytes", raw.max_replay_body_bytes)?;
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

    // Keep replay eligibility within the already-enforced downstream request allocation boundary.
    if raw.max_replay_body_bytes > raw.max_request_body_bytes {
        return Err(BootstrapConfigError::ReplayLimitExceedsRequest {
            replay: raw.max_replay_body_bytes,
            request: raw.max_request_body_bytes,
        });
    }

    // Parse and restrict the listen address to loopback so bootstrap cannot expose the service directly.
    let listen = raw
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| BootstrapConfigError::NonLoopbackListen {
            listen: raw.listen.clone(),
        })?;

    // Validate the optional exporter before any listener or telemetry worker can create network egress.
    let (otlp_http_trace_export, otlp_http_metrics_export) = match raw.telemetry {
        Some(telemetry) => {
            let traces = telemetry
                .traces
                .map(|traces| {
                    parse_otlp_http_endpoint(
                        &traces.otlp_http_endpoint,
                        BootstrapConfigError::InvalidOtlpHttpTraceEndpoint,
                    )
                })
                .transpose()?;
            let metrics = telemetry
                .metrics
                .map(|metrics| {
                    parse_otlp_http_endpoint(
                        &metrics.otlp_http_endpoint,
                        BootstrapConfigError::InvalidOtlpHttpMetricsEndpoint,
                    )
                })
                .transpose()?;
            (traces, metrics)
        }
        None => (None, None),
    };

    // Convert the optional local logging switches into a default-off immutable policy.
    let logging = raw.logging.unwrap_or_default();

    // Validate the JSONL directory conditionally: required when any snapshot switch is enabled,
    // and must be an absolute path so startup owns only a predictable filesystem boundary.
    let http_jsonl_directory = if logging.http_jsonl_directory.is_none()
        && (logging.request_headers
            || logging.request_body
            || logging.response_headers
            || logging.response_body)
    {
        return Err(BootstrapConfigError::MissingHttpJsonlDirectory);
    } else {
        logging
            .http_jsonl_directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path)
                } else {
                    Err(BootstrapConfigError::RelativeHttpJsonlDirectory)
                }
            })
            .transpose()?
    };

    // Convert raw fields into runtime value objects.
    Ok(BootstrapConfig {
        listen,
        users_file: raw.users_file,
        upstream_credentials_file: raw.upstream_credentials_file,
        default_instructions: raw.default_instructions,
        limits: RuntimeLimits {
            max_request_body_bytes: raw.max_request_body_bytes,
            max_json_response_body_bytes: raw.max_json_response_body_bytes,
            max_replay_body_bytes: raw.max_replay_body_bytes,
            max_sse_event_bytes: raw.max_sse_event_bytes,
        },
        http_client: HttpClientConfig {
            connect_timeout: Duration::from_millis(raw.upstream_connect_timeout_ms),
            pool_idle_timeout: Duration::from_millis(raw.upstream_pool_idle_timeout_ms),
            pool_max_idle_per_host: raw.upstream_pool_max_idle_per_host,
        },
        http_logging: HttpLoggingConfig {
            http_jsonl_directory,
            request_headers: logging.request_headers,
            request_body: logging.request_body,
            response_headers: logging.response_headers,
            response_body: logging.response_body,
        },
        otlp_http_trace_export,
        otlp_http_metrics_export,
    })
}

/// Parses one startup-owned OTLP/HTTP collector base without accepting embedded routing policy.
fn parse_otlp_http_endpoint(
    endpoint: &str,
    invalid: BootstrapConfigError,
) -> Result<OtlpHttpExportConfig, BootstrapConfigError> {
    // Parse one absolute, plaintext HTTP URL without accepting URL-carried credentials or routing data.
    let endpoint = Url::parse(endpoint)
        .ok()
        .filter(|endpoint| endpoint.scheme() == "http")
        .filter(|endpoint| endpoint.username().is_empty() && endpoint.password().is_none())
        .filter(|endpoint| endpoint.path() == "/")
        .filter(|endpoint| endpoint.query().is_none() && endpoint.fragment().is_none());
    let Some(endpoint) = endpoint else {
        return Err(invalid);
    };

    // Require a concrete host while allowing the bootstrap owner to select local or remote collectors.
    if !endpoint.has_host() {
        return Err(invalid);
    }

    // Preserve only the normalized collector base used by a fixed signal exporter.
    Ok(OtlpHttpExportConfig { endpoint })
}

/// Rejects zero-valued configuration to protect memory, time, and connection-pool boundaries.
fn validate_nonzero(
    name: &'static str,
    value: impl Copy + PartialEq + From<u8>,
) -> Result<(), BootstrapConfigError> {
    // Reject zero values consistently so later memory and time boundaries remain valid.
    if value == 0.into() {
        Err(BootstrapConfigError::InvalidLimit { name })
    } else {
        Ok(())
    }
}
