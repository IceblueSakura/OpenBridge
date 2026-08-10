//! Static MCP test tool definitions, input validation, and side-effect-free execution.
//!
//! Tools in this module may use only their bounded request arguments. They must not read private
//! configuration, inspect the registry, access files, perform network egress, or call Providers.

use serde_json::{Map, Value, json};

const HELLO_TOOL_NAME: &str = "hello";
const INVALID_HELLO_ARGUMENTS: &str =
    "Invalid arguments: `name` must be a string and no other arguments are allowed.";

/// Identifies a protocol-level failure to select a registered tool.
pub(super) enum ToolDispatchError {
    /// The requested tool does not exist in the static catalog.
    UnknownTool,
}

/// Returns the deterministic list of locally registered tools.
pub(super) fn catalog() -> Value {
    json!([{
        "name": HELLO_TOOL_NAME,
        "description": "Returns a greeting for the provided name.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name to greet."
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }
    }])
}

/// Executes one registered tool without performing external side effects.
pub(super) fn call(
    tool_name: &str,
    arguments: Option<&Map<String, Value>>,
) -> Result<Value, ToolDispatchError> {
    // Select the static implementation without reflecting unknown tool names into diagnostics.
    if tool_name != HELLO_TOOL_NAME {
        return Err(ToolDispatchError::UnknownTool);
    }

    // Validate the advertised closed input schema before formatting the greeting.
    let Some(arguments) = arguments else {
        return Ok(invalid_hello_arguments_result());
    };
    let Some(name) = arguments.get("name").and_then(Value::as_str) else {
        return Ok(invalid_hello_arguments_result());
    };
    if arguments.len() != 1 {
        return Ok(invalid_hello_arguments_result());
    }

    // Return the exact user-visible greeting as one MCP text content block.
    Ok(json!({
        "resultType": "complete",
        "content": [{
            "type": "text",
            "text": format!("Hi, {name}!")
        }],
        "isError": false
    }))
}

/// Builds the actionable MCP tool result for invalid hello arguments.
fn invalid_hello_arguments_result() -> Value {
    json!({
        "resultType": "complete",
        "content": [{
            "type": "text",
            "text": INVALID_HELLO_ARGUMENTS
        }],
        "isError": true
    })
}
