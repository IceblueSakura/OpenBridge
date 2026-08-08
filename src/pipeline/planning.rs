//! Generates ordered Native/Bridged Route candidates from immutable Public Model execution interfaces.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    bridge::BridgePlan,
    core::{ApiRequest, EmbeddingRequest, OperationKind},
    registry::{NonStreamingConversion, RouteMode, RuntimeRegistry, UpstreamStreamingPolicy},
};

use super::{
    error::{EmbeddingRequestError, RequestPlanningError},
    preflight::{preflight_embedding_public_model, preflight_public_model},
    types::{
        EmbeddingRequestRequirements, EmbeddingRouteCandidate, EmbeddingRoutePlan,
        RequestRequirements, RouteCandidate, RoutePlan, StreamResponseConversion,
    },
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
        let (request, stream_response_conversion) = apply_streaming_policy(
            request,
            candidate.streaming_policy(),
            requirements.is_streaming,
        )?;
        prepared_candidates.push(RouteCandidate {
            route_id: candidate.route_id().to_owned(),
            upstream_target_id: candidate.upstream_target_id().to_owned(),
            upstream_operation: candidate.upstream_operation(),
            request,
            bridge,
            stream_response_conversion,
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

/// Applies one validated Upstream API streaming policy without changing Route order.
fn apply_streaming_policy(
    request: ApiRequest,
    policy: UpstreamStreamingPolicy,
    downstream_streaming: bool,
) -> Result<(ApiRequest, Option<StreamResponseConversion>), RequestPlanningError> {
    // Preserve requests for APIs that natively accept both streaming modes.
    if policy == UpstreamStreamingPolicy::Optional {
        return Ok((request, None));
    }

    // Reject a disabled non-streaming conversion even if a compiler invariant were bypassed.
    let conversion = match (downstream_streaming, policy) {
        (true, UpstreamStreamingPolicy::Required { .. }) => None,
        (
            false,
            UpstreamStreamingPolicy::Required {
                non_streaming: NonStreamingConversion::BufferResponsesSse,
            },
        ) => Some(StreamResponseConversion::BufferResponsesSse),
        (
            false,
            UpstreamStreamingPolicy::Required {
                non_streaming: NonStreamingConversion::Disabled,
            },
        ) => return Err(RequestPlanningError::NonStreamingUnsupported),
        (_, UpstreamStreamingPolicy::Optional) => unreachable!("optional policy returned above"),
    };

    // Force the trusted upstream envelope to stream after Native or Bridge request preparation.
    let mut value: Value =
        serde_json::from_slice(request.body()).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = value
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    object.insert("stream".to_owned(), Value::Bool(true));
    let body = serde_json::to_vec(&value).map_err(|_| RequestPlanningError::InvalidJson)?;
    Ok((
        ApiRequest::new(request.protocol(), Bytes::from(body)),
        conversion,
    ))
}

/// Generates the single Native Embeddings candidate from its precompiled execution interface.
pub fn plan_embedding_request(
    registry: &RuntimeRegistry,
    requirements: &EmbeddingRequestRequirements,
    body: Bytes,
) -> Result<EmbeddingRoutePlan, EmbeddingRequestError> {
    // Complete fixed-interface preflight and retain its resolved response expectations.
    let (interface, encoding, dimensions) =
        preflight_embedding_public_model(registry, requirements)?;
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
            upstream_operation: candidate.upstream_operation(),
            request: EmbeddingRequest::new(body),
        },
        input_count: requirements.input_count,
        encoding,
        dimensions,
    })
}
