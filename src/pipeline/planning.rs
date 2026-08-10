//! Generates ordered Native/Bridged Route candidates from immutable Public Model execution interfaces.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    bridge::BridgePlan,
    core::{ApiProtocol, ApiRequest, EmbeddingRequest, OperationKind},
    registry::{
        IgnorableGenerationParameter, NonStreamingConversion, ReasoningLevel, RouteMode,
        RuntimeRegistry, UpstreamStreamingPolicy,
    },
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
/// Native request fields remain unchanged except for one preflight-resolved reasoning level and
/// the `model` later rewritten by the adapter. Bridged requests convert only shared semantics in
/// the explicit allowlist. A failed BridgePlan rejects the request and does not skip the Route.
pub fn plan_request(
    registry: &RuntimeRegistry,
    requirements: &RequestRequirements,
    body: Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    // Complete fixed-contract preflight and resolve the same interface's static candidates.
    let (interface, normalized_reasoning_level) = preflight_public_model(registry, requirements)?;
    if requirements.requested_capabilities.previous_response_id {
        debug_assert!(interface.continuation_candidates_match_issuer());
    }

    // Remove the typed inactive Responses projection before any Native or Bridged egress body is built.
    let normalized_body = normalize_inactive_response_include(&body, requirements.protocol())?;

    // Normalize the canonical request once so every static fallback candidate receives one effort.
    let normalized_body = normalize_reasoning_level(
        &normalized_body,
        requirements.protocol(),
        normalized_reasoning_level,
    )?;

    // Build requests in compiled priority order; request facts cannot filter or reorder candidates.
    let mut prepared_candidates = Vec::with_capacity(interface.candidates().len());
    for candidate in interface.candidates() {
        // Rebuild this candidate from the canonical body and apply only its typed omission rules.
        let candidate_body = discard_candidate_ignored_parameters(
            &normalized_body,
            candidate.ignored_generation_parameters(),
        )?;
        let (request, bridge) = match candidate.mode() {
            RouteMode::Native => (
                ApiRequest::new(candidate.downstream_protocol(), candidate_body),
                None,
            ),
            RouteMode::Bridged => match BridgePlan::prepare_with_reasoning_output(
                candidate.downstream_protocol(),
                candidate.upstream_protocol(),
                requirements.public_model(),
                candidate.upstream_model(),
                candidate_body,
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

/// Removes only an analyzed inactive Responses `include` value from the canonical request body.
fn normalize_inactive_response_include(
    body: &Bytes,
    protocol: ApiProtocol,
) -> Result<Bytes, RequestPlanningError> {
    // Preserve original bytes unless the Responses field is explicitly null or an empty array.
    if protocol != ApiProtocol::Responses {
        return Ok(body.clone());
    }
    let document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let inactive = document
        .as_object()
        .and_then(|object| object.get("include"))
        .is_some_and(|value| value.is_null() || value.as_array().is_some_and(Vec::is_empty));
    if !inactive {
        return Ok(body.clone());
    }

    // Remove the no-op field once and serialize one immutable source for every candidate.
    let mut document = document;
    document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?
        .remove("include");
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
}

/// Rewrites only a preflight-resolved canonical reasoning level before candidate expansion.
fn normalize_reasoning_level(
    body: &Bytes,
    protocol: ApiProtocol,
    normalized_level: Option<ReasoningLevel>,
) -> Result<Bytes, RequestPlanningError> {
    // Preserve the original bytes exactly when the requested level already matches the contract.
    let Some(level) = normalized_level else {
        return Ok(body.clone());
    };

    // Parse the analyzed object and replace only the protocol-owned canonical effort field.
    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    match protocol {
        ApiProtocol::ChatCompletions => {
            object.insert(
                "reasoning_effort".to_owned(),
                Value::String(level.as_wire().to_owned()),
            );
        }
        ApiProtocol::Responses => {
            let reasoning = object
                .get_mut("reasoning")
                .and_then(Value::as_object_mut)
                .ok_or(RequestPlanningError::InvalidJson)?;
            reasoning.insert(
                "effort".to_owned(),
                Value::String(level.as_wire().to_owned()),
            );
        }
    }

    // Serialize one immutable canonical body for every Native or Bridged fallback candidate.
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
}

/// Removes the selected Upstream API's closed ordinary-parameter set from one candidate body.
fn discard_candidate_ignored_parameters(
    body: &Bytes,
    ignored_parameters: &[IgnorableGenerationParameter],
) -> Result<Bytes, RequestPlanningError> {
    // Preserve the original bytes exactly when this candidate has no omission rule.
    if ignored_parameters.is_empty() {
        return Ok(body.clone());
    }

    // Parse the already analyzed object and remove only statically typed parameter names.
    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    for parameter in ignored_parameters {
        object.remove(parameter.as_wire_name());
    }

    // Serialize an independent body so omission cannot mutate later fallback candidates.
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
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
