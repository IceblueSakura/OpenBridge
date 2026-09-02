//! Errors raised while preparing an explicit bounded upstream probe.
//!
//! These errors describe probe admission failures only; probe observations remain in the
//! report types owned by the parent module.

use thiserror::Error;

/// Invalid administrative probe selection rejected before credential access or egress.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProbeSelectionError {
    /// The explicit model ID is empty, padded, oversized, or contains a control character.
    #[error(
        "--model must be a non-empty, unpadded upstream model id of at most 256 bytes without control characters"
    )]
    InvalidUpstreamModel,
    /// The model override cannot affect any selected discovery or Generation request.
    #[error("--model requires models discovery or a Generation case")]
    UnusedUpstreamModel,
    /// The risk opt-in cannot affect any selected streaming Generation request.
    #[error("--allow-unbounded-streaming-output requires streaming Generation")]
    UnusedUnboundedStreamingOutput,
    /// The selected case only exists on the Responses wire.
    #[error("the selected case requires --protocol responses")]
    ResponsesOnlyCase,
    /// The admin-authored overrides cannot affect any selected Generation request.
    #[error("--prompt/--schema/--schema-name require a Generation case")]
    UnusedGenerationOverrides,
    /// The selected case binds its oracle to the fixed prompt, so no prompt override applies.
    #[error("--prompt cannot replace the fixed prompt of the selected tool case")]
    UnsupportedPromptOverride,
    /// Only JSON Schema cases expose a response-format schema to override.
    #[error("--schema/--schema-name require a json-schema or json-schema-strict case")]
    UnsupportedSchemaOverride,
    /// The admin-authored prompt is empty or oversized.
    #[error("--prompt must be a non-empty value of at most 4096 bytes")]
    InvalidCustomPrompt,
    /// The admin-authored schema is oversized, not a JSON object, or required by a schema name.
    #[error(
        "--schema must be a JSON object of at most 8192 bytes, and is required when --schema-name is provided"
    )]
    InvalidCustomSchema,
    /// The admin-authored schema name is empty, oversized, padded, or contains a separator.
    #[error(
        "--schema-name must be a non-empty value of at most 64 bytes without whitespace or control characters"
    )]
    InvalidCustomSchemaName,
}

/// Probe preparation failed.
#[derive(Debug, Error)]
pub enum ProbeError {
    /// The caller supplied an invalid model or unit-case selection.
    #[error(transparent)]
    InvalidSelection(#[from] ProbeSelectionError),
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
    /// No enabled Generation Target is available for the selected Provider.
    #[error("provider '{provider}' has no enabled Generation target")]
    ProviderGenerationTargetUnavailable {
        /// Stable Provider slug.
        provider: String,
    },
    /// An explicit Target belongs to another Provider or task.
    #[error(
        "configured upstream target '{upstream_target}' is not an enabled Generation target for provider '{provider}'"
    )]
    ProviderTargetMismatch {
        /// Stable Provider slug.
        provider: String,
        /// Explicit mismatched Target ID.
        upstream_target: String,
    },
    /// More than one trusted deployment is available and requires explicit disambiguation.
    #[error(
        "provider '{provider}' has multiple trusted Generation deployments; select --target from: {targets}"
    )]
    AmbiguousProviderTarget {
        /// Stable Provider slug.
        provider: String,
        /// Safe comma-separated internal Target IDs.
        targets: String,
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
