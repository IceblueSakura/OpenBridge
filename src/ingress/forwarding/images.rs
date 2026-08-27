//! Native Images Generations admission, planning, trusted candidate preparation, and single attempt.
//!
//! Images Generations has no Bridge, no fallback, and no automatic retry: a request may already be
//! accepted or billed by the upstream, so only one bounded attempt is sent. This handler owns
//! analysis, planning, trusted registry resolution, the one credentialed send, and the response
//! validation boundary.

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    core::OperationKind,
    execution::AttemptCoordinator,
    observability::{
        AttemptFailure, ErrorType, FailureStage, NextAction, ProviderAttemptContext,
        RequestObservation, TimeoutPhase,
    },
    pipeline::{analyze_images_request, plan_images_request},
    provider::{ImagesProviderAdapter, ProviderOperationAdapter},
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse},
};

use super::super::{
    response::{images_request_error_type, images_route_error, images_server_error},
    state::GatewayState,
};
use super::image_response::validated_images_response;

/// Sends one preflighted Native Images request through its single trusted candidate without retry.
pub(in crate::ingress) async fn forward_images_request(
    state: GatewayState,
    observation: RequestObservation,
    downstream_headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Analyze the strict request union and plan from the same immutable interface exposed by Models.
    let registry = state.registry.clone();
    let requirements = match analyze_images_request(&body) {
        Ok(requirements) => requirements,
        Err(error) => {
            observation.record_request_failure(
                images_request_error_type(&error),
                FailureStage::Analysis,
                false,
            );
            return images_route_error(error);
        }
    };
    let plan = match plan_images_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => {
            observation.record_request_failure(
                images_request_error_type(&error),
                FailureStage::Planning,
                false,
            );
            return images_route_error(error);
        }
    };
    observation.record_planned_request(
        OperationKind::ImagesGenerations,
        requirements.public_model(),
        false,
    );
    let candidate = plan.candidate();

    // Resolve only compiler-bound target, API, and credential-pool identities.
    let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
        return configuration_error(&observation, "Configured upstream target is unavailable");
    };
    let Some(upstream_api) = target.upstream_api(candidate.upstream_api_key()) else {
        return configuration_error(
            &observation,
            "Configured native upstream API is unavailable",
        );
    };
    let Some(credential_pool) = registry.credential_pool(target.credential_pool_id()) else {
        return configuration_error(&observation, "Configured credential pool is unavailable");
    };
    let credentials = match state.credentials.upstream_pool(
        target.kind(),
        credential_pool.id(),
        credential_pool.kind(),
    ) {
        Ok(credentials) => credentials,
        Err(_) => {
            observation.record_request_failure(
                ErrorType::UpstreamAuthentication,
                FailureStage::Credential,
                false,
            );
            return images_server_error(
                StatusCode::BAD_GATEWAY,
                "upstream_authentication_error",
                "Upstream credentials are unavailable",
            );
        }
    };

    // Prepare the one trusted path and upstream model independently of credential rotation.
    let adapter = match target
        .kind()
        .definition()
        .operation_adapter(upstream_api.operation())
    {
        Some(ProviderOperationAdapter::ImagesGenerations(adapter)) => adapter,
        Some(ProviderOperationAdapter::Generation(_))
        | Some(ProviderOperationAdapter::Embeddings(_))
        | None => {
            return configuration_error(
                &observation,
                "Configured Provider operation is unavailable",
            );
        }
    };
    let request = match adapter.prepare_routed_request(candidate.request(), upstream_api) {
        Ok(request) => request,
        Err(_) => {
            return configuration_error(&observation, "Provider request preparation failed");
        }
    };

    // Send exactly one attempt; Images requests are never replayed after an uncertain outcome.
    let mut attempts = AttemptCoordinator::new();
    attempts.begin_candidate();
    assert!(
        attempts.start_attempt(),
        "a new Images request has one attempt budget"
    );
    let attempt = attempts.attempts_started() as u64;
    observation.record_attempt(ProviderAttemptContext {
        attempt,
        upstream_target: candidate.upstream_target_id(),
        upstream_operation: OperationKind::ImagesGenerations,
        upstream_model: upstream_api.upstream_model(),
        provider: target.kind(),
        bridged: false,
    });
    let upstream = send_single_attempt(
        &state,
        target,
        upstream_api,
        &adapter,
        &credentials,
        &downstream_headers,
        request,
    )
    .await;
    let upstream = match upstream {
        Ok(upstream) => upstream,
        Err(error) => {
            let error_type = images_transport_error_type(&error);
            let (status, code, message) = match &error {
                TransportError::Timeout => (
                    StatusCode::GATEWAY_TIMEOUT,
                    "upstream_timeout",
                    "The upstream request timed out",
                ),
                _ => (
                    StatusCode::BAD_GATEWAY,
                    "upstream_error",
                    "The upstream request failed",
                ),
            };
            if matches!(error, TransportError::Timeout) {
                observation.record_timeout_context(TimeoutPhase::ResponseHeaders);
            }
            observation.record_attempt_transport_failure(
                attempt,
                AttemptFailure::new(error_type, false, NextAction::Finish),
            );
            observation.record_request_failure(error_type, FailureStage::Upstream, false);
            return images_server_error(status, code, message);
        }
    };

    // Normalize a non-success status without exposing the upstream body or internal topology.
    if !upstream.status().is_success() {
        observation.record_attempt_http_result(
            attempt,
            upstream.status(),
            Some(AttemptFailure::new(
                ErrorType::UpstreamFailure,
                false,
                NextAction::Finish,
            )),
        );
        observation.record_request_failure(
            ErrorType::UpstreamFailure,
            FailureStage::Upstream,
            false,
        );
        return normalized_images_upstream_error(upstream);
    }
    observation.record_attempt_http_result(attempt, upstream.status(), None);

    // Validate the complete bounded upstream body before downstream commit; never retain image values.
    match validated_images_response(
        upstream,
        &observation,
        plan.outputs(),
        plan.response_format(),
        plan.max_json_response_body_bytes(),
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            observation.record_images_response_failure(error.error_type(), error.failure_stage());
            images_server_error(
                StatusCode::BAD_GATEWAY,
                "invalid_upstream_response",
                "The upstream response did not match the Images contract",
            )
        }
    }
}

