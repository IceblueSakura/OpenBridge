//! Validated static Generation leaf values.

use std::io::{self, Write};

use serde_json::{Map, Value};
use thiserror::Error;

pub(in crate::ir::generation) fn encoded_json_len(
    value: &Value,
    max_bytes: usize,
) -> Option<usize> {
    struct Counter {
        written: usize,
        max_bytes: usize,
    }

    impl Write for Counter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let Some(next) = self.written.checked_add(buffer.len()) else {
                return Err(io::Error::other("JSON size overflow"));
            };
            if next > self.max_bytes {
                return Err(io::Error::other("JSON exceeds configured bound"));
            }
            self.written = next;
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut counter = Counter {
        written: 0,
        max_bytes,
    };
    serde_json::to_writer(&mut counter, value)
        .ok()
        .map(|()| counter.written)
}

/// Validation failure for one static Generation IR value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValidationError {
    /// A bounded text value is empty.
    #[error("text must not be empty")]
    EmptyText,
    /// A bounded text value exceeds its caller-supplied byte limit.
    #[error("text exceeds the {max_bytes}-byte limit")]
    TextTooLarge {
        /// Maximum accepted UTF-8 bytes.
        max_bytes: usize,
    },
    /// A message contains no semantic content.
    #[error("message must contain at least one content part")]
    EmptyMessage,
    /// Ordered history contains a duplicate canonical item identity.
    #[error("duplicate input item identity '{id}'")]
    DuplicateInputItemId {
        /// Duplicate item identity.
        id: String,
    },
    /// Ordered history contains a duplicate canonical tool-call identity.
    #[error("duplicate prior tool-call identity '{id}'")]
    DuplicateInputCallId {
        /// Duplicate tool-call identity.
        id: String,
    },
    /// A schema is not represented by a JSON object.
    #[error("JSON Schema must be an object")]
    InvalidJsonSchema,
    /// A schema exceeds its caller-supplied encoded byte limit.
    #[error("JSON Schema exceeds the {max_bytes}-byte limit")]
    JsonSchemaTooLarge {
        /// Maximum accepted encoded JSON bytes.
        max_bytes: usize,
    },
    /// Two tool definitions use the same canonical name.
    #[error("duplicate tool name '{name}'")]
    DuplicateToolName {
        /// Duplicate canonical tool name.
        name: String,
    },
    /// A required or named tool choice is configured without matching tools.
    #[error("tool choice does not match the configured tools")]
    InvalidToolChoice,
    /// Parallel tool calls are enabled without a standard function tool.
    #[error("parallel tool calls require at least one function tool")]
    ParallelToolsWithoutFunction,
    /// Tool arguments or structured payload are not represented by a JSON object.
    #[error("tool JSON value must be an object")]
    InvalidJsonObject,
    /// A JSON object exceeds its encoded byte bound.
    #[error("tool JSON exceeds the {max_bytes}-byte limit")]
    JsonObjectTooLarge {
        /// Maximum accepted encoded JSON bytes.
        max_bytes: usize,
    },
}

/// Non-empty UTF-8 text checked against an owning boundary's byte limit.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TextValue(String);

impl TextValue {
    /// Creates text after validating non-emptiness and the supplied UTF-8 byte limit.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ValidationError::EmptyText);
        }
        if value.len() > max_bytes {
            return Err(ValidationError::TextTooLarge { max_bytes });
        }
        Ok(Self(value))
    }

    /// Returns the validated text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-empty bounded canonical tool name.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ToolName(String);

impl ToolName {
    /// Creates a tool name using the same non-empty byte-bounded contract as text.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, ValidationError> {
        TextValue::new(value, max_bytes).map(|value| Self(value.0))
    }

    /// Returns the canonical tool name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated bounded JSON object used by completed function/server tool values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonObject(Map<String, Value>);

impl JsonObject {
    /// Creates an object after validating its top-level shape and encoded size.
    pub fn new(value: Value, max_bytes: usize) -> Result<Self, ValidationError> {
        let object = value
            .as_object()
            .ok_or(ValidationError::InvalidJsonObject)?;
        if encoded_json_len(&value, max_bytes).is_none() {
            return Err(ValidationError::JsonObjectTooLarge { max_bytes });
        }
        Ok(Self(object.clone()))
    }

    /// Returns the validated object.
    pub const fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }
}

/// Bounded JSON Schema object with validated top-level shape.
///
/// JSON Schema permits unknown extension keywords, so R1 does not pretend to validate a Target's
/// supported keyword subset. Capability/lowering owns that check before encoding.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonSchema {
    value: Value,
    encoded_len: usize,
}

impl JsonSchema {
    /// Creates a JSON Schema after validating its object shape and encoded size.
    pub fn new(value: Value, max_bytes: usize) -> Result<Self, ValidationError> {
        if !value.is_object() {
            return Err(ValidationError::InvalidJsonSchema);
        }
        let encoded_len = encoded_json_len(&value, max_bytes)
            .ok_or(ValidationError::JsonSchemaTooLarge { max_bytes })?;
        Ok(Self { value, encoded_len })
    }

    /// Returns the validated JSON Schema value.
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    /// Returns the canonical compact-JSON byte length used by semantic limits.
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }
}

impl Eq for JsonSchema {}
