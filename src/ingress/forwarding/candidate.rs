//! Preparation of one planned generation candidate before the attempt loop.
//!
//! This module resolves only startup-validated Target/API/pool references, borrows the appropriate
//! credential source, and prepares the Provider-relative request. Cross-candidate health policy and
//! retry decisions remain in the forwarding orchestrator.

use axum::response::Response;

use crate::{
    credential::UpstreamCredential,
    ingress::{response::api_error, state::GatewayState},
    oauth2_credentials::OAuth2CredentialLease,
    pipeline::RouteCandidate,
    provider::{CredentialKind, PreparedUpstreamRequest, ProviderAdapter},
    registry::{CredentialPoolBinding, RuntimeRegistry, UpstreamApi, UpstreamTarget},
};

/// All trusted data needed to execute one planned generation candidate.
pub(super) struct PreparedCandidate<'a> {
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) credential_pool: &'a CredentialPoolBinding,
    pub(super) uses_oauth2: bool,
    pub(super) oauth2_lease: Option<OAuth2CredentialLease>,
    pub(super) static_credentials: Option<Vec<UpstreamCredential<'a>>>,
    pub(super) adapter: ProviderAdapter,
    pub(super) request: PreparedUpstreamRequest,
}

/// Resolves and prepares one candidate without selecting a retry or fallback step.
pub(super) async fn prepare_candidate<'a>(
    state: &'a GatewayState,
    registry: &'a RuntimeRegistry,
    target: &'a UpstreamTarget,
    candidate: &RouteCandidate,
) -> Result<PreparedCandidate<'a>, Response> {
    // Resolve only the typed upstream API and credential-pool references under the selected target.
    let upstream_api = target
        .upstream_api(candidate.upstream_api_key())
        .ok_or_else(|| {
            api_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured native upstream API is unavailable",
            )
        })?;
    let credential_pool = registry
        .credential_pool(target.credential_pool_id())
        .ok_or_else(|| {
            api_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured credential pool is unavailable",
            )
        })?;
    let uses_oauth2 = credential_pool.kind() == CredentialKind::OAuth2BearerAccessToken;

    // Borrow either one guarded OAuth generation or the immutable API-key pool snapshot.
    let (oauth2_lease, static_credentials) = if uses_oauth2 {
        let lease = match state
            .oauth2_credentials()
            .lease_for_request(target.kind())
            .await
        {
            Ok(lease) if lease.pool_id() == credential_pool.id() => lease,
            Ok(_) | Err(_) => {
                return Err(super::oauth::oauth2_authentication_error());
            }
        };
        (Some(lease), None)
    } else {
        let credentials = state
            .credentials
            .upstream_pool(target.kind(), credential_pool.id(), credential_pool.kind())
            .map_err(|_| {
                api_error(
                    http::StatusCode::BAD_GATEWAY,
                    "upstream_authentication_error",
                    "Upstream credentials are unavailable",
                )
            })?;
        (None, Some(credentials))
    };

    // Prepare the relative Provider request before entering the bounded attempt loop.
    let adapter = ProviderAdapter::for_kind(target.kind());
    let request = adapter
        .prepare_routed_request(candidate.request(), upstream_api)
        .map_err(|_| {
            api_error(
                http::StatusCode::BAD_REQUEST,
                "unsupported_request",
                "Request is not supported by the selected provider",
            )
        })?;

    Ok(PreparedCandidate {
        upstream_api,
        credential_pool,
        uses_oauth2,
        oauth2_lease,
        static_credentials,
        adapter,
        request,
    })
}
