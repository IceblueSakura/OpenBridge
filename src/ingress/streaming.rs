//! Incremental processing of Native and Bridged upstream SSE bodies.

use std::io;

use bytes::Bytes;
use futures_util::{StreamExt, stream};

use crate::{
    bridge::BridgeStreamRenderer,
    core::ApiProtocol,
    observability::RequestObservation,
    provider::{ProviderAdapter, StreamEventStatus},
    transport::sse::SseDecoder,
};

/// Incrementally decodes upstream SSE and uses a per-request Bridge renderer to emit target-protocol events.
pub(super) fn bridge_sse_body(
    body: axum::body::Body,
    renderer: BridgeStreamRenderer,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // Keep the source, decoder, and renderer together so downstream drop cancels the upstream body.
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            renderer,
            false,
            observation,
        ),
        move |(mut source, mut decoder, mut renderer, finished, observation)| async move {
            if finished {
                return None;
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    observation.record_upstream_chunk(&chunk);
                    let events = match decoder.push(&chunk) {
                        Ok(events) => events,
                        Err(_) => {
                            observation.record_upstream_failure();
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true, observation),
                            ));
                        }
                    };
                    observation.record_upstream_events(&events);
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                observation.record_upstream_failure();
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true, observation),
                                ));
                            }
                        }
                    }
                    Some((
                        Ok::<_, io::Error>(Bytes::from(output)),
                        (source, decoder, renderer, false, observation),
                    ))
                }
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    {
                        observation.record_upstream_failure();
                        (source, decoder, renderer, true, observation)
                    },
                )),
                None => {
                    let events = match decoder.finish() {
                        Ok(events) => events,
                        Err(_) => {
                            observation.record_upstream_failure();
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true, observation),
                            ));
                        }
                    };
                    observation.record_upstream_events(&events);
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                observation.record_upstream_failure();
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true, observation),
                                ));
                            }
                        }
                    }
                    match renderer.finish() {
                        Ok(bytes) => output.extend_from_slice(&bytes),
                        Err(_) => {
                            observation.record_upstream_failure();
                            return Some((
                                Err(io::Error::other("upstream bridge stream is invalid")),
                                (source, decoder, renderer, true, observation),
                            ));
                        }
                    }
                    observation.record_upstream_complete();
                    if output.is_empty() {
                        None
                    } else {
                        Some((
                            Ok::<_, io::Error>(Bytes::from(output)),
                            (source, decoder, renderer, true, observation),
                        ))
                    }
                }
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// Observes the upstream SSE lifecycle without rewriting the original bytes.
///
/// The decoder handles UTF-8/SSE framing across network chunks and delegates protocol-terminal
/// detection to the Provider adapter. A clean EOF without a terminal preserves received bytes and
/// records a warning; invalid framing, invalid UTF-8, or an upstream body error closes with a stream
/// error. When downstream drops the body, `source` is dropped as well, cancelling the reqwest stream.
pub(super) fn validate_sse_body(
    body: axum::body::Body,
    protocol: ApiProtocol,
    adapter: ProviderAdapter,
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
                                protocol,
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
                                    Some((
                                        Err(io::Error::other("upstream SSE stream is invalid")),
                                        (source, decoder, terminal_seen, true, observation),
                                    ))
                                }
                            }
                        }
                        Err(_) => {
                            observation.record_upstream_failure();
                            Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            ))
                        }
                    }
                }
                Some(Err(_)) => {
                    observation.record_upstream_failure();
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
                        if observe_sse_events(
                            adapter,
                            protocol,
                            events,
                            &mut terminal_seen,
                            &observation,
                        )
                            .is_err()
                        {
                            observation.record_upstream_failure();
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            ));
                        }
                        observation.record_upstream_complete();
                        if !terminal_seen {
                            observation.record_stream_failure("sse_eof_before_terminal");
                            tracing::warn!(
                                ?protocol,
                                "upstream SSE stream ended before a terminal event"
                            );
                        }
                        None
                    }
                    Err(_) => {
                        observation.record_upstream_failure();
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
    adapter: ProviderAdapter,
    protocol: ApiProtocol,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
    observation: &RequestObservation,
) -> Result<(), ()> {
    // Classify each event through the Provider adapter; record only terminal/failure state, not event content.
    for event in events {
        let decoded = adapter
            .classify_sse_event(protocol, event)
            .map_err(|_| ())?;
        match decoded.status() {
            StreamEventStatus::Continue => {}
            StreamEventStatus::Completed => *terminal_seen = true,
            StreamEventStatus::Failed => {
                *terminal_seen = true;
                observation.record_stream_failure("provider_terminal_failed");
            }
        }
    }
    Ok(())
}
