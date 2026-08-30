//! Native/Bridged body takeover and safe downstream construction for a selected upstream response.
//!
//! This module handles status, response headers, SSE validation, and non-streaming Bridge conversion
//! after retry/fallback ends. Error response bodies remain unchanged, and taking over a body never
//! appends another upstream attempt.

use axum::{body::to_bytes, response::Response};
use http::{HeaderValue, StatusCode, header::CONTENT_TYPE};

use crate::{
    bridge::BridgePlan,
    observability::{ErrorType, RequestObservation},
    pipeline::{
        GenerationResponseFacts, GenerationResponseMode, StreamResponseConversion,
        classify_generation_response,
    },
    provider::GenerationProviderAdapter,
    transport::upstream::{TransportError, UpstreamResponse},
};

use super::super::{
    response::{api_error, filtered_upstream_headers},
    streaming::{
        SsePrecommitError, bridge_sse_body, buffer_responses_sse_body, enforce_sse_liveness,
        precommit_sse_body, validate_sse_body,
    },
};

/// Generation response handoff before the first valid SSE event commits downstream output.
pub(super) enum UpstreamResponseOutcome {
    Response(Response),
    PrecommitFailure(TransportError),
}

impl From<Response> for UpstreamResponseOutcome {
    fn from(response: Response) -> Self {
        Self::Response(response)
    }
}

/// Response-conversion, SSE, and observation context for one selected candidate.
pub(super) struct UpstreamResponseContext {
    pub(super) validate_sse: bool,
    pub(super) adapter: GenerationProviderAdapter,
    pub(super) max_sse_event_bytes: usize,
    pub(super) max_json_body_bytes: usize,
    pub(super) generation_plan: BridgePlan,
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
) -> UpstreamResponseOutcome {
    // Split fixed response facts so call sites cannot omit protocol or observation boundaries.
    let UpstreamResponseContext {
        validate_sse,
        adapter,
        max_sse_event_bytes,
        max_json_body_bytes,
        generation_plan,
        stream_response_conversion,
        observation,
    } = context;

    // Extract status and classify successful bodies through the static Provider media profile.
    let status = upstream.status();
    let mut response_headers = filtered_upstream_headers(upstream.headers());
    let is_sse = status.is_success() && adapter.recognizes_sse_response(upstream.headers());

    // Select one operation-owned response mode before any body I/O or downstream commit.
    let response_mode = classify_generation_response(GenerationResponseFacts {
        status_is_success: status.is_success(),
        downstream_streaming: validate_sse,
        recognized_sse: is_sse,
        preserve_source: generation_plan.preserves_source(),
        stream_response_conversion,
    });
    if response_mode == GenerationResponseMode::RejectInvalidMedia {
        observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
        return api_error(
            StatusCode::BAD_GATEWAY,
            "invalid_upstream_response",
            "The upstream response could not be converted",
        )
        .into();
    }

    // Normalize a trusted implicit or parameterized SSE media type for downstream clients.
    if is_sse {
        response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    }

    // Add transparent success observation or timeout-only error observation without changing downstream bytes.
    let stream_timeout_policy = upstream.stream_timeout_policy();
    let upstream_body = upstream.into_body();
    let precommit_mode = matches!(
        response_mode,
        GenerationResponseMode::BridgeSse | GenerationResponseMode::ValidateNativeSse
    );
    let upstream_body = if precommit_mode {
        match precommit_sse_body(
            upstream_body,
            max_sse_event_bytes,
            stream_timeout_policy,
            adapter,
            (!generation_plan.preserves_source()).then_some(&generation_plan),
            &observation,
        )
        .await
        {
            Ok(body) => match response_mode {
                GenerationResponseMode::BridgeSse => {
                    body.into_bridge_liveness_body(max_sse_event_bytes, observation.clone())
                }
                GenerationResponseMode::ValidateNativeSse => {
                    body.into_native_liveness_body(max_sse_event_bytes, observation.clone())
                }
                _ => unreachable!("precommit mode is limited to streaming response modes"),
            },
            Err(SsePrecommitError::Timeout) => {
                return UpstreamResponseOutcome::PrecommitFailure(TransportError::Timeout);
            }
            Err(SsePrecommitError::Transport) => {
                return UpstreamResponseOutcome::PrecommitFailure(TransportError::ResponseBody);
            }
            Err(SsePrecommitError::Invalid) => {
                observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "invalid_upstream_response",
                    "The upstream response is invalid",
                )
                .into();
            }
            Err(SsePrecommitError::Bridge) => {
                observation.record_bridge_failure();
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "invalid_upstream_response",
                    "The upstream response cannot be converted",
                )
                .into();
            }
            Err(SsePrecommitError::EofBeforeEvent) => {
                observation.record_stream_failure(ErrorType::SseEofBeforeTerminal);
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "invalid_upstream_response",
                    "The upstream response ended before a terminal event",
                )
                .into();
            }
        }
    } else {
        upstream_body
    };
    let upstream_body = if is_sse && !precommit_mode {
        enforce_sse_liveness(
            upstream_body,
            max_sse_event_bytes,
            stream_timeout_policy,
            observation.clone(),
        )
    } else if status.is_success() {
        observation.observe_upstream_json_body(upstream_body, max_json_body_bytes)
    } else {
        observation.observe_upstream_timeout_body(upstream_body)
    };

    // Execute the selected takeover while retaining body I/O, observation, and commit in ingress.
    let body = match response_mode {
        GenerationResponseMode::BufferResponsesSse => {
            let response = match buffer_responses_sse_body(
                upstream_body,
                generation_plan.stream_renderer(),
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
                    )
                    .into();
                }
            };
            let downstream_body = match generation_plan.render_semantic_response(response) {
                Ok(body) => body,
                Err(_) => {
                    observation.record_bridge_failure();
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    )
                    .into();
                }
            };
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            axum::body::Body::from(downstream_body)
        }
        GenerationResponseMode::BridgeSse => {
            if precommit_mode {
                upstream_body
            } else {
                bridge_sse_body(
                    upstream_body,
                    generation_plan.stream_renderer(),
                    max_sse_event_bytes,
                    observation.clone(),
                )
            }
        }
        GenerationResponseMode::ValidateNativeSse => validate_sse_body(
            upstream_body,
            adapter,
            Some(generation_plan.stream_renderer()),
            max_sse_event_bytes,
            observation,
        ),
        GenerationResponseMode::RenderJson => {
            let upstream_body = match to_bytes(upstream_body, max_json_body_bytes).await {
                Ok(body) => body,
                Err(_) => {
                    observation.record_upstream_failure();
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    )
                    .into();
                }
            };
            match generation_plan.render_non_stream(upstream_body) {
                Ok(body) => axum::body::Body::from(body),
                Err(_) => {
                    observation.record_bridge_failure();
                    return api_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response could not be converted",
                    )
                    .into();
                }
            }
        }
        GenerationResponseMode::PassthroughError => upstream_body,
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
    response.into()
}
