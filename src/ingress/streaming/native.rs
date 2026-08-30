//! Native Provider SSE validation, canonical sidecar validation, and terminal observation.

use super::*;

pub(in crate::ingress) fn validate_sse_body(
    body: axum::body::Body,
    adapter: GenerationProviderAdapter,
    renderer: Option<BridgeStreamRenderer>,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // Decode and validate every framed event while yielding the original source chunks unchanged.
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            false,
            false,
            renderer,
            observation,
        ),
        move |(mut source, mut decoder, mut terminal_seen, finished, mut renderer, observation)| async move {
            if finished {
                return None;
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    observation.record_upstream_chunk(&chunk);
                    match decoder.push(&chunk) {
                        Ok(events) => {
                            observation.record_upstream_events(&events);
                            if observe_sse_events(
                                adapter,
                                events,
                                &mut terminal_seen,
                                &mut renderer,
                                &observation,
                            )
                            .is_ok()
                            {
                                Some((
                                    Ok::<_, io::Error>(chunk),
                                    (source, decoder, terminal_seen, false, renderer, observation),
                                ))
                            } else {
                                observation.record_upstream_failure();
                                tokio::task::yield_now().await;
                                Some((
                                    Err(io::Error::other("upstream SSE stream is invalid")),
                                    (source, decoder, terminal_seen, true, renderer, observation),
                                ))
                            }
                        }
                        Err(_) => {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, renderer, observation),
                            ))
                        }
                    }
                }
                Some(Err(_)) => {
                    observation.record_upstream_failure();
                    tokio::task::yield_now().await;
                    Some((
                        Err(io::Error::other(
                            "upstream SSE stream terminated unexpectedly",
                        )),
                        (source, decoder, terminal_seen, true, renderer, observation),
                    ))
                }
                None => match decoder.finish() {
                    Ok(events) => {
                        observation.record_upstream_events(&events);
                        if observe_sse_events(
                            adapter,
                            events,
                            &mut terminal_seen,
                            &mut renderer,
                            &observation,
                        )
                        .is_err()
                        {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, renderer, observation),
                            ));
                        }
                        if !terminal_seen {
                            observation.record_stream_failure(ErrorType::SseEofBeforeTerminal);
                            tracing::warn!(
                                protocol = ?adapter.protocol(),
                                "upstream SSE stream ended before a terminal event"
                            );
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other(
                                    "upstream SSE stream ended before a terminal event",
                                )),
                                (source, decoder, terminal_seen, true, renderer, observation),
                            ));
                        }
                        if renderer
                            .as_mut()
                            .is_some_and(|renderer| renderer.finish().is_err())
                        {
                            observation.record_bridge_failure();
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream canonical stream is invalid")),
                                (source, decoder, terminal_seen, true, renderer, observation),
                            ));
                        }
                        observation.record_upstream_complete();
                        None
                    }
                    Err(_) => {
                        observation.record_upstream_failure();
                        tokio::task::yield_now().await;
                        Some((
                            Err(io::Error::other("upstream SSE stream is invalid")),
                            (source, decoder, terminal_seen, true, renderer, observation),
                        ))
                    }
                },
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// Classifies framed events through both the Provider profile and optional same-protocol Event IR.
fn observe_sse_events(
    adapter: GenerationProviderAdapter,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
    renderer: &mut Option<BridgeStreamRenderer>,
    observation: &RequestObservation,
) -> Result<(), ()> {
    for event in events {
        if renderer
            .as_mut()
            .is_some_and(|renderer| renderer.render(event.clone()).is_err())
        {
            observation.record_bridge_failure();
            return Err(());
        }
        let decoded = adapter.classify_sse_event(event).map_err(|_| ())?;
        match decoded.status() {
            StreamEventStatus::Continue => {}
            StreamEventStatus::Completed => *terminal_seen = true,
            StreamEventStatus::Failed => {
                *terminal_seen = true;
                observation.record_stream_failure(ErrorType::ProviderTerminalFailed);
            }
        }
    }
    Ok(())
}
