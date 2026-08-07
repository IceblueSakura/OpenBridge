//! Bounded OAuth2 recovery for one pre-commit upstream authorization failure.
//!
//! The helper keeps the one-replay rule and generation rejection beside the OAuth lifecycle call;
//! forwarding remains responsible only for deciding when an upstream 401 reaches this boundary.

use axum::response::Response;
use http::StatusCode;

use crate::{
    ingress::{response::api_error, state::GatewayState},
    oauth2_credentials::{OAuth2CredentialLease, OAuth2RefreshOutcome},
    provider::ProviderKind,
};

/// Recovers one newer OAuth2 lease after a pre-commit 401, allowing at most one replay.
pub(super) async fn recover_after_unauthorized(
    state: &GatewayState,
    provider: ProviderKind,
    pool_id: &str,
    lease: &OAuth2CredentialLease,
    replayed: &mut bool,
) -> Result<OAuth2CredentialLease, Response> {
    // Reject a second 401 for the already replayed generation instead of looping indefinitely.
    let rejected_generation = lease.generation();
    if *replayed {
        state
            .oauth2_credentials()
            .reject_replayed_generation(provider, rejected_generation);
        return Err(oauth2_authentication_error());
    }

    // Reload an externally rotated bundle or refresh the rejected generation once.
    match state
        .oauth2_credentials()
        .recover_after_unauthorized(provider, rejected_generation)
        .await
    {
        OAuth2RefreshOutcome::Current { .. } | OAuth2RefreshOutcome::Refreshed { .. } => {}
        OAuth2RefreshOutcome::NotConfigured
        | OAuth2RefreshOutcome::Backoff { .. }
        | OAuth2RefreshOutcome::ReauthRequired { .. }
        | OAuth2RefreshOutcome::Ambiguous { .. } => {
            return Err(oauth2_authentication_error());
        }
    }

    // Require a different guarded generation from the expected compile-time pool before replay.
    let next_lease = match state.oauth2_credentials().lease_for_request(provider).await {
        Ok(lease) if lease.pool_id() == pool_id && lease.generation() != rejected_generation => {
            lease
        }
        Ok(_) | Err(_) => return Err(oauth2_authentication_error()),
    };
    *replayed = true;
    Ok(next_lease)
}

/// Returns one value-free failure for unavailable or rejected OAuth2 request credentials.
pub(super) fn oauth2_authentication_error() -> Response {
    api_error(
        StatusCode::BAD_GATEWAY,
        "upstream_authentication_error",
        "Upstream OAuth2 credentials require explicit authentication",
    )
}
