//! Function-tool meaning; argument/result strings are untrusted data, never executable here.
use super::ItemId;
use crate::semantic::value::Text;

/// Omission has different schema semantics in the two protocol families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrictDefault {
    NonStrict,
    NormalizeSchema,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionStrictness {
    Omitted(StrictDefault),
    Explicit(bool),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionTool {
    pub name: Text,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub strict: FunctionStrictness,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolDefinition {
    Function(FunctionTool),
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum ToolChoice {
    None,
    #[default]
    Auto,
    Required,
    Specific(Text),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCall {
    pub call_id: Text,
    pub name: Text,
    /// Exact bounded wire string, including incomplete or invalid JSON. No repair or execution.
    pub arguments: String,
    /// Stable owner of a Chat assistant message, independent of call_id and item position.
    pub message: Option<ItemId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolResult {
    pub call_id: Text,
    pub output: String,
}
