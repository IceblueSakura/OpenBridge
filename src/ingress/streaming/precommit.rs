//! Bounded SSE precommit decoding and same-renderer downstream handoff.

use super::*;

/// Failure before a valid encoded event commits downstream headers or bytes.
pub(in crate::ingress) enum SsePrecommitError {
    Timeout,
    Transport,
    Invalid,
    Bridge,
    EofBeforeEvent,
}

/// Encoded prefix with the exact decoder lifecycle and upstream deadline that produced it.
pub(in crate::ingress) struct PrecommittedSseBody {
    body: axum::body::Body,
    liveness: SseLivenessDeadline,
    rendered_prefix: Bytes,
    renderer: BridgeStreamRenderer,
}

impl PrecommittedSseBody {
    /// Emits the validated prefix, then continues through the same Native or Bridge codec.
    pub(in crate::ingress) fn into_encoded_liveness_body(
        self,
        max_sse_event_bytes: usize,
        observation: RequestObservation,
    ) -> axum::body::Body {
        let source = enforce_sse_liveness_with_state(
            self.body,
            max_sse_event_bytes,
            self.liveness,
            false,
            observation.clone(),
        );
        let continuation = bridge_sse_body(source, self.renderer, max_sse_event_bytes, observation);
        let first = stream::once(async move { Ok::<Bytes, axum::Error>(self.rendered_prefix) });
        axum::body::Body::from_stream(first.chain(continuation.into_data_stream()))
    }
}

fn classify_precommit_event(
    adapter: GenerationProviderAdapter,
    renderer: &mut BridgeStreamRenderer,
    event: SseEvent,
) -> Result<Bytes, SsePrecommitError> {
    // Retain the trusted Provider classification boundary, then require canonical decoding.
    adapter
        .classify_sse_event(event.clone())
        .map_err(|_| SsePrecommitError::Invalid)?;
    renderer
        .render(event)
        .map_err(|_| SsePrecommitError::Bridge)
}

/// Consumes at most one bounded raw event at a time until the encoder produces downstream output.
pub(in crate::ingress) async fn precommit_sse_body(
    body: axum::body::Body,
    max_sse_event_bytes: usize,
    policy: Option<UpstreamTimeoutPolicy>,
    adapter: GenerationProviderAdapter,
    plan: &BridgePlan,
    observation: &RequestObservation,
) -> Result<PrecommittedSseBody, SsePrecommitError> {
    let mut source = Box::pin(body.into_data_stream());
    let mut decoder = SseDecoder::new(max_sse_event_bytes);
    let mut renderer = plan.stream_renderer();
    let mut liveness = SseLivenessDeadline::new(policy);
    let mut prefix_bytes = 0_usize;

    // Release each consumed raw event; only the codec owns partial semantic state.
    loop {
        let next = liveness
            .next(source.as_mut())
            .await
            .map_err(|_| SsePrecommitError::Timeout)?;
        match next {
            Some(Ok(chunk)) => {
                let mut offset = 0;
                while offset < chunk.len() {
                    let start = offset;
                    let (event, consumed) = decoder
                        .push_until_event(&chunk[offset..])
                        .map_err(|_| SsePrecommitError::Invalid)?;
                    prefix_bytes = prefix_bytes
                        .checked_add(consumed)
                        .ok_or(SsePrecommitError::Invalid)?;
                    if prefix_bytes > max_sse_event_bytes {
                        return Err(SsePrecommitError::Invalid);
                    }
                    offset += consumed;
                    observation.record_upstream_chunk(&chunk.slice(start..offset));
                    let Some(event) = event else { break };
                    observation.record_upstream_events(std::slice::from_ref(&event));
                    let rendered_prefix = classify_precommit_event(adapter, &mut renderer, event)?;
                    liveness.record_framed_event();
                    prefix_bytes = 0;
                    if rendered_prefix.is_empty() {
                        continue;
                    }
                    // Keep unread bytes and the same renderer; never decode the prefix twice.
                    let remainder = (offset < chunk.len()).then(|| chunk.slice(offset..));
                    let remainder =
                        stream::iter(remainder.into_iter().map(Ok::<Bytes, axum::Error>));
                    return Ok(PrecommittedSseBody {
                        body: axum::body::Body::from_stream(remainder.chain(source)),
                        liveness,
                        rendered_prefix,
                        renderer,
                    });
                }
            }
            Some(Err(error)) => {
                return Err(if is_timeout_error(&error) {
                    SsePrecommitError::Timeout
                } else {
                    SsePrecommitError::Transport
                });
            }
            None => {
                let mut events = decoder.finish().map_err(|_| SsePrecommitError::Invalid)?;
                if let Some(event) = events.pop() {
                    debug_assert!(events.is_empty());
                    observation.record_upstream_events(std::slice::from_ref(&event));
                    let rendered_prefix = classify_precommit_event(adapter, &mut renderer, event)?;
                    liveness.record_framed_event();
                    if !rendered_prefix.is_empty() {
                        // EOF validation stays in the continuation so already produced bytes are not discarded.
                        return Ok(PrecommittedSseBody {
                            body: axum::body::Body::empty(),
                            liveness,
                            rendered_prefix,
                            renderer,
                        });
                    }
                }
                return Err(SsePrecommitError::EofBeforeEvent);
            }
        }
    }
}
