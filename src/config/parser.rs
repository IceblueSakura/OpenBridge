//! Parses bootstrap TOML and validates runtime boundaries.

use std::{net::SocketAddr, time::Duration};

use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapConfig, BootstrapConfigError, HttpClientConfig,
    RuntimeLimits, document::RawBootstrap,
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
    // Parse and restrict the listen address to loopback so bootstrap cannot expose the service directly.
    let listen = raw
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| BootstrapConfigError::NonLoopbackListen {
            listen: raw.listen.clone(),
        })?;

    // Convert raw fields into runtime value objects.
    Ok(BootstrapConfig {
        listen,
        users_file: raw.users_file,
        upstream_credentials_file: raw.upstream_credentials_file,
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
