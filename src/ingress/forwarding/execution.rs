//! Prepared-candidate attempt execution shared by operation forwarding handlers.
//!
//! This first slice hosts the existing Embeddings lifecycle without changing policy. Generation
//! remains in the parent orchestrator until both paths can use one audited closed driver dispatch.

use std::collections::HashSet;

use axum::response::Response;
use http::{HeaderMap, StatusCode};

use crate::{
    credential::UpstreamCredential,
    execution::{AttemptCoordinator, AttemptStep},
    observability::{
        ErrorType, FailureStage, NextAction, ProviderAttemptContext, RequestObservation,
    },
    pipeline::{EmbeddingRequestRequirements, EmbeddingRoutePlan},
    provider::{PreparedUpstreamRequest, ProviderAdapter},
    registry::{CredentialPoolBinding, UpstreamApi, UpstreamTarget},
};

use super::{
    embedding_response::validated_embedding_response,
    embeddings::configuration_error,
    policy::{
        http_attempt_failure, should_retry_error, should_retry_status, transport_attempt_failure,
    },
};
use crate::ingress::{
    response::{
        embedding_server_error, embedding_upstream_error, normalized_embedding_upstream_error,
    },
    state::GatewayState,
};

/// Trusted data needed to execute one prepared Embeddings candidate.
pub(super) struct PreparedEmbeddingExecution<'a> {
    pub(super) requirements: &'a EmbeddingRequestRequirements,
    pub(super) plan: &'a EmbeddingRoutePlan,
    pub(super) target: &'a UpstreamTarget,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) credential_pool: &'a CredentialPoolBinding,
    pub(super) credentials: Vec<UpstreamCredential<'a>>,
    pub(super) adapter: ProviderAdapter,
    pub(super) request: PreparedUpstreamRequest,
    pub(super) replayable: bool,
}

/// Executes one prepared Native Embeddings candidate without owning analysis or planning.
pub(super) async fn run_embedding_candidate(
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
    attempts: &mut AttemptCoordinator,
    execution: PreparedEmbeddingExecution<'_>,
) -> Response {
    let PreparedEmbeddingExecution {
        requirements,
        plan,
        target,
        upstream_api,
        credential_pool,
        credentials,
        adapter,
        request,
        replayable,
    } = execution;
    let candidate = plan.candidate();
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
                    observation.record_request_failure(
                        ErrorType::UpstreamUnavailable,
                        FailureStage::Credential,
                        true,
                    );
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
        let headers = match adapter.build_outbound_headers(credential, downstream_headers) {
            Ok(headers) => headers,
            Err(_) => {
                return configuration_error(
                    observation,
                    "Provider authentication could not be prepared",
                );
            }
        };
        if !attempts.start_attempt() {
            observation.record_request_failure(
                ErrorType::UpstreamFailure,
                FailureStage::Upstream,
                false,
            );
            return embedding_server_error(
                StatusCode::BAD_GATEWAY,
                "upstream_attempts_exhausted",
                "The upstream attempt budget was exhausted",
            );
        }
        observation.record_attempt(ProviderAttemptContext {
            attempt: attempts.attempts_started() as u64,
            route_id: candidate.route_id(),
            upstream_target: candidate.upstream_target_id(),
            upstream_operation: candidate.upstream_operation(),
            upstream_model: upstream_api.upstream_model(),
            provider: target.kind(),
            bridged: false,
        });

        // Send one owned adapter request; dropping this handler cancels the in-flight transport future.
        match state.upstream.send(target, request.clone(), headers).await {
            Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                // Classify retryable HTTP failures without reading or exposing their response bodies.
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
                let step = if replayable
                    && has_retry_credential
                    && attempts.next_step(0) == AttemptStep::RetryCandidate
                {
                    AttemptStep::RetryCandidate
                } else {
                    AttemptStep::Finish
                };
                let attempt_failure =
                    http_attempt_failure(&adapter, upstream.status(), step.next_action());
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                    Some(attempt_failure),
                );
                if step == AttemptStep::RetryCandidate {
                    let backoff = attempts.schedule_backoff();
                    observation.record_retry(attempt_failure.error_type, backoff);
                    AttemptCoordinator::wait_before_next_attempt(backoff).await;
                    continue;
                }
                return normalized_embedding_upstream_error(upstream);
            }
            Ok(upstream) => {
                // Record the HTTP outcome before consuming a bounded success body or returning an error.
                let status = upstream.status();
                let failure = (!status.is_success())
                    .then(|| http_attempt_failure(&adapter, status, NextAction::Finish));
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    status,
                    failure,
                );
                if !status.is_success() {
                    return normalized_embedding_upstream_error(upstream);
                }

                // Validate the complete bounded success before any downstream response bytes are committed.
                let response = match validated_embedding_response(
                    upstream,
                    observation,
                    requirements.public_model(),
                    upstream_api.upstream_model(),
                    plan.input_count(),
                    plan.encoding(),
                    plan.dimensions(),
                    plan.max_json_response_body_bytes(),
                )
                .await
                {
                    Ok(response) => response,
                    Err(_) => {
                        observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
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
                let step = if replayable && attempts.next_step(0) == AttemptStep::RetryCandidate {
                    AttemptStep::RetryCandidate
                } else {
                    AttemptStep::Finish
                };
                let attempt_failure = transport_attempt_failure(&error, step.next_action());
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    attempt_failure,
                );

                // Retry only replayable bodies and let handler cancellation own the backoff timer.
                if step == AttemptStep::RetryCandidate {
                    let backoff = attempts.schedule_backoff();
                    observation.record_retry(attempt_failure.error_type, backoff);
                    AttemptCoordinator::wait_before_next_attempt(backoff).await;
                    continue;
                }
                return embedding_upstream_error(error);
            }
            Err(error) => {
                // Fail immediately for non-retryable local transport construction or target errors.
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    transport_attempt_failure(&error, NextAction::Finish),
                );
                return embedding_upstream_error(error);
            }
        }
    }
}
