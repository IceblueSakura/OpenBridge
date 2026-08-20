//! Credential rotation, bounded retry/fallback, and response takeover for planned Route candidates.

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    core::ApiProtocol,
    execution::AttemptCoordinator,
    observability::{ErrorType, FailureStage, RequestObservation},
    pipeline::{analyze_request, plan_request},
};

use super::{
    response::{api_error, request_planning_error_type, route_error},
    state::GatewayState,
};

use self::response::{UpstreamResponseContext, upstream_response};

mod candidate;
mod embedding_response;
mod embeddings;
mod execution;
mod oauth;
mod policy;
mod response;

pub(super) use embeddings::forward_embeddings_request;

use candidate::prepare_candidate;
use execution::{
    GenerationCandidateOutcome, PreparedGenerationExecution, StoredHttpFailure,
    run_generation_candidate,
};

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
    let mut attempts = AttemptCoordinator::new();
    let observe_cross_request_health = plan.allows_fallback();
    let mut cooldown_skipped = false;
    let mut last_http_failure: Option<StoredHttpFailure> = None;

    // Resolve the target before health checks so cooling down candidates do not touch credentials.
    for (candidate_index, candidate) in plan.candidates().iter().take(candidate_count).enumerate() {
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
        match run_generation_candidate(
            &state,
            &observation,
            &downstream_headers,
            &mut attempts,
            PreparedGenerationExecution {
                plan: &plan,
                candidate,
                target,
                prepared,
                candidate_index,
                candidate_count,
            },
        )
        .await
        {
            GenerationCandidateOutcome::Response(response) => return response,
            GenerationCandidateOutcome::NextCandidate {
                failure,
                cooldown_skipped: candidate_skipped,
            } => {
                if let Some(failure) = failure {
                    last_http_failure = Some(failure);
                }
                cooldown_skipped |= candidate_skipped;
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
pub(super) fn configuration_error(
    observation: &RequestObservation,
    message: &'static str,
) -> Response {
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
