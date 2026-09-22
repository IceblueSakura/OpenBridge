use crate::semantic::value::Text;
#[derive(Clone,Debug,Eq,PartialEq)] pub struct FunctionTool{pub name:Text,pub description:Option<Text>,pub parameters:serde_json::Value,pub strict:bool}
#[derive(Clone,Debug,Eq,PartialEq)] pub enum ToolDefinition{Function(FunctionTool)}
#[derive(Clone,Debug,Eq,PartialEq,Default)] pub enum ToolChoice{None,#[default] Auto,Required,Specific(Text)}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct ToolCall{pub call_id:Text,pub name:Text,pub arguments:serde_json::Value}
#[derive(Clone,Debug,Eq,PartialEq)] pub struct ToolResult{pub call_id:Text,pub output:Text}
