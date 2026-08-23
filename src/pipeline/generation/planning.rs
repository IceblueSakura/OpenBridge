//! Generates ordered Native/Bridged Route candidates from immutable Public Model execution interfaces.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    bridge::BridgePlan,
    core::{ApiProtocol, ApiRequest, ChatStreamUsage, ResponseInclude, ResponseIncludePolicy},
    registry::{
        IgnorableGenerationParameter, NonStreamingConversion, ReasoningLevel, RouteMode,
        RuntimeRegistry, UpstreamStreamingPolicy,
    },
};

use super::super::{
    error::RequestPlanningError,
    types::{RequestRequirements, RouteCandidate, RoutePlan, StreamResponseConversion},
};
use super::{instructions::normalize_generation_request, preflight::preflight_public_model};

/// Generates a Native or Bridged execution plan from one Public Model's precompiled interface.
///
/// Planning first applies one preflight-resolved reasoning level and the gateway-owned
/// instructions/store envelope to an immutable canonical request. Native candidates preserve that
/// body until the adapter rewrites `model`; Bridged candidates convert only shared semantics in the
/// explicit allowlist. A failed BridgePlan rejects the request and does not skip the Route.
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

    // Resolve the project fallback once after Public Model preflight and before candidate expansion.
    let normalized_body = if interface.task() == crate::registry::CanonicalTaskKind::Generation {
        let default_instructions = registry
            .default_instructions()
            .expect("general Generation registries validate default instructions at startup");
        normalize_generation_request(
            &body,
            requirements.protocol(),
            &requirements.requested_instructions,
            default_instructions,
        )?
    } else {
        body
    };

    // Remove explicit Chat usage no-ops once so every fixed candidate sees the omitted-equivalent body.
    let normalized_body = normalize_chat_stream_options(
        &normalized_body,
        requirements.protocol(),
        requirements.chat_stream_usage,
    )?;

    // Remove the typed inactive Responses projection before any Native or Bridged egress body is built.
    let normalized_body =
        normalize_inactive_response_include(&normalized_body, requirements.protocol())?;

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
        let candidate_body = filter_candidate_response_includes(
            &normalized_body,
            requirements.protocol(),
            candidate.forwarded_response_includes(),
        )?;
        let candidate_body = discard_candidate_ignored_parameters(
            &candidate_body,
            candidate.ignored_generation_parameters(),
        )?;
        let (request, bridge) = match candidate.mode() {
            RouteMode::Native => (
                ApiRequest::new(candidate.downstream_protocol(), candidate_body),
                None,
            ),
            RouteMode::GenerationBridge(direction) => match BridgePlan::prepare_with_request_facts(
                direction.downstream_protocol(),
                direction.upstream_protocol(),
                requirements.public_model(),
                candidate.upstream_model(),
                candidate_body,
                candidate.reasoning_output(),
                requirements.chat_stream_usage,
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => return Err(RequestPlanningError::UnsupportedCapabilities),
            },
        };
        let (request, upstream_streaming, stream_response_conversion) = apply_streaming_policy(
            request,
            candidate.streaming_policy(),
            requirements.is_streaming,
        )?;
        prepared_candidates.push(RouteCandidate {
            route_id: candidate.route_id().to_owned(),
            upstream_target_id: candidate.upstream_target_id().to_owned(),
            upstream_api_key: candidate.upstream_api_key(),
            request,
            upstream_streaming,
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
        response_budget: interface.response_budget(),
    })
}

/// Removes only accepted `ForwardOrOmit` values absent from this candidate's Native contract.
fn filter_candidate_response_includes(
    body: &Bytes,
    protocol: ApiProtocol,
    forwarded: &[ResponseInclude],
) -> Result<Bytes, RequestPlanningError> {
    if protocol != ApiProtocol::Responses {
        return Ok(body.clone());
    }
    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    let Some(values) = object.get_mut("include").and_then(Value::as_array_mut) else {
        return Ok(body.clone());
    };

    // Preserve array order and duplicates while refusing to omit any exact-forward-only value.
    let mut removed = false;
    values.retain(|value| {
        let Some(include) = value.as_str().and_then(ResponseInclude::from_wire) else {
            return true;
        };
        if forwarded.contains(&include) {
            return true;
        }
        if include.policy() == ResponseIncludePolicy::ForwardOrOmit {
            removed = true;
            return false;
        }
        true
    });
    if values.iter().any(|value| {
        value
            .as_str()
            .and_then(ResponseInclude::from_wire)
            .is_some_and(|include| !forwarded.contains(&include))
    }) {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }
    if !removed {
        return Ok(body.clone());
    }
    if values.is_empty() {
        object.remove("include");
    }
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
}

/// Removes only omitted-equivalent Chat stream options before any candidate body is materialized.
fn normalize_chat_stream_options(
    body: &Bytes,
    protocol: ApiProtocol,
    usage: ChatStreamUsage,
) -> Result<Bytes, RequestPlanningError> {
    if protocol != ApiProtocol::ChatCompletions || usage.is_requested() {
        return Ok(body.clone());
    }
    let mut document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    if object.remove("stream_options").is_none() {
        return Ok(body.clone());
    }
    serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)
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
) -> Result<(ApiRequest, bool, Option<StreamResponseConversion>), RequestPlanningError> {
    // Preserve requests for APIs that natively accept both streaming modes.
    if policy == UpstreamStreamingPolicy::Optional {
        return Ok((request, downstream_streaming, None));
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
        true,
        conversion,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_include_filter_removes_only_the_approved_hint_and_preserves_order() {
        let body = Bytes::from_static(
            br#"{"include":["file_search_call.results","reasoning.encrypted_content","file_search_call.results"],"input":"hello"}"#,
        );

        let filtered = filter_candidate_response_includes(
            &body,
            ApiProtocol::Responses,
            &[ResponseInclude::FileSearchCallResults],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&filtered).unwrap();

        assert_eq!(
            value["include"],
            serde_json::json!(["file_search_call.results", "file_search_call.results"])
        );
    }
}
