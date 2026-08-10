//! Definition, input validation, and side-effect-free execution for the `hello` test tool.
//!
//! This tool may use only its bounded request arguments. It must not read private configuration,
//! inspect the registry, access files, perform network egress, or call Providers.

use serde_json::{Map, Value, json};

pub(super) const NAME: &str = "hello";
const INVALID_HELLO_ARGUMENTS: &str =
    "Invalid arguments: `name` must be a string and no other arguments are allowed.";

/// Returns the deterministic MCP definition for the hello tool.
pub(super) fn definition() -> Value {
    json!({
        "name": NAME,
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
    })
}

/// Executes the hello tool without performing external side effects.
pub(super) fn call(arguments: Option<&Map<String, Value>>) -> Value {
    // Validate the advertised closed input schema before formatting the greeting.
    let Some(arguments) = arguments else {
        return invalid_hello_arguments_result();
    };
    let Some(name) = arguments.get("name").and_then(Value::as_str) else {
        return invalid_hello_arguments_result();
    };
    if arguments.len() != 1 {
        return invalid_hello_arguments_result();
    }

    // Return the exact user-visible greeting as one MCP text content block.
    json!({
        "resultType": "complete",
        "content": [{
            "type": "text",
            "text": format!("Hi, {name}!")
        }],
        "isError": false
    })
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
