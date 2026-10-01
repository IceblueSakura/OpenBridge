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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallerMode {
    Direct,
    Programmatic,
}
/// Active tool-definition dispatch. Inactive defaults stay absent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ToolDispatch {
    pub async_call: bool,
    pub defer_loading: bool,
    pub allowed_callers: Option<Vec<CallerMode>>,
}
impl ToolDispatch {
    pub fn is_inactive(&self) -> bool {
        !self.async_call && !self.defer_loading && self.allowed_callers.is_none()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallOrigin {
    Program { caller_id: Text },
}
/// Active call context. Direct callers, empty namespaces and async false stay absent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallContext {
    pub namespace: Option<Text>,
    pub async_call: bool,
    pub caller: Option<CallOrigin>,
}
impl CallContext {
    pub fn is_direct(&self) -> bool {
        self.namespace.is_none() && !self.async_call && self.caller.is_none()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionTool {
    pub name: Text,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub strict: FunctionStrictness,
    pub output_schema: Option<serde_json::Value>,
    pub dispatch: ToolDispatch,
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
    pub dispatch: ToolDispatch,
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
    pub fn dispatch_inactive(&self) -> bool {
        match self {
            Self::Function(t) => t.dispatch.is_inactive(),
            Self::Custom(t) => t.dispatch.is_inactive(),
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
    /// Explicit assistant owner. Absence does not declare membership in a neighboring group.
    pub message: Option<ItemId>,
    pub status: ItemLifecycle,
    pub context: CallContext,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomCall {
    pub call_id: Text,
    pub name: Text,
    pub input: String,
    pub context: CallContext,
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
    pub context: CallContext,
}
/// Programmatic-calling program item. Opaque `code` and `fingerprint` round-trip
/// verbatim; no code may rebuild or validate them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub call_id: Text,
    pub code: String,
    pub fingerprint: String,
}
/// Terminal result of a program item, keyed by the program call ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramOutput {
    pub call_id: Text,
    pub result: String,
    pub status: ItemLifecycle,
}
