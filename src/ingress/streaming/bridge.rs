//! Incremental protocol Bridge SSE rendering.

use super::*;

/// Incrementally decodes upstream SSE and uses a per-request Bridge renderer to emit target-protocol events.
pub(in crate::ingress) fn bridge_sse_body(
    body: axum::body::Body,
    renderer: BridgeStreamRenderer,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // Keep the source, decoder, renderer, and deferred error together so drop cancels upstream work.
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            renderer,
            false,
            false,
            observation,
        ),
        move |(mut source, mut decoder, mut renderer, pending_error, finished, observation)| async move {
            if finished {
                return None;
            }
            if pending_error {
                tokio::task::yield_now().await;
                return Some((
                    Err(io::Error::other("upstream bridge stream is invalid")),
                    (source, decoder, renderer, false, true, observation),
                ));
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    observation.record_upstream_chunk(&chunk);
                    let events = match decoder.push(&chunk) {
                        Ok(events) => events,
                        Err(_) => {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, false, true, observation),
                            ));
                        }
                    };
                    observation.record_upstream_events(&events);
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                observation.record_bridge_failure();
                                if !output.is_empty() {
                                    return Some((
                                        Ok::<_, io::Error>(Bytes::from(output)),
                                        (source, decoder, renderer, true, false, observation),
                                    ));
                                }
                                tokio::task::yield_now().await;
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, false, true, observation),
                                ));
                            }
                        }
                    }
                    Some((
                        Ok::<_, io::Error>(Bytes::from(output)),
                        (source, decoder, renderer, false, false, observation),
                    ))
                }
                Some(Err(_)) => {
                    observation.record_upstream_failure();
                    tokio::task::yield_now().await;
                    Some((
                        Err(io::Error::other(
                            "upstream SSE stream terminated unexpectedly",
                        )),
                        (source, decoder, renderer, false, true, observation),
                    ))
                }
                None => {
                    let events = match decoder.finish() {
                        Ok(events) => events,
                        Err(_) => {
                            observation.record_upstream_failure();
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, false, true, observation),
                            ));
                        }
                    };
                    observation.record_upstream_events(&events);
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                observation.record_bridge_failure();
                                if !output.is_empty() {
                                    return Some((
                                        Ok::<_, io::Error>(Bytes::from(output)),
                                        (source, decoder, renderer, true, false, observation),
                                    ));
                                }
                                tokio::task::yield_now().await;
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, false, true, observation),
                                ));
                            }
                        }
                    }
                    match renderer.finish() {
                        Ok(bytes) => output.extend_from_slice(&bytes),
                        Err(_) => {
                            observation.record_bridge_failure();
                            if !output.is_empty() {
                                return Some((
                                    Ok::<_, io::Error>(Bytes::from(output)),
                                    (source, decoder, renderer, true, false, observation),
                                ));
                            }
                            tokio::task::yield_now().await;
                            return Some((
                                Err(io::Error::other("upstream bridge stream is invalid")),
                                (source, decoder, renderer, false, true, observation),
                            ));
                        }
                    }
                    observation.record_upstream_complete();
                    if output.is_empty() {
                        None
                    } else {
                        Some((
                            Ok::<_, io::Error>(Bytes::from(output)),
                            (source, decoder, renderer, false, true, observation),
                        ))
                    }
                }
            }
        },
    );
    axum::body::Body::from_stream(stream)
}
