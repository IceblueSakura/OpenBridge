//! Replay-bounded Native Embeddings forwarding with pre-commit success-response validation.
//!
//! Request analysis, fixed-interface preflight, trusted egress, and pre-commit validation are
//! complete here. A single compiled Route may reuse its finite attempt budget only for request
//! bodies within the independent replay limit. Errors are gateway-owned and discard upstream
//! bodies; request and attempt metrics use the independent Embeddings operation identity.

use std::collections::HashSet;

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    core::OperationKind,
    observability::RequestObservation,
    pipeline::{analyze_embedding_request, plan_embedding_request},
    provider::ProviderAdapter,
};

use super::super::{
    attempt::{AttemptManager, AttemptStep},
    response::{
        embedding_route_error, embedding_server_error, embedding_upstream_error,
        normalized_embedding_upstream_error,
    },
    state::GatewayState,
};
use super::{should_retry_error, should_retry_status};

mod response;

/// Sends one preflighted Native Embeddings request through its single trusted candidate.
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
        Err(error) => return embedding_route_error(error),
    };
    observation.record_request(
        OperationKind::EmbeddingsCreate,
        requirements.public_model(),
        false,
    );
    let replayable = body.len() <= registry.limits().max_replay_body_bytes();
    let plan = match plan_embedding_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => return embedding_route_error(error),
    };
    let candidate = plan.candidate();

    // Resolve only compiler-bound target, API, and credential-pool identities.
    let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
        return configuration_error("Configured upstream target is unavailable");
    };
    let Some(upstream_api) = target.upstream_api(candidate.upstream_operation()) else {
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
            return embedding_server_error(
                StatusCode::BAD_GATEWAY,
                "upstream_authentication_error",
                "Upstream credentials are unavailable",
            );
        }
    };

    // Prepare the one trusted path and upstream model independently of credential rotation.
    let adapter = ProviderAdapter::for_kind(target.kind());
    let request = match adapter.prepare_embedding_routed_request(candidate.request(), upstream_api)
    {
        Ok(request) => request,
        Err(_) => {
            return configuration_error("Provider request preparation failed");
        }
    };
    let mut attempts = AttemptManager::new();
    attempts.begin_candidate();
    let mut rejected_members = HashSet::new();
    let mut current_member = None;

    // Select credentials and execute the shared finite candidate budget before downstream commit.
    loop {
        // Rotate only after 429; 5xx and transport retries retain the current member.
        let member_index = match current_member {
            Some(index) => index,
            None => match state.credential_health.select_member(
                credential_pool.id(),
                &credentials,
                &rejected_members,
                std::time::Instant::now(),
            ) {
                Some(index) => {
                    if !rejected_members.is_empty() {
                        observation.record_credential_rotation();
                    }
                    index
                }
                None => {
                    return embedding_server_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "upstream_cooldown",
                        "The configured upstream target is temporarily unavailable",
                    );
                }
            },
        };
        current_member = Some(member_index);
        let credential = &credentials[member_index];
        let headers = match adapter.build_outbound_headers(credential, &downstream_headers) {
            Ok(headers) => headers,
            Err(_) => return configuration_error("Provider authentication could not be prepared"),
        };
        if !attempts.start_attempt() {
            return embedding_server_error(
                StatusCode::BAD_GATEWAY,
                "upstream_attempts_exhausted",
                "The upstream attempt budget was exhausted",
            );
        }
        observation.record_attempt(
            attempts.attempts_started() as u64,
            candidate.route_id(),
            candidate.upstream_target_id(),
            candidate.upstream_operation(),
            target.kind(),
            false,
        );

        // Send one owned adapter request; dropping this handler cancels the in-flight transport future.
        match state.upstream.send(target, request.clone(), headers).await {
            Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                // Classify retryable HTTP failures without reading or exposing their response bodies.
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                );
                let classification = adapter.classify_status(upstream.status());
                let rate_limited =
                    classification.kind() == crate::provider::UpstreamErrorKind::RateLimited;
                if rate_limited {
                    state.credential_health.record_rate_limited(
                        credential_pool.id(),
                        credential,
                        upstream.headers(),
                        std::time::Instant::now(),
                    );
                    rejected_members.insert(credential.member_id().to_owned());
                    current_member = None;
                }

                // Permit one shared-policy retry only when the body is independently replayable.
                let has_retry_credential = !rate_limited
                    || state.credential_health.has_available_member(
                        credential_pool.id(),
                        &credentials,
                        &rejected_members,
                        std::time::Instant::now(),
                    );
                if replayable
                    && has_retry_credential
                    && attempts.next_step(0) == AttemptStep::RetryCandidate
                {
                    attempts.wait_before_next_attempt().await;
                    observation.record_retry();
                    continue;
                }
                return normalized_embedding_upstream_error(upstream);
            }
            Ok(upstream) => {
                // Record the HTTP outcome before consuming a bounded success body or returning an error.
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                );
                if !upstream.status().is_success() {
                    return normalized_embedding_upstream_error(upstream);
                }

                // Validate the complete bounded success before any downstream response bytes are committed.
                let response = match response::validated_embedding_response(
                    upstream,
                    &observation,
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
                        return embedding_server_error(
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
            Err(error) if should_retry_error(&error) => {
                // Record a low-cardinality transport outcome without retaining its underlying message.
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    transport_error_kind(&error),
                );

                // Retry only replayable bodies and let handler cancellation own the backoff timer.
                if replayable && attempts.next_step(0) == AttemptStep::RetryCandidate {
                    attempts.wait_before_next_attempt().await;
                    observation.record_retry();
                    continue;
                }
                return embedding_upstream_error(error);
            }
            Err(error) => {
                // Fail immediately for non-retryable local transport construction or target errors.
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    transport_error_kind(&error),
                );
                return embedding_upstream_error(error);
            }
        }
    }
}

/// Builds one stable internal configuration error without exposing registry topology.
fn configuration_error(message: &'static str) -> Response {
    embedding_server_error(
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
