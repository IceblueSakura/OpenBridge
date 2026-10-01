//! Closed tool result values. Error reporting never changes a generation outcome.
use super::{CallContext, GenerationError, ItemLifecycle, MAX_TEXT_BYTES, PartId, Resource};
use crate::semantic::value::Text;
use serde_json::Value;

/// Structured result authority, not a parsed view over a second string payload.
/// This bounded value does not imply Schema adherence or a protocol carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredToolOutput(Value);
impl StructuredToolOutput {
    pub fn new(value: Value) -> Result<Self, GenerationError> {
        let mut nodes = 0;
        structure(&value, 0, &mut nodes)?;
        crate::semantic::value::json_size(&value, MAX_TEXT_BYTES)
            .map_err(|_| GenerationError::Limit)?;
        Ok(Self(value))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    pub(crate) fn bytes(&self) -> Result<usize, GenerationError> {
        crate::semantic::value::json_size(&self.0, MAX_TEXT_BYTES)
            .map_err(|_| GenerationError::Limit)
    }
}
fn structure(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), GenerationError> {
    *nodes += 1;
    if depth > 32 || *nodes > 8192 {
        return Err(GenerationError::Limit);
    }
    match value {
        Value::Array(values) => {
            for value in values {
                structure(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                *nodes += 1; // Object keys consume the same bounded node budget.
                structure(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolExecutionError {
    pub code: Option<Text>,
    pub message: Text,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolResultPart {
    Text(Text),
    Resource(Resource),
}
impl ToolResultPart {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Resource(_) => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolOutput {
    Text(String),
    Parts(Vec<(PartId, ToolResultPart)>),
    Structured(StructuredToolOutput),
    Error(ToolExecutionError),
}
impl ToolOutput {
    pub fn is_text_only(&self) -> bool {
        match self {
            Self::Text(_) => true,
            Self::Parts(parts) => parts
                .iter()
                .all(|(_, part)| matches!(part, ToolResultPart::Text(_))),
            Self::Structured(_) | Self::Error(_) => false,
        }
    }
}
impl From<String> for ToolOutput {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<&str> for ToolOutput {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolResult {
    pub call_id: Text,
    pub output: ToolOutput,
    /// Artifact lifecycle, not tool success; absent remains unreported.
    pub status: Option<ItemLifecycle>,
    pub context: CallContext,
}
