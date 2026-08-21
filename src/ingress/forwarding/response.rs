//! Native/Bridged body takeover and safe downstream construction for a selected upstream response.
//!
//! This module handles status, response headers, SSE validation, and non-streaming Bridge conversion
//! after retry/fallback ends. Error response bodies remain unchanged, and taking over a body never
//! appends another upstream attempt.

use axum::{body::to_bytes, response::Response};
use http::{HeaderValue, StatusCode, header::CONTENT_TYPE};

use crate::{
    bridge::BridgePlan,
    core::ApiProtocol,
    observability::{ErrorType, RequestObservation},
    pipeline::{
        GenerationResponseFacts, GenerationResponseMode, StreamResponseConversion,
        classify_generation_response,
    },
    provider::ProviderAdapter,
    transport::upstream::UpstreamResponse,
};

use super::super::{
    response::{api_error, filtered_upstream_headers},
    streaming::{bridge_sse_body, buffer_responses_sse_body, validate_sse_body},
};

/// Response-conversion, SSE, and observation context for one selected candidate.
pub(super) struct UpstreamResponseContext {
    pub(super) validate_sse: bool,
    pub(super) upstream_protocol: ApiProtocol,
    pub(super) adapter: ProviderAdapter,
    pub(super) max_sse_event_bytes: usize,
    pub(super) max_json_body_bytes: usize,
    pub(super) bridge: Option<BridgePlan>,
    pub(super) stream_response_conversion: Option<StreamResponseConversion>,
    pub(super) observation: RequestObservation,
}

/// Sends upstream status, safe response headers, and Native/Bridged body downstream.
///
/// SSE is validated only when the original request requires streaming and the upstream returns a
/// successful status matching the Provider's trusted SSE media profile. An error response may be
/// JSON or another diagnostic body even for a streaming request; decoding it as SSE would damage
/// visible HTTP error semantics.
pub(super) async fn upstream_response(
    upstream: UpstreamResponse,
    context: UpstreamResponseContext,
) -> Response {
    // Split fixed response facts so call sites cannot omit protocol or observation boundaries.
    let UpstreamResponseContext {
        validate_sse,
        upstream_protocol,
        adapter,
        max_sse_event_bytes,
        max_json_body_bytes,
        bridge,
        stream_response_conversion,
        observation,
    } = context;

    // Extract status and classify successful bodies through the static Provider media profile.
    let status = upstream.status();
    let mut response_headers = filtered_upstream_headers(upstream.headers());
    let is_sse = status.is_success()
        && adapter.recognizes_sse_response(upstream_protocol, upstream.headers());

    // Select one operation-owned response mode before any body I/O or downstream commit.
    let response_mode = classify_generation_response(GenerationResponseFacts {
        status_is_success: status.is_success(),
        downstream_streaming: validate_sse,
        recognized_sse: is_sse,
        has_bridge: bridge.is_some(),
        stream_response_conversion,
    });
    if response_mode == GenerationResponseMode::RejectInvalidMedia {
        observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
        return api_error(
            StatusCode::BAD_GATEWAY,
            "invalid_upstream_response",
            "The upstream response could not be converted",
        );
    }

    // Normalize a trusted implicit or parameterized SSE media type for downstream clients.
    if is_sse {
        response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    }

    // Add transparent Provider usage/first-byte observation to successful non-SSE bodies without changing downstream bytes.
    let upstream_body = upstream.into_body();
    let upstream_body = if status.is_success() && !is_sse {
        observation.observe_upstream_json_body(upstream_body, max_json_body_bytes)
    } else {
        upstream_body
    };

    // Execute the selected takeover while retaining body I/O, observation, and commit in ingress.
    let body = match response_mode {
        GenerationResponseMode::BufferResponsesSse { render_bridge } => {
            let upstream_body = match buffer_responses_sse_body(
                upstream_body,
                max_sse_event_bytes,
                max_json_body_bytes,
                &observation,
            )
            .await
            {
                Ok(body) => body,
                Err(()) => {
                    observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    );
                }
            };
            let downstream_body = if render_bridge {
                let bridge = bridge
                    .as_ref()
                    .expect("response decision requires a Generation Bridge");
                match bridge.render_non_stream(upstream_body) {
                    Ok(body) => body,
                    Err(_) => {
                        observation.record_bridge_failure();
                        return api_error(
                            StatusCode::BAD_GATEWAY,
                            "invalid_upstream_response",
                            "The upstream response could not be converted",
                        );
                    }
                }
            } else {
                upstream_body
            };
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            axum::body::Body::from(downstream_body)
        }
        GenerationResponseMode::BridgeSse => {
            let bridge = bridge.expect("response decision requires a Generation Bridge");
            bridge_sse_body(
                upstream_body,
                bridge.stream_renderer(),
                max_sse_event_bytes,
                observation.clone(),
            )
        }
        GenerationResponseMode::ValidateNativeSse => validate_sse_body(
            upstream_body,
            upstream_protocol,
            adapter,
            max_sse_event_bytes,
            observation,
        ),
        GenerationResponseMode::BridgeJson => {
            let bridge = bridge.expect("response decision requires a Generation Bridge");
            let upstream_body = match to_bytes(upstream_body, max_json_body_bytes).await {
                Ok(body) => body,
                Err(_) => {
                    observation.record_upstream_failure();
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    );
                }
            };
            match bridge.render_non_stream(upstream_body) {
                Ok(body) => axum::body::Body::from(body),
                Err(_) => {
                    observation.record_bridge_failure();
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    );
                }
            }
        }
        GenerationResponseMode::Passthrough => upstream_body,
        GenerationResponseMode::RejectInvalidMedia => {
            unreachable!("invalid media returned before body takeover")
        }
    };

    // Build the final downstream response with safe upstream headers preserved.
    let mut response = Response::builder()
        .status(status)
        .body(body)
        .expect("valid upstream response status");
    response.headers_mut().extend(response_headers);
    response
}
