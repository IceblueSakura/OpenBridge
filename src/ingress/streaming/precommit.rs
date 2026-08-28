//! SSE precommit validation and exact downstream handoff.

use super::*;

/// Outcome before a successful SSE response may commit downstream headers or bytes.
pub(in crate::ingress) enum SsePrecommitError {
    Timeout,
    Transport,
    Invalid,
    Bridge,
    EofBeforeEvent,
}

/// Exact precommitted SSE prefix plus the same liveness state that admitted it.
pub(in crate::ingress) struct PrecommittedSseBody {
    pub(super) body: axum::body::Body,
    pub(super) liveness: SseLivenessDeadline,
    pub(super) kind: PrecommittedSseKind,
}

pub(super) enum PrecommittedSseKind {
    Native,
    Bridge {
        rendered_prefix: Bytes,
        renderer: Option<Box<BridgeStreamRenderer>>,
    },
}

impl PrecommittedSseBody {
    /// Continues event-idle timing without replaying the prefix as a new first event.
    pub(in crate::ingress) fn into_native_liveness_body(
        self,
        max_sse_event_bytes: usize,
        observation: RequestObservation,
    ) -> axum::body::Body {
        assert!(matches!(self.kind, PrecommittedSseKind::Native));
        enforce_sse_liveness_with_state(
            self.body,
            max_sse_event_bytes,
            self.liveness,
            true,
            observation,
        )
    }

    /// Emits the already rendered first output and continues with the same Bridge renderer.
    pub(in crate::ingress) fn into_bridge_liveness_body(
        self,
        max_sse_event_bytes: usize,
        observation: RequestObservation,
    ) -> axum::body::Body {
        let PrecommittedSseKind::Bridge {
            rendered_prefix,
            renderer,
        } = self.kind
        else {
            unreachable!("Bridge response mode requires a Bridge precommit handoff");
        };
        let continuation = match renderer {
            Some(renderer) => {
                let source = enforce_sse_liveness_with_state(
                    self.body,
                    max_sse_event_bytes,
                    self.liveness,
                    false,
                    observation.clone(),
                );
                bridge_sse_body(source, *renderer, max_sse_event_bytes, observation)
            }
            None => axum::body::Body::empty(),
        };
        let first = stream::once(async move { Ok::<Bytes, axum::Error>(rendered_prefix) });
        axum::body::Body::from_stream(first.chain(continuation.into_data_stream()))
    }
}

fn classify_precommit_event(
    adapter: GenerationProviderAdapter,
    renderer: Option<&mut BridgeStreamRenderer>,
    event: SseEvent,
) -> Result<(StreamEventStatus, Option<Bytes>), SsePrecommitError> {
    let status = adapter
        .classify_sse_event(event.clone())
        .map_err(|_| SsePrecommitError::Invalid)?
        .status();
    let rendered = match renderer {
        Some(renderer) => Some(
            renderer
                .render(event)
                .map_err(|_| SsePrecommitError::Bridge)?,
        ),
        None => None,
    };
    Ok((status, rendered))
}

