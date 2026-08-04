//! Generates ordered Native/Bridged Route candidates from immutable Public Model execution interfaces.

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

/// Generates a Native or Bridged execution plan from one Public Model's precompiled interface.
///
/// Native request fields remain unchanged except for the `model` later rewritten by the adapter;
/// Bridged requests convert only shared semantics in the explicit allowlist. A failed BridgePlan
/// rejects the request and does not become a reason to skip the Route.
pub fn plan_request(
    registry: &RuntimeRegistry,
    requirements: &RequestRequirements,
    body: Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    // Complete fixed-contract preflight and resolve the same interface's static candidates.
    let interface = preflight_public_model(registry, requirements)?;

    // Build requests in compiled priority order; request facts cannot filter or reorder candidates.
    let mut prepared_candidates = Vec::with_capacity(interface.candidates().len());
    for candidate in interface.candidates() {
        let (request, bridge) = match candidate.mode() {
            RouteMode::Native => (
                ApiRequest::new(candidate.downstream_protocol(), body.clone()),
                None,
            ),
            RouteMode::Bridged => match BridgePlan::prepare_with_reasoning_output(
                candidate.downstream_protocol(),
                candidate.upstream_protocol(),
                requirements.public_model(),
                candidate.upstream_model(),
                body.clone(),
                candidate.reasoning_output(),
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => return Err(RequestPlanningError::UnsupportedCapabilities),
            },
        };
        prepared_candidates.push(RouteCandidate {
            route_id: candidate.route_id().to_owned(),
            upstream_target_id: candidate.upstream_target_id().to_owned(),
            upstream_api_id: candidate.upstream_api_id().to_owned(),
            request,
            bridge,
        });
    }

    // The compiler creates an interface only for at least one static candidate.
    debug_assert!(!prepared_candidates.is_empty());

    // Preserve request-level state affinity while handing the fixed candidate sequence to forwarding.
    Ok(RoutePlan {
        candidates: prepared_candidates,
        is_streaming: requirements.is_streaming,
        allows_fallback: !requirements.requested_capabilities.previous_response_id,
    })
}
