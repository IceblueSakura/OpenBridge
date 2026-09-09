//! Same-protocol encoder for a source envelope accepted by the canonical reducer.
//!
//! The envelope is a codec sidecar, like Native JSON fields: it retains unmodeled metadata without
//! inventing portable meaning. This encoder is called only after decode and reduce have succeeded.

use bytes::Bytes;
use serde_json::Value;

use super::{
    StaticEventCodecError,
    shared::{parse_object, response_event, sse_data},
};
use crate::{
    core::ApiProtocol,
    ir::generation::{EventLimits, EventState},
    transport::sse::SseEvent,
};

pub(super) fn encode(
    protocol: ApiProtocol,
    event: &SseEvent,
    state: &EventState,
    limits: EventLimits,
) -> Result<Bytes, StaticEventCodecError> {
    // A sentinel is emitted only after the reducer accepted the actual terminal.
    if protocol == ApiProtocol::ChatCompletions && event.data() == "[DONE]" {
        if state.terminal().is_none() {
            return Err(StaticEventCodecError::InvalidLifecycle);
        }
        let encoded = Bytes::from_static(b"data: [DONE]\n\n");
        if encoded.len() > limits.max_event_bytes() {
            return Err(StaticEventCodecError::LimitExceeded);
        }
        return Ok(encoded);
    }
    let mut payload = parse_object(event.data(), limits)?;
    let bytes = match protocol {
        ApiProtocol::ChatCompletions => sse_data(&Value::Object(payload), limits)?,
        ApiProtocol::Responses => {
            let kind = payload
                .get("type")
                .and_then(Value::as_str)
                .or(event.event())
                .ok_or(StaticEventCodecError::InvalidJson)?
                .to_owned();
            payload.insert("type".to_owned(), Value::String(kind.clone()));
            if state.terminal().is_some_and(|terminal| {
                terminal.status() == crate::ir::generation::TerminalStatus::Completed
            }) && let Some(response) = payload.get_mut("response").and_then(Value::as_object_mut)
            {
                let sparse = response
                    .get("output")
                    .is_none_or(|value| value.as_array().is_some_and(Vec::is_empty));
                if sparse {
                    let semantic = crate::ir::generation::materialize(state)?;
                    if semantic
                        .candidates()
                        .iter()
                        .any(|candidate| !candidate.output().is_empty())
                    {
                        let output =
                            crate::bridge::static_codec::terminal_output_snapshot(&semantic)
                                .map_err(|_| StaticEventCodecError::UnsupportedSemantics)?;
                        response.insert("output".to_owned(), output);
                    }
                }
            }
            response_event(&kind, &Value::Object(payload), limits)?
        }
    };
    // Preserve standard SSE control fields, not source whitespace, comments, or network fragments.
    let mut output = Vec::new();
    if let Some(id) = event.id() {
        output.extend_from_slice(format!("id: {id}\n").as_bytes());
    }
    if let Some(retry) = event.retry_ms() {
        output.extend_from_slice(format!("retry: {retry}\n").as_bytes());
    }
    output.extend_from_slice(&bytes);
    if output.len() > limits.max_event_bytes() {
        return Err(StaticEventCodecError::LimitExceeded);
    }
    Ok(Bytes::from(output))
}