/// Buffers one raw event at a time until Provider-valid downstream output is available.
pub(in crate::ingress) async fn precommit_sse_body(
    body: axum::body::Body,
    max_sse_event_bytes: usize,
    policy: Option<UpstreamTimeoutPolicy>,
    adapter: GenerationProviderAdapter,
    bridge: Option<&BridgePlan>,
    observation: &RequestObservation,
) -> Result<PrecommittedSseBody, SsePrecommitError> {
    let mut source = Box::pin(body.into_data_stream());
    let mut decoder = SseDecoder::new(max_sse_event_bytes);
    let mut bridge_renderer = bridge.map(BridgePlan::stream_renderer);
    let mut liveness = SseLivenessDeadline::new(policy);
    let mut prefix = BytesMut::new();
    let mut terminal_seen = false;

    loop {
        let next = liveness
            .next(source.as_mut())
            .await
            .map_err(|_| SsePrecommitError::Timeout)?;
        match next {
            Some(Ok(chunk)) => {
                let mut offset = 0;
                while offset < chunk.len() {
                    let segment_start = offset;
                    let (event, consumed) = decoder
                        .push_until_event(&chunk[offset..])
                        .map_err(|_| SsePrecommitError::Invalid)?;
                    if prefix.len().saturating_add(consumed) > max_sse_event_bytes {
                        return Err(SsePrecommitError::Invalid);
                    }
                    prefix.extend_from_slice(&chunk[offset..offset + consumed]);
                    offset += consumed;
                    if bridge_renderer.is_some() {
                        observation.record_upstream_chunk(&chunk.slice(segment_start..offset));
                    }
                    let Some(event) = event else {
                        break;
                    };
                    if bridge_renderer.is_some() {
                        observation.record_upstream_events(std::slice::from_ref(&event));
                    }
                    let (status, rendered) =
                        classify_precommit_event(adapter, bridge_renderer.as_mut(), event)?;
                    terminal_seen |= status != StreamEventStatus::Continue;
                    liveness.record_framed_event();

                    // Preserve the same-chunk suffix and original source for the selected handoff.
                    let remainder = (offset < chunk.len()).then(|| chunk.slice(offset..));
                    let remainder =
                        stream::iter(remainder.into_iter().map(Ok::<Bytes, axum::Error>));
                    match rendered {
                        Some(rendered_prefix) if rendered_prefix.is_empty() => {
                            // The renderer owns this event's state, so its raw bytes need not be replayed.
                            prefix.clear();
                        }
                        Some(rendered_prefix) => {
                            return Ok(PrecommittedSseBody {
                                body: axum::body::Body::from_stream(remainder.chain(source)),
                                liveness,
                                kind: PrecommittedSseKind::Bridge {
                                    rendered_prefix,
                                    renderer: bridge_renderer.take().map(Box::new),
                                },
                            });
                        }
                        None => {
                            let first =
                                stream::once(
                                    async move { Ok::<Bytes, axum::Error>(prefix.freeze()) },
                                );
                            return Ok(PrecommittedSseBody {
                                body: axum::body::Body::from_stream(
                                    first.chain(remainder).chain(source),
                                ),
                                liveness,
                                kind: PrecommittedSseKind::Native,
                            });
                        }
                    }
                }
            }
            Some(Err(error)) => {
                return if is_timeout_error(&error) {
                    Err(SsePrecommitError::Timeout)
                } else {
                    Err(SsePrecommitError::Transport)
                };
            }
            None => {
                let mut events = decoder.finish().map_err(|_| SsePrecommitError::Invalid)?;
                if let Some(event) = events.pop() {
                    debug_assert!(events.is_empty());
                    if bridge_renderer.is_some() {
                        observation.record_upstream_events(std::slice::from_ref(&event));
                    }
                    let (status, rendered) =
                        classify_precommit_event(adapter, bridge_renderer.as_mut(), event)?;
                    terminal_seen |= status != StreamEventStatus::Continue;
                    liveness.record_framed_event();
                    match rendered {
                        Some(rendered_prefix) if rendered_prefix.is_empty() => prefix.clear(),
                        Some(rendered_prefix) => {
                            return Ok(PrecommittedSseBody {
                                body: axum::body::Body::empty(),
                                liveness,
                                kind: PrecommittedSseKind::Bridge {
                                    rendered_prefix,
                                    renderer: bridge_renderer.take().map(Box::new),
                                },
                            });
                        }
                        None => {
                            let first =
                                stream::once(
                                    async move { Ok::<Bytes, axum::Error>(prefix.freeze()) },
                                );
                            return Ok(PrecommittedSseBody {
                                body: axum::body::Body::from_stream(first),
                                liveness,
                                kind: PrecommittedSseKind::Native,
                            });
                        }
                    }
                }
                if terminal_seen && let Some(mut renderer) = bridge_renderer.take() {
                    let rendered_prefix =
                        renderer.finish().map_err(|_| SsePrecommitError::Bridge)?;
                    if !rendered_prefix.is_empty() {
                        return Ok(PrecommittedSseBody {
                            body: axum::body::Body::empty(),
                            liveness,
                            kind: PrecommittedSseKind::Bridge {
                                rendered_prefix,
                                renderer: None,
                            },
                        });
                    }
                }
                return Err(SsePrecommitError::EofBeforeEvent);
            }
        }
    }
}
