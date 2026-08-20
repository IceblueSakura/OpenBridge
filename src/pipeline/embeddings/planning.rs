//! Binds one validated Embeddings request to its immutable Native execution candidate.

use bytes::Bytes;

use crate::{
    core::{EmbeddingRequest, OperationKind},
    registry::{RouteMode, RuntimeRegistry},
};

use super::{
    super::{
        error::EmbeddingRequestError,
        types::{EmbeddingRequestRequirements, EmbeddingRouteCandidate, EmbeddingRoutePlan},
    },
    preflight::preflight_public_model,
};

/// Generates the single Native Embeddings candidate from its precompiled execution interface.
pub fn plan_embedding_request(
    registry: &RuntimeRegistry,
    requirements: &EmbeddingRequestRequirements,
    body: Bytes,
) -> Result<EmbeddingRoutePlan, EmbeddingRequestError> {
    // Complete fixed-interface preflight and retain its resolved response expectations.
    let (interface, encoding, dimensions) = preflight_public_model(registry, requirements)?;
    let [candidate] = interface.candidates() else {
        return Err(EmbeddingRequestError::RouteUnavailable);
    };

    // Enforce the compiler invariant again without interpreting request facts or selecting another Route.
    if candidate.mode() != RouteMode::Native
        || candidate.downstream_operation() != OperationKind::EmbeddingsCreate
        || candidate.upstream_operation() != OperationKind::EmbeddingsCreate
    {
        return Err(EmbeddingRequestError::RouteUnavailable);
    }

    // Bind the preserved body to the one trusted target/API identity owned by the interface.
    Ok(EmbeddingRoutePlan {
        candidate: EmbeddingRouteCandidate {
            route_id: candidate.route_id().to_owned(),
            upstream_target_id: candidate.upstream_target_id().to_owned(),
            upstream_api_key: candidate.upstream_api_key(),
            request: EmbeddingRequest::new(body),
        },
        input_count: requirements.input_count,
        encoding,
        dimensions,
        response_budget: interface.response_budget(),
    })
}
