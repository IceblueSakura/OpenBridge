//! Single-attempt Native Embeddings forwarding with bounded success-response validation.
//!
//! Request analysis, fixed-interface preflight, trusted egress, and pre-commit validation are
//! complete here. Replay eligibility, stable errors, and operation-aware metrics remain owned by
//! later current-focus stages.

use std::collections::HashSet;

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    observability::RequestObservation,
    pipeline::{analyze_embedding_request, plan_embedding_request},
    provider::ProviderAdapter,
};

use super::super::{
    response::{api_error, filtered_upstream_headers, route_error, upstream_error},
    state::GatewayState,
};

mod response;

/// Sends one preflighted Native Embeddings request to its single trusted candidate.
pub(in crate::ingress) async fn forward_embeddings_request(
    state: GatewayState,
    observation: RequestObservation,
    downstream_headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Analyze the strict request union and plan from the same immutable interface exposed by Models.
    let registry = state.registry.clone();
    let requirements = match analyze_embedding_request(&body) {
        Ok(requirements) => requirements,
        Err(error) => return route_error(error),
    };
    let plan = match plan_embedding_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => return route_error(error),
    };
    let candidate = plan.candidate();

    // Resolve only compiler-bound target, API, and credential-pool identities.
    let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
        return configuration_error("Configured upstream target is unavailable");
    };
    let Some(upstream_api) = target.upstream_api(candidate.upstream_api_id()) else {
        return configuration_error("Configured native upstream API is unavailable");
    };
    let Some(credential_pool) = registry.credential_pool(target.credential_pool_id()) else {
        return configuration_error("Configured credential pool is unavailable");
    };
    let credentials = match state.credentials.upstream_pool(
        target.kind(),
        credential_pool.id(),
        credential_pool.kind(),
    ) {
        Ok(credentials) => credentials,
        Err(_) => {
            return api_error(
                StatusCode::BAD_GATEWAY,
                "upstream_authentication_error",
                "Upstream credentials are unavailable",
            );
        }
    };

    // Select one available credential and prepare the trusted path/model/auth/header egress.
    let rejected_members = HashSet::new();
    let Some(member_index) = state.credential_health.select_member(
        credential_pool.id(),
        &credentials,
        &rejected_members,
        std::time::Instant::now(),
    ) else {
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_cooldown",
            "The configured upstream target is temporarily unavailable",
        );
    };
    let credential = &credentials[member_index];
    let adapter = ProviderAdapter::for_kind(target.kind());
    let request = match adapter.prepare_embedding_routed_request(candidate.request(), upstream_api)
    {
        Ok(request) => request,
        Err(_) => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "unsupported_request",
                "Request is not supported by the selected provider",
            );
        }
    };
    let headers = match adapter.build_outbound_headers(credential, &downstream_headers) {
        Ok(headers) => headers,
        Err(_) => return configuration_error("Provider authentication could not be prepared"),
    };

    // Execute exactly one stage-2 attempt; retry/replay/cancellation policy is added atomically in stage 4.
    observation.record_attempt(
        1,
        candidate.route_id(),
        candidate.upstream_target_id(),
        candidate.upstream_api_id(),
        target.kind(),
        false,
    );
    let upstream = match state.upstream.send(target, request, headers).await {
        Ok(upstream) => upstream,
        Err(error) => {
            observation.record_attempt_transport_failure(1, transport_error_kind(&error));
            return upstream_error(error);
        }
    };
    observation.record_attempt_http_result(1, upstream.status());
    if upstream.status().is_success() {
        // Validate the complete bounded success before any downstream response bytes are committed.
        let response = match response::validated_embedding_response(
            upstream,
            requirements.public_model(),
            upstream_api.upstream_model(),
            plan.input_count(),
            plan.encoding(),
            plan.dimensions(),
            registry.limits().max_json_response_body_bytes(),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => {
                observation.record_stream_failure("invalid_upstream_response");
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "invalid_upstream_response",
                    "The upstream response is invalid",
                );
            }
        };

        // Clear health state only after the upstream success body passes the endpoint contract.
        state
            .credential_health
            .record_success(credential_pool.id(), credential);
        state
            .health
            .record_success(candidate.upstream_target_id(), target);
        return response;
    }

    // Preserve non-success status and safe headers until the stage-5 error contract normalizes them.
    let status = upstream.status();
    let headers = filtered_upstream_headers(upstream.headers());
    let mut response = Response::builder()
        .status(status)
        .body(upstream.into_body())
        .expect("validated upstream status builds a response");
    response.headers_mut().extend(headers);
    response
}

/// Builds one stable internal configuration error without exposing registry topology.
fn configuration_error(message: &'static str) -> Response {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "configuration_error",
        message,
    )
}

/// Maps transport errors to the existing low-cardinality observation categories.
fn transport_error_kind(error: &crate::transport::upstream::TransportError) -> &'static str {
    match error {
        crate::transport::upstream::TransportError::ClientBuild(_) => "client_build",
        crate::transport::upstream::TransportError::Request(_) => "request",
        crate::transport::upstream::TransportError::Timeout => "timeout",
        crate::transport::upstream::TransportError::InvalidTarget => "invalid_target",
    }
}
