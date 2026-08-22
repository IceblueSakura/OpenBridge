//! Binds one validated Images Generations request to its immutable Native execution candidate.

use bytes::Bytes;

use crate::{
    core::{ImagesRequest, OperationKind},
    registry::{RouteMode, RuntimeRegistry},
};

use super::{
    super::{
        error::ImagesRequestError,
        types::{ImagesRequestRequirements, ImagesRouteCandidate, ImagesRoutePlan},
    },
    preflight::preflight_public_model,
};

/// Generates the single Native Images candidate from its precompiled execution interface.
pub fn plan_images_request(
    registry: &RuntimeRegistry,
    requirements: &ImagesRequestRequirements,
    body: Bytes,
) -> Result<ImagesRoutePlan, ImagesRequestError> {
    // Complete fixed-interface preflight and retain its resolved response expectations.
    let (interface, preflight) = preflight_public_model(registry, requirements)?;
    let [candidate] = interface.candidates() else {
        return Err(ImagesRequestError::RouteUnavailable);
    };

    // Enforce the compiler invariant again without interpreting request facts or selecting another Route.
    if candidate.mode() != RouteMode::Native
        || candidate.downstream_operation() != OperationKind::ImagesGenerations
        || candidate.upstream_api_key().operation() != OperationKind::ImagesGenerations
    {
        return Err(ImagesRequestError::RouteUnavailable);
    }

    // Bind the preserved body to the one trusted target/API identity owned by the interface.
    Ok(ImagesRoutePlan {
        candidate: ImagesRouteCandidate {
            route_id: candidate.route_id().to_owned(),
            upstream_target_id: candidate.upstream_target_id().to_owned(),
            upstream_api_key: candidate.upstream_api_key(),
            request: ImagesRequest::new(body),
        },
        outputs: preflight.outputs,
        size: preflight.size,
        response_format: preflight.response_format,
        response_budget: interface.response_budget(),
    })
}
