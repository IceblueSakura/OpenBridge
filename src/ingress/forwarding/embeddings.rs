//! Native Embeddings admission, planning, and trusted candidate preparation.
//!
//! The prepared-candidate attempt lifecycle lives in the sibling execution module. This handler
//! owns only operation analysis/planning, trusted registry resolution, and the request-wide attempt
//! coordinator boundary.

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    core::OperationKind,
    execution::AttemptCoordinator,
    observability::{ErrorType, FailureStage, RequestObservation},
    pipeline::{analyze_embedding_request, plan_embedding_request},
    provider::ProviderOperationAdapter,
};

use super::super::{
    response::{embedding_request_error_type, embedding_route_error, embedding_server_error},
    state::GatewayState,
};
use super::execution::{PreparedEmbeddingExecution, run_embedding_candidate};

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
        Err(error) => {
            observation.record_request_failure(
                embedding_request_error_type(&error),
                FailureStage::Analysis,
                false,
            );
            return embedding_route_error(error);
        }
    };
    let replayable = body.len() <= registry.limits().max_replay_body_bytes();
    let plan = match plan_embedding_request(&registry, &requirements, body) {
        Ok(plan) => plan,
        Err(error) => {
            observation.record_request_failure(
                embedding_request_error_type(&error),
                FailureStage::Planning,
                false,
            );
            return embedding_route_error(error);
        }
    };
    observation.record_planned_request(
        OperationKind::EmbeddingsCreate,
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
            return embedding_server_error(
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
        Some(ProviderOperationAdapter::Embeddings(adapter)) => adapter,
        Some(ProviderOperationAdapter::Generation(_))
        | Some(ProviderOperationAdapter::ImagesGenerations(_))
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

    // Preserve one request-wide attempt budget while the runner owns the prepared candidate loop.
    let mut attempts = AttemptCoordinator::new();
    attempts.begin_candidate();
    run_embedding_candidate(
        &state,
        &observation,
        &downstream_headers,
        &mut attempts,
        PreparedEmbeddingExecution {
            requirements: &requirements,
            plan: &plan,
            target,
            upstream_api,
            credential_pool,
            credentials,
            adapter,
            request,
            replayable,
        },
    )
    .await
}

/// Builds one stable internal configuration error without exposing registry topology.
pub(super) fn configuration_error(
    observation: &RequestObservation,
    message: &'static str,
) -> Response {
    observation.record_request_failure(
        ErrorType::ConfigurationError,
        FailureStage::Credential,
        false,
    );
    embedding_server_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "configuration_error",
        message,
    )
}
