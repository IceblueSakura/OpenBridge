//! Credential rotation, bounded retry/fallback, and response takeover for planned Route candidates.

use std::collections::HashSet;

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    bridge::BridgePlan,
    core::ApiProtocol,
    observability::RequestObservation,
    pipeline::{analyze_request, plan_request},
    provider::ProviderAdapter,
    transport::upstream::{TransportError, UpstreamResponse},
};

use super::{
    attempt::{AttemptManager, AttemptStep},
    response::{api_error, route_error, upstream_error},
    state::GatewayState,
};

use self::response::{UpstreamResponseContext, upstream_response};

mod response;

struct StoredHttpFailure {
    upstream: UpstreamResponse,
    adapter: ProviderAdapter,
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
        Err(error) => return route_error(error),
    };
    observation.record_request(
        protocol,
        requirements.public_model(),
        requirements.is_streaming(),
    );
    let plan = match plan_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => return route_error(error),
    };
    let candidate_count = if plan.allows_fallback() {
        plan.candidates().len()
    } else {
        1
    };
    let mut attempts = AttemptManager::new();
    let observe_cross_request_health = plan.allows_fallback();
    let mut cooldown_skipped = false;
    let mut last_http_failure = None;

    // Prepare each candidate's target, credential, adapter, and Native request by priority.
    'candidates: for (candidate_index, candidate) in
        plan.candidates().iter().take(candidate_count).enumerate()
    {
        attempts.begin_candidate();
        let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured upstream target is unavailable",
            );
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
        let Some(upstream_api) = target.upstream_api(candidate.upstream_api_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured native upstream API is unavailable",
            );
        };
        let Some(credential_pool) = registry.credential_pool(target.credential_pool_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured credential pool is unavailable",
            );
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
        let adapter = ProviderAdapter::for_kind(target.kind());
        let request =
            match adapter.prepare_request(candidate.request(), upstream_api.upstream_model()) {
                Ok(request) => request,
                Err(_) => {
                    return api_error(
                        StatusCode::BAD_REQUEST,
                        "unsupported_request",
                        "Request is not supported by the selected provider",
                    );
                }
            };

        let mut rejected_members = HashSet::new();
        let mut current_member = None;

        // Select members and execute bounded request-level attempts before committing a downstream response.
        loop {
            // After 429, select the next member from the shared cursor; 5xx and transport retries keep the current member.
            let member_index = match current_member {
                Some(index) => index,
                None => {
                    if !plan.allows_fallback() {
                        if credentials.len() != 1 {
                            return api_error(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "configuration_error",
                                "State-bound routes require exactly one credential member",
                            );
                        }
                        0
                    } else {
                        match state.credential_health.select_member(
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
                                cooldown_skipped = true;
                                observation.record_cooldown_skip(candidate.upstream_target_id());
                                continue 'candidates;
                            }
                        }
                    }
                }
            };
            current_member = Some(member_index);
            let credential = &credentials[member_index];
            let headers = match adapter.build_outbound_headers(credential, &downstream_headers) {
                Ok(headers) => headers,
                Err(_) => {
                    return api_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "configuration_error",
                        "Provider authentication could not be prepared",
                    );
                }
            };
            if !attempts.start_attempt() {
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_attempts_exhausted",
                    "The upstream attempt budget was exhausted",
                );
            }
            observation.record_attempt(
                attempts.attempts_started() as u64,
                candidate.route_id(),
                candidate.upstream_target_id(),
                candidate.upstream_api_id(),
                target.kind(),
                candidate.bridge().is_some(),
            );
            if let Some(mapping) = candidate.reasoning_level_mapping() {
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
                Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                    // Record member-level 429 or target-level temporary unavailability by HTTP category.
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
                        && (!plan.allows_fallback()
                            || !state.credential_health.has_available_member(
                                credential_pool.id(),
                                &credentials,
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
                    match step {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            last_http_failure = Some(StoredHttpFailure {
                                upstream,
                                adapter,
                                bridge: candidate.bridge().cloned(),
                            });
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => {
                            return upstream_response(
                                upstream,
                                UpstreamResponseContext {
                                    validate_sse: plan.is_streaming(),
                                    protocol,
                                    adapter,
                                    max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                                    max_json_body_bytes: registry.limits().max_request_body_bytes(),
                                    bridge: candidate.bridge().cloned(),
                                    observation: observation.clone(),
                                },
                            )
                            .await;
                        }
                    }
                }
                Ok(upstream) => {
                    // Clear the target's known cooldown only after a successful HTTP response.
                    observation.record_attempt_http_result(
                        attempts.attempts_started() as u64,
                        upstream.status(),
                    );
                    if upstream.status().is_success() {
                        state
                            .credential_health
                            .record_success(credential_pool.id(), credential);
                        state
                            .health
                            .record_success(candidate.upstream_target_id(), target);
                    }
                    return upstream_response(
                        upstream,
                        UpstreamResponseContext {
                            validate_sse: plan.is_streaming(),
                            protocol,
                            adapter,
                            max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                            max_json_body_bytes: registry.limits().max_request_body_bytes(),
                            bridge: candidate.bridge().cloned(),
                            observation: observation.clone(),
                        },
                    )
                    .await;
                }
                Err(error) if should_retry_error(&error) => {
                    // Timeout/transport failure isolates only the fault domain and does not affect the quota scope.
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
                    );
                    state.health.record_transport_failure(
                        candidate.upstream_target_id(),
                        target,
                        std::time::Instant::now(),
                    );
                    let untried_candidates = candidate_count - candidate_index - 1;
                    match attempts.next_step(untried_candidates) {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => return upstream_error(error),
                    }
                }
                Err(error) => {
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
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
                protocol,
                adapter: failure.adapter,
                max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                max_json_body_bytes: registry.limits().max_request_body_bytes(),
                bridge: failure.bridge,
                observation,
            },
        )
        .await
    } else if cooldown_skipped {
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_cooldown",
            "All configured upstream targets are temporarily unavailable",
        )
    } else {
        api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        )
    }
}

/// Returns whether a status permits continuing the current attempt before the first downstream event.
fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == crate::provider::RetryHint::BeforeFirstEvent
}

/// Includes only timeout/request transport failures that can be safely resent in retry.
fn should_retry_error(error: &TransportError) -> bool {
    matches!(error, TransportError::Timeout | TransportError::Request(_))
}

/// Maps transport errors to low-cardinality observation categories without underlying messages.
fn transport_error_kind(error: &TransportError) -> &'static str {
    match error {
        TransportError::ClientBuild(_) => "client_build",
        TransportError::Request(_) => "request",
        TransportError::Timeout => "timeout",
        TransportError::InvalidTarget => "invalid_target",
    }
}
