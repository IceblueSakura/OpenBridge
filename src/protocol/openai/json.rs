//! Raw JSON admission shared by complete Responses envelopes and SSE payloads.
use super::CodecError;
use crate::semantic::task::generation::MAX_TOTAL_BYTES;
use serde::{
    Deserialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use std::fmt;

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 65_536;

/// Bound the raw input before parsing; never expose parser diagnostics containing wire data.
pub(super) fn decode(input: &[u8]) -> Result<Value, CodecError> {
    if input.len() > MAX_TOTAL_BYTES {
        return Err(CodecError::Limit);
    }
    let mut budget = Budget {
        nodes: 0,
        exhausted: false,
    };
    let mut parser = serde_json::Deserializer::from_slice(input);
    let result = Seed {
        budget: &mut budget,
        depth: 0,
    }
    .deserialize(&mut parser)
    .and_then(|value| {
        parser.end()?;
        Ok(value)
    });
    result.map_err(|_| {
        if budget.exhausted {
            CodecError::Limit
        } else {
            CodecError::Invalid("JSON")
        }
    })
}

struct Budget {
    nodes: usize,
    exhausted: bool,
}
impl Budget {
    fn charge<E: de::Error>(&mut self) -> Result<(), E> {
        if self.nodes == MAX_NODES {
            self.exhausted = true;
            return Err(E::custom("JSON limit"));
        }
        self.nodes += 1;
        Ok(())
    }
}
struct Key<'a>(&'a mut Budget);
impl<'de> DeserializeSeed<'de> for Key<'_> {
    type Value = String;
    fn deserialize<D: de::Deserializer<'de>>(self, parser: D) -> Result<String, D::Error> {
        self.0.charge()?;
        String::deserialize(parser)
    }
}
struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}
impl Seed<'_> {
    fn container<E: de::Error>(&mut self) -> Result<(), E> {
        if self.depth >= MAX_DEPTH {
            self.budget.exhausted = true;
            return Err(E::custom("JSON depth"));
        }
        Ok(())
    }
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, parser: D) -> Result<Value, D::Error> {
        self.budget.charge()?;
        parser.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> Result<Value, A::Error> {
        self.container()?;
        // No size-hint allocation: each child is charged before its value is built.
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed {
            budget: &mut *self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<Value, A::Error> {
        self.container()?;
        let mut values = Map::new();
        while let Some(key) = map.next_key_seed(Key(&mut *self.budget))? {
            // Compare decoded keys before reading/inserting their values; no overwrite is allowed.
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON key"));
            }
            let value = map.next_value_seed(Seed {
                budget: &mut *self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::task::generation::MAX_TOTAL_BYTES;
    use serde_json::json;

    #[test]
    fn duplicate_keys_are_compared_after_unescaping_at_every_level() {
        for input in [
            r#"{"a":1,"a":2}"#,
            r#"{"a":1,"\u0061":2}"#,
            r#"{"items":[{"a":1,"a":1}]}"#,
            r#"{"outer":{"a":null,"a":false}}"#,
        ] {
            assert_eq!(decode(input.as_bytes()), Err(CodecError::Invalid("JSON")));
        }
        assert_eq!(
            decode(br#"{"a":{"x":1},"b":{"x":2}}"#).unwrap(),
            json!({"a":{"x":1},"b":{"x":2}})
        );
        // Embedded tool arguments remain opaque text, not a second JSON document to parse.
        assert_eq!(
            decode(br#""{\"a\":1,\"a\":2}""#).unwrap(),
            json!("{\"a\":1,\"a\":2}")
        );
    }

    #[test]
    fn malformed_utf8_json_and_trailing_data_are_rejected_without_echoing_payload() {
        for input in [
            b"\xff".as_slice(),
            b"{} []",
            b"null x",
            b"{\"a\":}",
            b"[1,]",
            b"1e9999",
            b"\xef\xbb\xbf{}",
        ] {
            let error = decode(input).unwrap_err();
            assert_eq!(error, CodecError::Invalid("JSON"));
            assert_eq!(error.to_string(), "invalid JSON");
        }
        assert_eq!(
            decode(b" \r\n {\"n\":18446744073709551615,\"text\":\"\\u4f60\\u597d\"} \t").unwrap(),
            json!({"n":u64::MAX,"text":"你好"})
        );
        assert_eq!(
            decode(b"[-9223372036854775808,1.25,-0.0,true,false,null]").unwrap(),
            json!([i64::MIN, 1.25, -0.0, true, false, null])
        );
    }

    #[test]
    fn raw_byte_budget_counts_whitespace_before_parsing() {
        let mut input = vec![b' '; MAX_TOTAL_BYTES];
        input[..4].copy_from_slice(b"null");
        assert_eq!(decode(&input).unwrap(), Value::Null);
        input.push(b' ');
        assert_eq!(decode(&input), Err(CodecError::Limit));
    }

    #[test]
    fn container_depth_is_bounded_before_building_nested_values() {
        for (open, close) in [("[", "]"), ("{\"child\":", "}")] {
            for (depth, accepted) in [(64, true), (65, false)] {
                let input = format!("{}0{}", open.repeat(depth), close.repeat(depth));
                let result = decode(input.as_bytes());
                if accepted {
                    assert!(result.is_ok());
                } else {
                    assert_eq!(result.err(), Some(CodecError::Limit));
                }
            }
        }
    }

    #[test]
    fn node_budget_counts_values_and_object_keys() {
        let input = format!("[{}]", vec!["0"; 65_535].join(","));
        assert!(decode(input.as_bytes()).is_ok());
        let input = format!("[{}]", vec!["0"; 65_536].join(","));
        assert_eq!(decode(input.as_bytes()).err(), Some(CodecError::Limit));
        for (entries, accepted) in [(32_767, true), (32_768, false)] {
            let input = format!(
                "{{{}}}",
                (0..entries)
                    .map(|i| format!("\"k{i}\":0"))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let result = decode(input.as_bytes());
            if accepted {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.err(), Some(CodecError::Limit));
            }
        }
    }
}
