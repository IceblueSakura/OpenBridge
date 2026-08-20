//! Prepared-candidate attempt execution shared by operation forwarding handlers.
//!
//! This first slice hosts the existing Embeddings lifecycle without changing policy. Generation
//! remains in the parent orchestrator until both paths can use one audited closed driver dispatch.

use std::collections::HashSet;

use axum::response::Response;
use http::{HeaderMap, StatusCode};

use crate::{
    bridge::BridgePlan,
    core::ApiProtocol,
    credential::UpstreamCredential,
    execution::{AttemptCoordinator, AttemptStep},
    observability::{
        ErrorType, FailureStage, NextAction, ProviderAttemptContext, RequestObservation,
    },
    pipeline::{EmbeddingRequestRequirements, EmbeddingRoutePlan, RouteCandidate, RoutePlan},
    provider::{PreparedUpstreamRequest, ProviderAdapter},
    registry::{CredentialPoolBinding, UpstreamApi, UpstreamTarget},
    transport::upstream::UpstreamResponse,
};

use super::{
    candidate::PreparedCandidate,
    embedding_response::validated_embedding_response,
    embeddings::configuration_error as embedding_configuration_error,
    oauth::{oauth2_authentication_error, recover_after_unauthorized},
    policy::{
        http_attempt_failure, should_retry_error, should_retry_status, transport_attempt_failure,
    },
    response::{UpstreamResponseContext, upstream_response},
};
use crate::ingress::{
    response::{
        api_error, embedding_server_error, embedding_upstream_error,
        normalized_embedding_upstream_error, upstream_error,
    },
    state::GatewayState,
};

use super::configuration_error as generation_configuration_error;

/// Retryable HTTP response retained while a later Generation candidate is attempted.
pub(super) struct StoredHttpFailure {
    pub(super) upstream: UpstreamResponse,
    pub(super) adapter: ProviderAdapter,
    pub(super) upstream_protocol: ApiProtocol,
    pub(super) bridge: Option<BridgePlan>,
}

/// Terminal response or request to continue the outer fixed Generation candidate sequence.
pub(super) enum GenerationCandidateOutcome {
    /// The selected candidate reached a terminal downstream response.
    Response(Response),
    /// The selected candidate yielded to the next configured candidate.
    NextCandidate {
        /// Retryable HTTP response preserved for final fallback rendering.
        failure: Option<StoredHttpFailure>,
        /// Whether this candidate was unavailable before an upstream attempt began.
        cooldown_skipped: bool,
    },
}

/// Trusted data needed to execute one prepared Generation candidate.
pub(super) struct PreparedGenerationExecution<'a> {
    pub(super) plan: &'a RoutePlan,
    pub(super) candidate: &'a RouteCandidate,
    pub(super) target: &'a UpstreamTarget,
    pub(super) prepared: PreparedCandidate<'a>,
    pub(super) candidate_index: usize,
    pub(super) candidate_count: usize,
}

