//! Semantic equality for Native SSE fixtures whose framing may be normalized.

use openbridge::transport::sse::SseDecoder;
use serde_json::Value;

type Frame = (Option<String>, Option<String>, Option<u64>, Value);

fn frames(bytes: &[u8]) -> Option<Vec<Frame>> {
    let mut decoder = SseDecoder::new(1024 * 1024);
    let mut events = decoder.push(bytes).ok()?;
    events.extend(decoder.finish().ok()?);
    let mut completed_items = std::collections::BTreeMap::new();
    events
        .into_iter()
        .map(|event| {
            let mut value = if event.data() == "[DONE]" {
                Value::String("[DONE]".to_owned())
            } else {
                serde_json::from_str::<Value>(event.data()).ok()?
            };
            let kind = event
                .event()
                .map(str::to_owned)
                .or_else(|| value.get("type").and_then(Value::as_str).map(str::to_owned));
            if let (Some(kind), Some(object)) = (kind.as_ref(), value.as_object_mut()) {
                object
                    .entry("type")
                    .or_insert_with(|| Value::String(kind.clone()));
            }
            // A sparse terminal may be completed only from preceding authoritative item snapshots.
            if kind.as_deref() == Some("response.output_item.done") {
                completed_items.insert(
                    value.get("output_index")?.as_u64()?,
                    value.get("item")?.clone(),
                );
            }
            if kind.as_deref() == Some("response.completed")
                && !completed_items.is_empty()
                && let Some(response) = value.get_mut("response").and_then(Value::as_object_mut)
                && response
                    .get("output")
                    .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
            {
                response.insert(
                    "output".to_owned(),
                    Value::Array(completed_items.values().cloned().collect()),
                );
            }
            Some((kind, event.id().map(str::to_owned), event.retry_ms(), value))
        })
        .collect()
}

/// Compares ordered fields, payloads and terminal sentinels without depending on JSON whitespace.
pub fn equivalent(actual: &[u8], expected: &[u8]) -> bool {
    match (frames(actual), frames(expected)) {
        (Some(actual), Some(expected)) => actual == expected,
        _ => false,
    }
}
