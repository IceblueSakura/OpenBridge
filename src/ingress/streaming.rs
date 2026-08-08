//! Incremental processing of Native and Bridged upstream SSE bodies.

use std::{collections::BTreeMap, io};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use serde_json::{Map, Value};

use crate::{
    bridge::{BridgeStreamRenderer, ResponsesStreamState, StreamTerminal},
    core::ApiProtocol,
    observability::RequestObservation,
    provider::{ProviderAdapter, StreamEventStatus},
    transport::sse::SseDecoder,
};

/// Buffers one bounded Responses SSE body and returns one assembled complete response object.
///
/// The raw SSE byte count is bounded by the JSON response limit in addition to the decoder's
/// per-event limit. No JSON bytes are returned until framing, lifecycle, identities, and an explicit
/// Responses terminal have all been validated.
pub(super) async fn buffer_responses_sse_body(
    body: axum::body::Body,
    max_sse_event_bytes: usize,
    max_body_bytes: usize,
    observation: &RequestObservation,
) -> Result<Bytes, ()> {
    // Initialize bounded framing and typed lifecycle state before consuming upstream bytes.
    let mut source = Box::pin(body.into_data_stream());
    let mut decoder = SseDecoder::new(max_sse_event_bytes);
    let mut state = ResponsesStreamState::new();
    let mut assembler = BufferedResponsesAssembler::default();
    let mut total_bytes = 0_usize;

    // Consume every body chunk without exposing partial data downstream.
    while let Some(chunk) = source.as_mut().next().await {
        let chunk = chunk.map_err(|_| {
            observation.record_upstream_failure();
        })?;
        observation.record_upstream_chunk(&chunk);
        total_bytes = total_bytes.checked_add(chunk.len()).ok_or_else(|| {
            observation.record_upstream_failure();
        })?;
        if total_bytes > max_body_bytes {
            observation.record_upstream_failure();
            return Err(());
        }
        let events = decoder.push(&chunk).map_err(|_| {
            observation.record_upstream_failure();
        })?;
        ingest_buffered_responses_events(&mut state, events, &mut assembler, observation)?;
    }

    // Flush the final event and require one legal typed terminal at normal EOF.
    let events = decoder.finish().map_err(|_| {
        observation.record_upstream_failure();
    })?;
    ingest_buffered_responses_events(&mut state, events, &mut assembler, observation)?;
    state.finish().map_err(|_| {
        observation.record_upstream_failure();
    })?;

    // Assemble one complete response from typed snapshots and item-done events only after a legal terminal.
    let response = assembler.finish(state.terminal().ok_or(())?).map_err(|_| {
        observation.record_upstream_failure();
    })?;
    observation.record_upstream_complete();
    serde_json::to_vec(&response)
        .map(Bytes::from)
        .map_err(|_| ())
}

#[derive(Default)]
/// Bounded in-memory assembler for a Responses stream that must become one JSON response.
struct BufferedResponsesAssembler {
    response: Option<Map<String, Value>>,
    output_items: BTreeMap<u64, Value>,
    terminal_response: Option<Map<String, Value>>,
}

impl BufferedResponsesAssembler {
    /// Captures response snapshots and complete output items after state-machine validation.
    fn ingest(&mut self, value: &Value) -> Result<(), ()> {
        // Select only snapshots needed to reproduce a complete non-streaming response.
        let kind = value.get("type").and_then(Value::as_str).ok_or(())?;
        match kind {
            "response.created" | "response.in_progress" => {
                let response = value.get("response").and_then(Value::as_object).ok_or(())?;
                let assembled = self.response.get_or_insert_with(Map::new);
                for (name, value) in response {
                    assembled.insert(name.clone(), value.clone());
                }
            }
            "response.output_item.done" => {
                let output_index = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .ok_or(())?;
                let item = value.get("item").cloned().ok_or(())?;
                if self.output_items.insert(output_index, item).is_some() {
                    return Err(());
                }
            }
            "response.completed"
            | "response.failed"
            | "response.incomplete"
            | "response.cancelled" => {
                let response = value
                    .get("response")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or(())?;
                if self.terminal_response.replace(response).is_some() {
                    return Err(());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Finishes one legal terminal response, filling sparse ChatGPT snapshots from completed items.
    fn finish(self, terminal: StreamTerminal) -> Result<Value, ()> {
        // Standalone errors have no response object and cannot become a successful JSON response.
        if terminal == StreamTerminal::Error {
            return Err(());
        }

        // Separate the terminal output from in-progress snapshots before merging terminal fields.
        let mut terminal_response = self.terminal_response.ok_or(())?;
        let terminal_output = match terminal_response.remove("output") {
            Some(Value::Array(output)) => output,
            None => Vec::new(),
            Some(_) => return Err(()),
        };
        let mut response = self.response.unwrap_or_default();
        response.remove("output");
        for (name, value) in terminal_response {
            response.insert(name, value);
        }

        // Require the terminal event and response status to describe the same typed outcome.
        let expected_status = match terminal {
            StreamTerminal::Completed => "completed",
            StreamTerminal::Failed => "failed",
            StreamTerminal::Incomplete => "incomplete",
            StreamTerminal::Cancelled => "cancelled",
            StreamTerminal::Error => unreachable!("standalone error returned above"),
        };
        if response.get("status").and_then(Value::as_str) != Some(expected_status) {
            return Err(());
        }
        match response.get("object") {
            Some(Value::String(kind)) if kind == "response" => {}
            None => {
                response.insert("object".to_owned(), Value::String("response".to_owned()));
            }
            Some(_) => return Err(()),
        }

        // Prefer a complete terminal output snapshot, otherwise assemble ordered item-done snapshots.
        let assembled_output = self.output_items.into_values().collect::<Vec<_>>();
        let output = if terminal_output.is_empty() {
            assembled_output
        } else {
            if terminal == StreamTerminal::Completed
                && !assembled_output.is_empty()
                && terminal_output != assembled_output
            {
                return Err(());
            }
            terminal_output
        };
        response.insert("output".to_owned(), Value::Array(output));
        Ok(Value::Object(response))
    }
}

/// Validates framed Responses events and captures bounded snapshots needed for final assembly.
fn ingest_buffered_responses_events(
    state: &mut ResponsesStreamState,
    events: Vec<crate::transport::sse::SseEvent>,
    assembler: &mut BufferedResponsesAssembler,
    observation: &RequestObservation,
) -> Result<(), ()> {
    // Observe and validate every event before retaining bounded response snapshots and completed items.
    observation.record_upstream_events(&events);
    for event in events {
        state.ingest(&event).map_err(|_| {
            observation.record_upstream_failure();
        })?;
        let value: Value = serde_json::from_str(event.data()).map_err(|_| {
            observation.record_upstream_failure();
        })?;
        assembler.ingest(&value).map_err(|_| {
            observation.record_upstream_failure();
        })?;
    }
    Ok(())
}

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
