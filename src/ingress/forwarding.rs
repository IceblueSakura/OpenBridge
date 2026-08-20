//! Credential rotation, bounded retry/fallback, and response takeover for planned Route candidates.

use std::collections::HashSet;

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    bridge::BridgePlan,
    core::ApiProtocol,
    observability::{
        ErrorType, FailureStage, NextAction, ProviderAttemptContext, RequestObservation,
    },
    pipeline::{analyze_request, plan_request},
    provider::ProviderAdapter,
    transport::upstream::UpstreamResponse,
};

use super::{
    attempt::{AttemptManager, AttemptStep},
    response::{api_error, request_planning_error_type, route_error, upstream_error},
    state::GatewayState,
};

use self::response::{UpstreamResponseContext, upstream_response};

mod candidate;
mod embedding_response;
mod embeddings;
mod oauth;
mod policy;
mod response;

pub(super) use embeddings::forward_embeddings_request;

use candidate::prepare_candidate;
use oauth::{oauth2_authentication_error, recover_after_unauthorized};
use policy::{
    http_attempt_failure, should_retry_error, should_retry_status, transport_attempt_failure,
};

struct StoredHttpFailure {
    upstream: UpstreamResponse,
    adapter: ProviderAdapter,
    upstream_protocol: ApiProtocol,
    bridge: Option<BridgePlan>,
}

