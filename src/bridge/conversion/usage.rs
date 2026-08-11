//! Strict token-usage projection shared by non-streaming and streaming Responses-to-Chat conversion.

use serde_json::{Map, Value};

use super::BridgeError;

/// Maps one complete Responses usage object to the Chat Completions token vocabulary.
pub(super) fn responses_usage_to_chat(source: &Map<String, Value>) -> Result<Value, BridgeError> {
    // Require the three complete non-negative counters; partial or estimated usage is not emitted.
    let mut target = Map::new();
    target.insert(
        "prompt_tokens".to_owned(),
        Value::from(required_count(source, "input_tokens")?),
    );
    target.insert(
        "completion_tokens".to_owned(),
        Value::from(required_count(source, "output_tokens")?),
    );
    target.insert(
        "total_tokens".to_owned(),
        Value::from(required_count(source, "total_tokens")?),
    );

    // Preserve the two interoperable detail counters and ignore Provider-only detail extensions.
    if let Some(details) = optional_details(source, "input_tokens_details")?
        && let Some(cached_tokens) = optional_count(details, "cached_tokens")?
    {
        target.insert(
            "prompt_tokens_details".to_owned(),
            Value::Object(Map::from_iter([(
                "cached_tokens".to_owned(),
                Value::from(cached_tokens),
            )])),
        );
    }
    if let Some(details) = optional_details(source, "output_tokens_details")?
        && let Some(reasoning_tokens) = optional_count(details, "reasoning_tokens")?
    {
        target.insert(
            "completion_tokens_details".to_owned(),
            Value::Object(Map::from_iter([(
                "reasoning_tokens".to_owned(),
                Value::from(reasoning_tokens),
            )])),
        );
    }

    Ok(Value::Object(target))
}

/// Reads one required non-negative integer counter.
fn required_count(source: &Map<String, Value>, field: &str) -> Result<u64, BridgeError> {
    source
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(BridgeError::InvalidShape)
}

/// Reads an optional detail object while treating explicit null as absent.
fn optional_details<'a>(
    source: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a Map<String, Value>>, BridgeError> {
    match source.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(details)) => Ok(Some(details)),
        Some(_) => Err(BridgeError::InvalidShape),
    }
}

/// Reads one optional known detail counter and rejects a malformed known value.
fn optional_count(source: &Map<String, Value>, field: &str) -> Result<Option<u64>, BridgeError> {
    match source.get(field) {
        None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or(BridgeError::InvalidShape),
    }
}
