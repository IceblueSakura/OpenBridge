//! Errors raised while preparing an explicit basic upstream probe.
//!
//! These errors describe probe admission failures only; probe observations remain in the
//! report types owned by the parent module.

use thiserror::Error;

/// Probe preparation failed.
#[derive(Debug, Error)]
pub enum ProbeError {
    /// The requested Upstream Target is not registered.
    #[error("configured upstream target '{upstream_target}' does not exist")]
    UnknownUpstreamTarget {
        /// Missing internal target ID.
        upstream_target: String,
    },
    /// The selected target is registered but disabled for all executable paths.
    #[error("configured upstream target '{upstream_target}' is disabled")]
    DisabledUpstreamTarget {
        /// Disabled internal target ID.
        upstream_target: String,
    },
    /// The OAuth2 probe entry point was used with a non-ChatGPT target.
    #[error("OAuth2 probe is not supported for configured upstream target '{upstream_target}'")]
    OAuth2UnsupportedTarget {
        /// Target ID that is outside the ChatGPT OAuth2 probe boundary.
        upstream_target: String,
    },
    /// The trusted credential source cannot provide the required secret.
    #[error("upstream credentials are unavailable for probe")]
    CredentialUnavailable,
    /// The adapter cannot build authentication headers for the probe.
    #[error("provider authentication could not be prepared for probe")]
    AuthenticationPreparation,
}
