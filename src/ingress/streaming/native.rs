//! Native Provider SSE validation and terminal observation.

use super::*;

pub(in crate::ingress) fn validate_sse_body(
    body: axum::body::Body,
    adapter: GenerationProviderAdapter,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // Create an incremental SSE decoder that owns the upstream source lifetime.
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            false,
            false,
            observation,
        ),
        move |(mut source, mut decoder, mut terminal_seen, finished, observation)| async move {
            if finished {
                return None;
            }
            // Read the next upstream chunk and observe framing/terminal state without rewriting bytes.
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    observation.record_upstream_chunk(&chunk);
                    match decoder.push(&chunk) {
                        Ok(events) => {
                            observation.record_upstream_events(&events);
                            match observe_sse_events(
                                adapter,
                                events,
                                &mut terminal_seen,
                                &observation,
                            ) {
                                Ok(()) => Some((
                                    Ok::<_, io::Error>(chunk),
                                    (source, decoder, terminal_seen, false, observation),
                                )),
                                Err(()) => {
                                    observation.record_upstream_failure();
                                    tokio::task::yield_now().await;
                                    Some((
                                        Err(io::Error::other("upstream SSE stream is invalid")),
                                        (source, decoder, terminal_seen, true, observation),
                                    ))
                                }
                            }
                        }
                        Err(_) => {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
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
                        (source, decoder, terminal_seen, true, observation),
                    ))
                }
                None => match decoder.finish() {
                    Ok(events) => {
                        observation.record_upstream_events(&events);
                        if observe_sse_events(adapter, events, &mut terminal_seen, &observation)
                            .is_err()
                        {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            ));
                        }
                        if !terminal_seen {
                            observation.record_stream_failure(ErrorType::SseEofBeforeTerminal);
                            tracing::warn!(
                                protocol = ?adapter.protocol(),
                                "upstream SSE stream ended before a terminal event"
                            );
                            // Yield one pending poll so already emitted data commits before the body error.
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other(
                                    "upstream SSE stream ended before a terminal event",
                                )),
                                (source, decoder, terminal_seen, true, observation),
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
                            (source, decoder, terminal_seen, true, observation),
                        ))
                    }
                },
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// Classifies one or more fully framed SSE events and updates terminal/failure observation.
fn observe_sse_events(
    adapter: GenerationProviderAdapter,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
    observation: &RequestObservation,
) -> Result<(), ()> {
    // Classify each event through the Provider adapter; record only terminal/failure state, not event content.
    for event in events {
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