/// Executes one prepared Generation candidate without owning outer Route order or final fallback rendering.
pub(super) async fn run_generation_candidate(
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
    attempts: &mut AttemptCoordinator,
    execution: PreparedGenerationExecution<'_>,
) -> GenerationCandidateOutcome {
    let PreparedGenerationExecution {
        plan,
        candidate,
        target,
        prepared,
        candidate_index,
        candidate_count,
    } = execution;
    let PreparedCandidate {
        upstream_api,
        credential_pool,
        uses_oauth2,
        mut oauth2_lease,
        static_credentials,
        adapter,
        request,
    } = prepared;
    let mut rejected_members = HashSet::new();
    let mut current_member = None;
    let mut oauth2_replayed = false;

    // Select members and execute bounded request-level attempts before committing a downstream response.
    loop {
        // After 429, select the next member from the shared cursor; 5xx and transport retries keep the current member.
        let member_index = if uses_oauth2 {
            0
        } else {
            let credentials = static_credentials
                .as_ref()
                .expect("API-key target must retain static credentials");
            match current_member {
                Some(index) => index,
                None => {
                    if !plan.allows_fallback() {
                        if credentials.len() != 1 {
                            return GenerationCandidateOutcome::Response(
                                generation_configuration_error(
                                    observation,
                                    "State-bound routes require exactly one credential member",
                                ),
                            );
                        }
                        0
                    } else {
                        match state.credential_health.select_member(
                            credential_pool.id(),
                            credentials,
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
                                observation.record_cooldown_skip(candidate.upstream_target_id());
                                return GenerationCandidateOutcome::NextCandidate {
                                    failure: None,
                                    cooldown_skipped: true,
                                };
                            }
                        }
                    }
                }
            }
        };
        current_member = Some(member_index);
        let (credential_member_id, headers) = {
            let credential = match oauth2_lease.as_ref() {
                Some(lease) => match lease.credential() {
                    Ok(credential) => credential,
                    Err(_) => {
                        observation.record_request_failure(
                            ErrorType::UpstreamAuthentication,
                            FailureStage::Credential,
                            false,
                        );
                        return GenerationCandidateOutcome::Response(oauth2_authentication_error());
                    }
                },
                None => static_credentials
                    .as_ref()
                    .expect("API-key target must retain static credentials")[member_index],
            };
            let headers = match adapter.build_outbound_headers(&credential, downstream_headers) {
                Ok(headers) => headers,
                Err(_) => {
                    return GenerationCandidateOutcome::Response(generation_configuration_error(
                        observation,
                        "Provider authentication could not be prepared",
                    ));
                }
            };
            (credential.member_id().to_owned(), headers)
        };
        if !attempts.start_attempt() {
            observation.record_request_failure(
                ErrorType::UpstreamFailure,
                FailureStage::Upstream,
                false,
            );
            return GenerationCandidateOutcome::Response(api_error(
                StatusCode::BAD_GATEWAY,
                "upstream_attempts_exhausted",
                "The upstream attempt budget was exhausted",
            ));
        }
        observation.record_attempt(ProviderAttemptContext {
            attempt: attempts.attempts_started() as u64,
            route_id: candidate.route_id(),
            upstream_target: candidate.upstream_target_id(),
            upstream_operation: candidate.upstream_operation(),
            upstream_model: upstream_api.upstream_model(),
            provider: target.kind(),
            bridged: candidate.bridge().is_some(),
        });
        if let Some(mapping) = request.reasoning_level_mapping() {
            tracing::info!(
                downstream_reasoning_level = mapping.downstream.as_wire(),
                upstream_reasoning_level = mapping.upstream,
                "reasoning_level_mapped"
            );
        }

        // Send one owned adapter request; dropping this future cancels the in-flight transport.
        match state
            .upstream
            .send(target, request.clone(), headers.clone())
            .await
        {
            Ok(upstream) if uses_oauth2 && upstream.status() == StatusCode::UNAUTHORIZED => {
                // Recover only before response takeover and never replay one rejected generation twice.
                let current_lease = oauth2_lease
                    .as_ref()
                    .expect("OAuth2 target must retain a request lease");
                let next_lease = match recover_after_unauthorized(
                    state,
                    target.kind(),
                    credential_pool.id(),
                    current_lease,
                    &mut oauth2_replayed,
                )
                .await
                {
                    Ok(lease) => lease,
                    Err(response) => {
                        observation.record_attempt_http_result(
                            attempts.attempts_started() as u64,
                            upstream.status(),
                            Some(http_attempt_failure(
                                &adapter,
                                upstream.status(),
                                NextAction::Finish,
                            )),
                        );
                        observation.record_request_failure(
                            ErrorType::UpstreamAuthentication,
                            FailureStage::Credential,
                            false,
                        );
                        return GenerationCandidateOutcome::Response(response);
                    }
                };
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                    Some(http_attempt_failure(
                        &adapter,
                        upstream.status(),
                        NextAction::RetryCandidate,
                    )),
                );
                oauth2_lease = Some(next_lease);
                observation
                    .record_retry(ErrorType::UpstreamAuthentication, std::time::Duration::ZERO);
                continue;
            }
            Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                // Record member-level 429 or target-level temporary unavailability by HTTP category.
                let classification = adapter.classify_status(upstream.status());
                let rate_limited =
                    classification.kind() == crate::provider::UpstreamErrorKind::RateLimited;
                if rate_limited {
                    if let Some(credentials) = static_credentials.as_ref() {
                        state.credential_health.record_rate_limited(
                            credential_pool.id(),
                            &credentials[member_index],
                            upstream.headers(),
                            std::time::Instant::now(),
                        );
                        rejected_members.insert(credential_member_id);
                        current_member = None;
                    } else {
                        // A single account-bound OAuth2 credential cannot rotate after 429.
                        state.health.record_http_failure(
                            candidate.upstream_target_id(),
                            target,
                            classification.kind(),
                            upstream.headers(),
                            std::time::Instant::now(),
                        );
                    }
                } else {
                    state.health.record_http_failure(
                        candidate.upstream_target_id(),
                        target,
                        classification.kind(),
                        upstream.headers(),
                        std::time::Instant::now(),
                    );
                }
                let untried_candidates = candidate_count - candidate_index - 1;
                let mut step = attempts.next_step(untried_candidates);
                if rate_limited
                    && (uses_oauth2
                        || !plan.allows_fallback()
                        || !state.credential_health.has_available_member(
                            credential_pool.id(),
                            static_credentials
                                .as_ref()
                                .expect("API-key target must retain static credentials"),
                            &rejected_members,
                            std::time::Instant::now(),
                        ))
                {
                    step = match step {
                        AttemptStep::RetryCandidate if untried_candidates > 0 => {
                            AttemptStep::NextCandidate
                        }
                        AttemptStep::RetryCandidate => AttemptStep::Finish,
                        other => other,
                    };
                }
                let attempt_failure =
                    http_attempt_failure(&adapter, upstream.status(), step.next_action());
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                    Some(attempt_failure),
                );
                match step {
                    AttemptStep::RetryCandidate => {
                        let backoff = attempts.schedule_backoff();
                        observation.record_retry(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                        continue;
                    }
                    AttemptStep::NextCandidate => {
                        let failure = StoredHttpFailure {
                            upstream,
                            adapter,
                            upstream_protocol: candidate.request().protocol(),
                            bridge: candidate.bridge().cloned(),
                        };
                        let backoff = attempts.schedule_backoff();
                        observation.record_fallback(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                        return GenerationCandidateOutcome::NextCandidate {
                            failure: Some(failure),
                            cooldown_skipped: false,
                        };
                    }
                    AttemptStep::Finish => {
                        return GenerationCandidateOutcome::Response(
                            upstream_response(
                                upstream,
                                UpstreamResponseContext {
                                    validate_sse: plan.is_streaming(),
                                    upstream_protocol: candidate.request().protocol(),
                                    adapter,
                                    max_sse_event_bytes: plan.max_sse_event_bytes(),
                                    max_json_body_bytes: plan.max_json_response_body_bytes(),
                                    bridge: candidate.bridge().cloned(),
                                    stream_response_conversion: candidate
                                        .stream_response_conversion(),
                                    observation: observation.clone(),
                                },
                            )
                            .await,
                        );
                    }
                }
            }
            Ok(upstream) => {
                // Clear the target's known cooldown only after a successful HTTP response.
                let status = upstream.status();
                let failure = (!status.is_success())
                    .then(|| http_attempt_failure(&adapter, status, NextAction::Finish));
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    status,
                    failure,
                );
                if status.is_success() {
                    if let Some(credentials) = static_credentials.as_ref() {
                        state
                            .credential_health
                            .record_success(credential_pool.id(), &credentials[member_index]);
                    }
                    state
                        .health
                        .record_success(candidate.upstream_target_id(), target);
                }
                return GenerationCandidateOutcome::Response(
                    upstream_response(
                        upstream,
                        UpstreamResponseContext {
                            validate_sse: plan.is_streaming(),
                            upstream_protocol: candidate.request().protocol(),
                            adapter,
                            max_sse_event_bytes: plan.max_sse_event_bytes(),
                            max_json_body_bytes: plan.max_json_response_body_bytes(),
                            bridge: candidate.bridge().cloned(),
                            stream_response_conversion: candidate.stream_response_conversion(),
                            observation: observation.clone(),
                        },
                    )
                    .await,
                );
            }
            Err(error) if should_retry_error(&error) => {
                // Timeout/transport failure isolates only the fault domain and does not affect the quota scope.
                state.health.record_transport_failure(
                    candidate.upstream_target_id(),
                    target,
                    std::time::Instant::now(),
                );
                let untried_candidates = candidate_count - candidate_index - 1;
                let step = attempts.next_step(untried_candidates);
                let attempt_failure = transport_attempt_failure(&error, step.next_action());
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    attempt_failure,
                );
                match step {
                    AttemptStep::RetryCandidate => {
                        let backoff = attempts.schedule_backoff();
                        observation.record_retry(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                        continue;
                    }
                    AttemptStep::NextCandidate => {
                        let backoff = attempts.schedule_backoff();
                        observation.record_fallback(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                        return GenerationCandidateOutcome::NextCandidate {
                            failure: None,
                            cooldown_skipped: false,
                        };
                    }
                    AttemptStep::Finish => {
                        return GenerationCandidateOutcome::Response(upstream_error(error));
                    }
                }
            }
            Err(error) => {
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    transport_attempt_failure(&error, NextAction::Finish),
                );
                return GenerationCandidateOutcome::Response(upstream_error(error));
            }
        }
    }
}

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
                return embedding_configuration_error(
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