/// Sends a request that passed HTTP input checks through ordered Native/Bridged candidates.
///
/// Each call uses the immutable registry built at startup. A streaming request may retry only
/// before any downstream body is returned. Once an `UpstreamResponse` is given to the client,
/// later SSE bytes may only continue unchanged or terminate with a body error; a second upstream
/// attempt can never be appended. Target-bound state such as `previous_response_id` disables
/// cross-candidate fallback while allowing bounded pre-output retry on the same candidate.
pub(super) async fn forward_request(
    state: GatewayState,
    observation: RequestObservation,
    protocol: ApiProtocol,
    downstream_headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Analyze request facts, perform one Public Model preflight, and build the fixed-order RoutePlan.
    let registry = state.registry.clone();
    let requirements = match analyze_request(protocol, &body) {
        Ok(requirements) => requirements,
        Err(error) => {
            observation.record_request_failure(
                request_planning_error_type(&error),
                FailureStage::Analysis,
                false,
            );
            return route_error(error);
        }
    };
    let plan = match plan_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => {
            observation.record_request_failure(
                request_planning_error_type(&error),
                FailureStage::Planning,
                false,
            );
            return route_error(error);
        }
    };
    observation.record_planned_request(
        protocol.operation(),
        requirements.public_model(),
        requirements.is_streaming(),
    );
    let candidate_count = if plan.allows_fallback() {
        plan.candidates().len()
    } else {
        1
    };
    let mut attempts = AttemptManager::new();
    let observe_cross_request_health = plan.allows_fallback();
    let mut cooldown_skipped = false;
    let mut last_http_failure = None;

    // Resolve the target before health checks so cooling down candidates do not touch credentials.
    'candidates: for (candidate_index, candidate) in
        plan.candidates().iter().take(candidate_count).enumerate()
    {
        attempts.begin_candidate();
        let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
            return configuration_error(&observation, "Configured upstream target is unavailable");
        };
        // New stateless requests skip scopes still cooling down; target-bound continuations always try the original target.
        if observe_cross_request_health
            && !state.health.is_available(
                candidate.upstream_target_id(),
                target,
                std::time::Instant::now(),
            )
        {
            cooldown_skipped = true;
            observation.record_cooldown_skip(candidate.upstream_target_id());
            continue;
        }
        // Prepare the selected target's typed API, credential source, adapter, and wire request.
        let prepared = match prepare_candidate(&state, &registry, target, candidate).await {
            Ok(prepared) => prepared,
            Err(response) => {
                observation.record_request_failure(
                    ErrorType::ConfigurationError,
                    FailureStage::Credential,
                    false,
                );
                return response;
            }
        };
        let upstream_api = prepared.upstream_api;
        let credential_pool = prepared.credential_pool;
        let uses_oauth2 = prepared.uses_oauth2;
        let mut oauth2_lease = prepared.oauth2_lease;
        let static_credentials = prepared.static_credentials;
        let adapter = prepared.adapter;
        let request = prepared.request;

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
                                return configuration_error(
                                    &observation,
                                    "State-bound routes require exactly one credential member",
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
                                    cooldown_skipped = true;
                                    observation
                                        .record_cooldown_skip(candidate.upstream_target_id());
                                    continue 'candidates;
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
                            return oauth2_authentication_error();
                        }
                    },
                    None => static_credentials
                        .as_ref()
                        .expect("API-key target must retain static credentials")[member_index],
                };
                let headers = match adapter.build_outbound_headers(&credential, &downstream_headers)
                {
                    Ok(headers) => headers,
                    Err(_) => {
                        return configuration_error(
                            &observation,
                            "Provider authentication could not be prepared",
                        );
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
                return api_error(
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
                bridged: candidate.bridge().is_some(),
            });
            if let Some(mapping) = request.reasoning_level_mapping() {
                tracing::info!(
                    downstream_reasoning_level = mapping.downstream.as_wire(),
                    upstream_reasoning_level = mapping.upstream,
                    "reasoning_level_mapped"
                );
            }
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
                        &state,
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
                            return response;
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
                            AttemptManager::wait_before_next_attempt(backoff).await;
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            last_http_failure = Some(StoredHttpFailure {
                                upstream,
                                adapter,
                                upstream_protocol: candidate.request().protocol(),
                                bridge: candidate.bridge().cloned(),
                            });
                            let backoff = attempts.schedule_backoff();
                            observation.record_fallback(attempt_failure.error_type, backoff);
                            AttemptManager::wait_before_next_attempt(backoff).await;
                            continue 'candidates;
                        }
                        AttemptStep::Finish => {
                            return upstream_response(
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
                            .await;
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
                    return upstream_response(
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
                    .await;
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
                            AttemptManager::wait_before_next_attempt(backoff).await;
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            let backoff = attempts.schedule_backoff();
                            observation.record_fallback(attempt_failure.error_type, backoff);
                            AttemptManager::wait_before_next_attempt(backoff).await;
                            continue 'candidates;
                        }
                        AttemptStep::Finish => return upstream_error(error),
                    }
                }
                Err(error) => {
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_attempt_failure(&error, NextAction::Finish),
                    );
                    return upstream_error(error);
                }
            }
        }
    }

    if let Some(failure) = last_http_failure {
        upstream_response(
            failure.upstream,
            UpstreamResponseContext {
                validate_sse: plan.is_streaming(),
                upstream_protocol: failure.upstream_protocol,
                adapter: failure.adapter,
                max_sse_event_bytes: plan.max_sse_event_bytes(),
                max_json_body_bytes: plan.max_json_response_body_bytes(),
                bridge: failure.bridge,
                stream_response_conversion: None,
                observation,
            },
        )
        .await
    } else if cooldown_skipped {
        observation.record_request_failure(
            ErrorType::UpstreamUnavailable,
            FailureStage::Upstream,
            true,
        );
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_cooldown",
            "All configured upstream targets are temporarily unavailable",
        )
    } else {
        observation.record_request_failure(
            ErrorType::UpstreamFailure,
            FailureStage::Upstream,
            false,
        );
        api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        )
    }
}

/// Records one credential/preparation failure before returning a stable configuration response.
fn configuration_error(observation: &RequestObservation, message: &'static str) -> Response {
    observation.record_request_failure(
        ErrorType::ConfigurationError,
        FailureStage::Credential,
        false,
    );
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "configuration_error",
        message,
    )
}
