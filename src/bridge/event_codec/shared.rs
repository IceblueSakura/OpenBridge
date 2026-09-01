//! Shared bounded wire helpers for Event IR codecs.

use bytes::Bytes;
use serde_json::{Map, Value};

use crate::ir::generation::{
    CallId, CandidateId, EventEnvelope, EventLimits, GenerationEvent, ItemId, PartId, ResponseId,
    Sequence, TextValue, ToolName, Usage,
};

use super::StaticEventCodecError;

pub(super) fn parse_object(
    data: &str,
    limits: EventLimits,
) -> Result<Map<String, Value>, StaticEventCodecError> {
    if data.len() > limits.max_event_bytes() {
        return Err(StaticEventCodecError::LimitExceeded);
    }
    super::super::strict_json::from_str(data)
        .map_err(|_| StaticEventCodecError::InvalidJson)?
        .as_object()
        .cloned()
        .ok_or(StaticEventCodecError::InvalidJson)
}

pub(super) fn required_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, StaticEventCodecError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(StaticEventCodecError::InvalidJson)
}

pub(super) fn required_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, StaticEventCodecError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(StaticEventCodecError::InvalidJson)
}

pub(super) fn text(value: &str, limits: EventLimits) -> Result<TextValue, StaticEventCodecError> {
    TextValue::new(value, limits.max_event_bytes())
        .map_err(|_| StaticEventCodecError::LimitExceeded)
}

macro_rules! bounded_identity {
    ($name:ident, $type:ty) => {
        pub(super) fn $name(
            value: impl Into<String>,
            limits: EventLimits,
        ) -> Result<$type, StaticEventCodecError> {
            <$type>::new(value, limits.max_event_bytes())
                .map_err(|_| StaticEventCodecError::LimitExceeded)
        }
    };
}

bounded_identity!(response_id, ResponseId);
bounded_identity!(candidate_id, CandidateId);
bounded_identity!(item_id, ItemId);
bounded_identity!(part_id, PartId);
bounded_identity!(call_id, CallId);
bounded_identity!(tool_name, ToolName);

pub(super) fn envelope(
    next_sequence: &mut u64,
    event: GenerationEvent,
) -> Result<EventEnvelope, StaticEventCodecError> {
    let sequence = *next_sequence;
    *next_sequence = next_sequence
        .checked_add(1)
        .ok_or(StaticEventCodecError::InvalidLifecycle)?;
    Ok(EventEnvelope::new(Sequence::new(sequence), event))
}

pub(super) fn map_id(id: &str, from: &str, to: &str) -> String {
    format!("{to}{}", id.strip_prefix(from).unwrap_or(id))
}

pub(super) fn bridge_item_id(call_id: &str) -> String {
    call_id
        .strip_prefix("call_")
        .map(|suffix| format!("fc_{suffix}"))
        .unwrap_or_else(|| format!("fc_{call_id}"))
}

pub(super) fn usage_from_chat(usage: &Map<String, Value>) -> Result<Usage, StaticEventCodecError> {
    let input = required_usage_u64(usage, "prompt_tokens")?;
    let output = required_usage_u64(usage, "completion_tokens")?;
    let total = required_usage_u64(usage, "total_tokens")?;
    if input.checked_add(output) != Some(total) {
        return Err(StaticEventCodecError::InvalidJson);
    }
    Ok(Usage::new(
        Some(input),
        Some(output),
        Some(total),
        optional_nullable_detail_object(usage, "completion_tokens_details")?
            .map(|details| optional_u64(details, "reasoning_tokens"))
            .transpose()?
            .flatten(),
        optional_nullable_detail_object(usage, "prompt_tokens_details")?
            .map(|details| optional_u64(details, "cached_tokens"))
            .transpose()?
            .flatten(),
    ))
}

pub(super) fn usage_from_responses(
    usage: &Map<String, Value>,
) -> Result<Usage, StaticEventCodecError> {
    let input = required_usage_u64(usage, "input_tokens")?;
    let output = required_usage_u64(usage, "output_tokens")?;
    let total = required_usage_u64(usage, "total_tokens")?;
    if input.checked_add(output) != Some(total) {
        return Err(StaticEventCodecError::InvalidJson);
    }
    Ok(Usage::new(
        Some(input),
        Some(output),
        Some(total),
        optional_detail_object(usage, "output_tokens_details")?
            .map(|details| optional_u64(details, "reasoning_tokens"))
            .transpose()?
            .flatten(),
        optional_detail_object(usage, "input_tokens_details")?
            .map(|details| optional_u64(details, "cached_tokens"))
            .transpose()?
            .flatten(),
    ))
}

fn required_usage_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, StaticEventCodecError> {
    optional_u64(object, field)?.ok_or(StaticEventCodecError::InvalidJson)
}

fn optional_detail_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a Map<String, Value>>, StaticEventCodecError> {
    match object.get(field) {
        None => Ok(None),
        Some(value) => value
            .as_object()
            .map(Some)
            .ok_or(StaticEventCodecError::InvalidJson),
    }
}

fn optional_nullable_detail_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a Map<String, Value>>, StaticEventCodecError> {
    if object.get(field).is_some_and(Value::is_null) {
        Ok(None)
    } else {
        optional_detail_object(object, field)
    }
}

fn optional_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, StaticEventCodecError> {
    object
        .get(field)
        .filter(|value| !value.is_null())
        .map(|value| value.as_u64().ok_or(StaticEventCodecError::InvalidJson))
        .transpose()
}

pub(super) fn sse_data(value: &Value, limits: EventLimits) -> Result<Bytes, StaticEventCodecError> {
    let data = serde_json::to_vec(value).map_err(|_| StaticEventCodecError::InvalidJson)?;
    let mut output = b"data: ".to_vec();
    output.extend(data);
    output.extend_from_slice(b"\n\n");
    if output.len() > limits.max_event_bytes() {
        return Err(StaticEventCodecError::LimitExceeded);
    }
    Ok(Bytes::from(output))
}

pub(super) fn response_event(
    event: &str,
    value: &Value,
    limits: EventLimits,
) -> Result<Bytes, StaticEventCodecError> {
    let data = serde_json::to_vec(value).map_err(|_| StaticEventCodecError::InvalidJson)?;
    let mut output = format!("event: {event}\n").into_bytes();
    output.extend_from_slice(b"data: ");
    output.extend(data);
    output.extend_from_slice(b"\n\n");
    if output.len() > limits.max_event_bytes() {
        return Err(StaticEventCodecError::LimitExceeded);
    }
    Ok(Bytes::from(output))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn chat_usage_treats_null_detail_objects_as_absent() {
        let usage = json!({
            "prompt_tokens": 8,
            "completion_tokens": 4,
            "total_tokens": 12,
            "prompt_tokens_details": null,
            "completion_tokens_details": null
        });

        assert_eq!(
            usage_from_chat(usage.as_object().expect("fixture must be an object"))
                .expect("nullable optional Chat details must decode"),
            Usage::new(Some(8), Some(4), Some(12), None, None)
        );
    }
}
