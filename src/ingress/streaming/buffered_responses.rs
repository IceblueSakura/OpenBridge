//! Bounded Responses SSE-to-JSON assembly.

use super::*;

/// Responses terminal have all been validated.
pub(in crate::ingress) async fn buffer_responses_sse_body(
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
