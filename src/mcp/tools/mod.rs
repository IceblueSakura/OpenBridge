//! Static MCP tool catalog and dispatch boundary.
//!
//! This module owns deterministic tool ordering and name-based selection. Individual tool schemas,
//! validation, and execution belong in leaf modules and must preserve the no-egress boundary unless
//! an explicitly approved tool contract states otherwise.

mod hello;

use serde_json::{Map, Value};

/// Identifies a protocol-level failure to select a registered tool.
pub(super) enum ToolDispatchError {
    /// The requested tool does not exist in the static catalog.
    UnknownTool,
}

/// Returns all registered tool definitions in deterministic order.
pub(super) fn catalog() -> Value {
    Value::Array(vec![hello::definition()])
}

/// Dispatches one tool call to its owning leaf module.
pub(super) fn call(
    tool_name: &str,
    arguments: Option<&Map<String, Value>>,
) -> Result<Value, ToolDispatchError> {
    // Select only an explicitly registered tool name.
    if tool_name != hello::NAME {
        return Err(ToolDispatchError::UnknownTool);
    }

    // Delegate schema validation and execution to the tool owner.
    Ok(hello::call(arguments))
}
