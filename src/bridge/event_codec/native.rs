//! Same-protocol encoder for a source envelope accepted by the canonical reducer.
//!
//! The envelope is a codec sidecar, like Native JSON fields: it retains unmodeled metadata without
//! inventing portable meaning. The migrated Responses text delta is rewritten from the validated
//! canonical event before framing. Encoding happens only after decode and reduce succeed.

use bytes::Bytes;
use serde_json::{Map, Value};

use super::{
    StaticEventCodecError,
    shared::{parse_object, response_event, sse_data},
};
use crate::{
    core::ApiProtocol,
    ir::generation::{EventLimits, EventState, GenerationEvent, PartDelta},
    transport::sse::SseEvent,
};

pub(super) fn encode(
    protocol: ApiProtocol,
    event: &SseEvent,
    canonical_events: &[GenerationEvent],
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
            apply_responses_text_delta(&kind, &mut payload, canonical_events)?;
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

fn apply_responses_text_delta(
    kind: &str,
    payload: &mut Map<String, Value>,
    canonical_events: &[GenerationEvent],
) -> Result<(), StaticEventCodecError> {
    if kind != "response.output_text.delta" {
        return Ok(());
    }
    let mut text_deltas = canonical_events.iter().filter_map(|event| match event {
        GenerationEvent::PartDelta {
            delta: PartDelta::Text(value),
            ..
        } => Some(value.as_str()),
        _ => None,
    });
    let text = text_deltas
        .next()
        .ok_or(StaticEventCodecError::UnsupportedSemantics)?;
    if text_deltas.next().is_some() {
        return Err(StaticEventCodecError::InvalidLifecycle);
    }
    payload.insert("delta".to_owned(), Value::String(text.to_owned()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ir::generation::{PartId, PartRef, TextValue},
        transport::sse::SseDecoder,
    };
    use serde_json::json;

    fn decode_one(input: &[u8]) -> SseEvent {
        let mut decoder = SseDecoder::new(4096);
        let mut events = decoder.push(input).unwrap();
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    #[test]
    fn responses_text_delta_uses_canonical_value_without_dropping_source_extensions() {
        let limits = EventLimits::new(4096, 4096, 16384).unwrap();
        let source = decode_one(
            br#"id: evt-1
retry: 1250
event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg","output_index":0,"content_index":0,"delta":"source","provider_note":{"keep":true}}

"#,
        );
        let canonical = GenerationEvent::PartDelta {
            part: PartRef::new(PartId::new("msg:text", 128).unwrap()),
            delta: PartDelta::Text(TextValue::new("canonical", 128).unwrap()),
        };

        let encoded = encode(
            ApiProtocol::Responses,
            &source,
            std::slice::from_ref(&canonical),
            &EventState::new(limits),
            limits,
        )
        .unwrap();
        let output = decode_one(&encoded);
        let payload: Value = serde_json::from_str(output.data()).unwrap();

        assert_eq!(payload["delta"], "canonical");
        assert_eq!(payload["provider_note"], json!({"keep": true}));
        assert_eq!(output.id(), Some("evt-1"));
        assert_eq!(output.retry_ms(), Some(1250));

        let mismatch = GenerationEvent::PartDelta {
            part: PartRef::new(PartId::new("msg:refusal", 128).unwrap()),
            delta: PartDelta::Refusal(TextValue::new("canonical", 128).unwrap()),
        };
        assert_eq!(
            encode(
                ApiProtocol::Responses,
                &source,
                &[mismatch],
                &EventState::new(limits),
                limits,
            ),
            Err(StaticEventCodecError::UnsupportedSemantics)
        );
    }
}