/// Sends one credentialed Images attempt through the shared transport without retry or rotation.
async fn send_single_attempt(
    state: &GatewayState,
    target: &UpstreamTarget,
    upstream_api: &crate::registry::UpstreamApi,
    adapter: &ImagesProviderAdapter,
    credentials: &[crate::credential::UpstreamCredential<'_>],
    downstream_headers: &HeaderMap,
    request: crate::provider::PreparedUpstreamRequest,
) -> Result<UpstreamResponse, crate::transport::upstream::TransportError> {
    // Bind the first available credential member; a cooldown-exhausted pool cannot be rotated here.
    let credential = credentials
        .first()
        .expect("Images targets require at least one credential member");
    let headers = adapter
        .build_outbound_headers(credential, downstream_headers, upstream_api)
        .map_err(|_| crate::transport::upstream::TransportError::InvalidTarget)?;
    state.upstream.send(target, request, headers).await
}

/// Maps one typed transport failure into the closed observability taxonomy.
fn images_transport_error_type(error: &TransportError) -> ErrorType {
    match error {
        TransportError::ClientBuild(_) => ErrorType::TransportClientBuild,
        TransportError::Request(_) => ErrorType::TransportRequest,
        TransportError::Timeout => ErrorType::Timeout,
        TransportError::ResponseBody => ErrorType::UpstreamBodyTransport,
        TransportError::InvalidTarget => ErrorType::InvalidTarget,
    }
}

/// Replaces an upstream non-success body with a stable error while preserving only safe metadata.
fn normalized_images_upstream_error(upstream: UpstreamResponse) -> Response {
    let status = upstream.status();
    drop(upstream);
    images_server_error(
        status,
        "upstream_error",
        "The upstream service rejected the request",
    )
}

/// Builds one stable internal configuration error without exposing registry topology.
fn configuration_error(observation: &RequestObservation, message: &'static str) -> Response {
    observation.record_request_failure(
        ErrorType::ConfigurationError,
        FailureStage::Credential,
        false,
    );
    images_server_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "configuration_error",
        message,
    )
}

#[cfg(test)]
mod tests {
    use crate::{observability::ErrorType, transport::upstream::TransportError};

    use super::images_transport_error_type;

    #[test]
    fn images_transport_failures_keep_timeout_and_invalid_target_distinct() {
        assert_eq!(
            images_transport_error_type(&TransportError::Timeout),
            ErrorType::Timeout
        );
        assert_eq!(
            images_transport_error_type(&TransportError::InvalidTarget),
            ErrorType::InvalidTarget
        );
    }
}
