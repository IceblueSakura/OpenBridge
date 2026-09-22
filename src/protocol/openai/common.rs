use super::{CodecError, Profile};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::{task::generation::*, value::Text},
};
use serde_json::{Map, Value};
use std::io::{self, Write};

// Count serialization without allocating another full wire buffer.
pub(super) fn bounded(v: &Value) -> Result<(), CodecError> {
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0 = self.0.saturating_add(b.len());
            if self.0 > MAX_TOTAL_BYTES {
                return Err(io::Error::other("wire limit"));
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Counter(0), v).map_err(|_| CodecError::Limit)
}
pub(super) fn object(v: &Value) -> Result<&Map<String, Value>, CodecError> {
    v.as_object().ok_or(CodecError::Invalid("object"))
}
pub(super) fn fields(o: &Map<String, Value>, allowed: &[&str]) -> Result<(), CodecError> {
    if let Some(key) = o.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(CodecError::Unsupported(key.clone()));
    }
    Ok(())
}
pub(super) fn string<'a>(
    o: &'a Map<String, Value>,
    key: &'static str,
) -> Result<&'a str, CodecError> {
    o.get(key)
        .and_then(Value::as_str)
        .ok_or(CodecError::Invalid(key))
}
pub(super) fn text(value: &str, kind: &'static str, max: usize) -> Result<Text, CodecError> {
    Text::new(value, kind, max).map_err(|_| CodecError::Invalid(kind))
}
pub(super) fn optional_bool(
    o: &Map<String, Value>,
    key: &'static str,
) -> Result<Option<bool>, CodecError> {
    o.get(key)
        .map(|v| v.as_bool().ok_or(CodecError::Invalid(key)))
        .transpose()
}
pub(super) fn controls(
    o: &Map<String, Value>,
    key: &'static str,
) -> Result<GenerationControls, CodecError> {
    let mut c = GenerationControls::default();
    c.max_output_tokens = o
        .get(key)
        .map(|v| {
            v.as_u64()
                .filter(|n| *n > 0)
                .ok_or(CodecError::Invalid(key))
        })
        .transpose()?;
    if let Some(t) = o.get("temperature") {
        c = c.with_temperature(t.as_f64().ok_or(CodecError::Invalid("temperature"))?)?;
    }
    Ok(c)
}
pub(super) fn write_controls(r: &GenerationRequest, o: &mut Map<String, Value>, key: &str) {
    if let Some(t) = r.controls().temperature() {
        o.insert("temperature".into(), Value::from(t));
    }
    if let Some(n) = r.controls().max_output_tokens {
        o.insert(key.into(), Value::from(n));
    }
}
#[derive(Default)]
pub(super) struct Items {
    pub items: Vec<(ItemId, Item)>,
    pub fidelity: FidelityRecords,
    next_item: u64,
    next_part: u64,
}
impl Items {
    pub fn id(&mut self) -> Result<ItemId, CodecError> {
        if self.next_item as usize >= MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        self.next_item += 1;
        Ok(ItemId::new(self.next_item))
    }
    pub fn part(&mut self, s: &str) -> Result<Part, CodecError> {
        self.next_part += 1;
        if self.next_part as usize > MAX_ITEMS {
            return Err(CodecError::Limit);
        }
        Ok(Part {
            id: PartId::new(self.next_part),
            content: ContentPart::Text(
                Text::allowing_empty(s, "text", MAX_TEXT_BYTES).map_err(|_| CodecError::Limit)?,
            ),
        })
    }
    pub fn record_id(&mut self, id: ItemId, o: &Map<String, Value>) -> Result<(), CodecError> {
        if let Some(v) = o.get("id") {
            self.fidelity
                .record_response_item_id(id, v.as_str().ok_or(CodecError::Invalid("item id"))?)?;
        }
        Ok(())
    }
}
pub(super) fn raw_string(o: &Map<String, Value>, key: &'static str) -> Result<String, CodecError> {
    let s = string(o, key)?;
    if s.len() > MAX_TEXT_BYTES {
        return Err(CodecError::Limit);
    }
    Ok(s.to_owned())
}
pub(super) fn strict_default(profile: Profile) -> StrictDefault {
    match profile {
        Profile::Chat => StrictDefault::NonStrict,
        Profile::Responses => StrictDefault::NormalizeSchema,
    }
}
pub(super) fn tool_call(
    o: &Map<String, Value>,
    profile: Profile,
    message: Option<ItemId>,
) -> Result<ToolCall, CodecError> {
    let (f, id) = match profile {
        Profile::Chat => {
            fields(o, &["id", "type", "function"])?;
            if string(o, "type")? != "function" {
                return Err(CodecError::Unsupported("tool kind".into()));
            }
            let f = object(o.get("function").ok_or(CodecError::Invalid("function"))?)?;
            fields(f, &["name", "arguments"])?;
            (f, string(o, "id")?)
        }
        Profile::Responses => {
            fields(o, &["id", "type", "call_id", "name", "arguments", "status"])?;
            completed_item(o)?;
            (o, string(o, "call_id")?)
        }
    };
    Ok(ToolCall {
        call_id: text(id, "call_id", 256)?,
        name: text(string(f, "name")?, "function name", 128)?,
        arguments: raw_string(f, "arguments")?,
        message,
    })
}
pub(super) fn completed_item(o: &Map<String, Value>) -> Result<(), CodecError> {
    if let Some(status) = o.get("status")
        && status.as_str() != Some("completed")
    {
        return Err(CodecError::Unsupported("non-completed item".into()));
    }
    Ok(())
}
