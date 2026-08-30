//! Bounded Responses SSE buffering through canonical Event IR.

use super::*;

/// Consumes one complete Responses SSE body and materializes its canonical terminal response.
pub(in crate::ingress) async fn buffer_responses_sse_body(
    body: axum::body::Body,
    mut renderer: BridgeStreamRenderer,
    max_sse_event_bytes: usize,
    max_body_bytes: usize,
    observation: &RequestObservation,
) -> Result<crate::ir::generation::GenerationResponse, ()> {
    let mut source = Box::pin(body.into_data_stream());
    let mut decoder = SseDecoder::new(max_sse_event_bytes);
    let mut total_bytes = 0_usize;

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
        observation.record_upstream_events(&events);
        for event in events {
            renderer.render(event).map_err(|_| {
                observation.record_bridge_failure();
            })?;
        }
    }

    let events = decoder.finish().map_err(|_| {
        observation.record_upstream_failure();
    })?;
    observation.record_upstream_events(&events);
    for event in events {
        renderer.render(event).map_err(|_| {
            observation.record_bridge_failure();
        })?;
    }
    renderer.finish().map_err(|_| {
        observation.record_bridge_failure();
    })?;
    let response = renderer.materialized_response().map_err(|_| {
        observation.record_bridge_failure();
    })?;
    observation.record_upstream_complete();
    Ok(response)
}
