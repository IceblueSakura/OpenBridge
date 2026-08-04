//! Generates ordered Native/Bridged Route candidates from the immutable registry.

use bytes::Bytes;

use crate::{
    bridge::BridgePlan,
    core::ApiRequest,
    registry::{RouteMode, RuntimeRegistry},
};

use super::{
    error::RequestPlanningError,
    preflight::preflight_public_model,
    types::{RequestRequirements, RouteCandidate, RoutePlan},
};

/// Generates a Native or Bridged execution plan along the Public Model's ordered Routes.
///
/// Native request fields remain unchanged except for the `model` later rewritten by the adapter;
/// Bridged requests convert only shared semantics in the explicit allowlist. A failed BridgePlan
/// rejects the request and does not become a reason to skip the Route.
pub fn plan_request(
    registry: &RuntimeRegistry,
    requirements: &RequestRequirements,
    body: Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    // Complete the fixed Public Model contract preflight before inspecting any Route.
    let public_model = preflight_public_model(registry, requirements)?;

    // Build executable Routes in configuration order; request capabilities do not change eligibility or order.
    let mut protocol_mismatch_seen = false;
    let mut prepared_candidates = Vec::new();
    for route_id in public_model.routes() {
        let route = registry
            .route(route_id)
            .ok_or(RequestPlanningError::NoRoute)?;
        if route.downstream_protocol() != requirements.protocol() {
            protocol_mismatch_seen = true;
            continue;
        }
        let target = registry
            .upstream_target(route.upstream_target())
            .ok_or(RequestPlanningError::NoRoute)?;
        if !target.enabled() {
            continue;
        }
        let upstream_api = target
            .upstream_api(route.upstream_api())
            .ok_or(RequestPlanningError::NoRoute)?;
        if !upstream_api
            .capabilities()
            .generation_capabilities()
            .enabled
        {
            continue;
        }
        let (request, bridge) = match route.mode() {
            RouteMode::Native => (ApiRequest::new(requirements.protocol, body.clone()), None),
            RouteMode::Bridged => match BridgePlan::prepare_with_reasoning_output(
                requirements.protocol,
                upstream_api.protocol(),
                requirements.public_model(),
                upstream_api.upstream_model(),
                body.clone(),
                upstream_api.reasoning_output(),
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => return Err(RequestPlanningError::UnsupportedCapabilities),
            },
        };
        prepared_candidates.push(RouteCandidate {
            route_id: route_id.clone(),
            upstream_target_id: route.upstream_target().to_owned(),
            upstream_api_id: route.upstream_api().to_owned(),
            request,
            bridge,
        });
    }
    // Return the most specific planning error when no candidate exists; otherwise build a plan with fallback boundaries.
    if prepared_candidates.is_empty() {
        return Err(if protocol_mismatch_seen {
            RequestPlanningError::UnsupportedProtocol
        } else {
            RequestPlanningError::NoRoute
        });
    }

    Ok(RoutePlan {
        candidates: prepared_candidates,
        is_streaming: requirements.is_streaming,
        allows_fallback: !requirements.requested_capabilities.previous_response_id,
    })
}
