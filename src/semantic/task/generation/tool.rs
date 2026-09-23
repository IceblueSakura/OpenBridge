//! Client-executed tools. Payloads and grammar are data; this module never executes them.
use super::{ItemId, PartId};
use crate::semantic::value::Text;
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
    pub output_schema: Option<serde_json::Value>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CustomFormat {
    Text,
    Grammar {
        syntax: GrammarSyntax,
        definition: Text,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrammarSyntax {
    Lark,
    Regex,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomTool {
    pub name: Text,
    pub description: Option<String>,
    pub format: Option<CustomFormat>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolDefinition {
    Function(FunctionTool),
    Custom(CustomTool),
}
impl ToolDefinition {
    pub fn name(&self) -> &Text {
        match self {
            Self::Function(t) => &t.name,
            Self::Custom(t) => &t.name,
        }
    }
    pub fn kind(&self) -> ToolKind {
        match self {
            Self::Function(_) => ToolKind::Function,
            Self::Custom(_) => ToolKind::Custom,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ToolKind {
    Function,
    Custom,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolReference {
    pub kind: ToolKind,
    pub name: Text,
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum ToolChoice {
    None,
    #[default]
    Auto,
    Required,
    Specific(Text),
    Custom(Text),
    Allowed {
        required: bool,
        tools: Vec<ToolReference>,
    },
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ItemLifecycle {
    #[default]
    Completed,
    Incomplete,
    InProgress,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCall {
    pub call_id: Text,
    pub name: Text,
    pub arguments: String,
    pub message: Option<ItemId>,
    pub status: ItemLifecycle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomCall {
    pub call_id: Text,
    pub name: Text,
    pub input: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolOutput {
    Text(String),
    Parts(Vec<(PartId, Text)>),
}
impl From<String> for ToolOutput {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self {
        Self::Text(s.into())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolResult {
    pub call_id: Text,
    pub output: ToolOutput,
    pub status: Option<ItemLifecycle>,
}
