//! Converts the modeled structured-output request shapes between Chat and Responses.
//!
//! Only JSON text, JSON object, and JSON Schema formats are translated. Unknown fields are
//! rejected so a Bridge never claims to preserve a Provider-specific output constraint.

use serde_json::{Map, Value, json};

use super::super::BridgeError;

/// Converts Chat `response_format` into the Responses `text.format` wrapper.
pub(in crate::bridge::conversion) fn chat_response_format_to_responses(
    source: &Value,
) -> Result<Value, BridgeError> {
    // Validate the Chat wrapper before selecting the corresponding Responses format.
    let source = source.as_object().ok_or(BridgeError::InvalidShape)?;
    reject_unknown_keys(source, &["type", "json_schema"])?;
    let kind = source
        .get("type")
        .and_then(Value::as_str)
        .ok_or(BridgeError::InvalidShape)?;
    let format = match kind {
        "text" => json!({"type": "text"}),
        "json_object" => json!({"type": "json_object"}),
        "json_schema" => {
            let schema = source
                .get("json_schema")
                .and_then(Value::as_object)
                .ok_or(BridgeError::InvalidShape)?;
            chat_schema_to_responses(schema)?
        }
        _ => return Err(BridgeError::UnsupportedSemantics),
    };
    Ok(json!({"format": format}))
}

/// Converts Responses `text` into Chat `response_format`.
pub(in crate::bridge::conversion) fn responses_text_to_chat(
    source: &Value,
) -> Result<Value, BridgeError> {
    // Validate the Responses text wrapper and use its documented default when format is omitted.
    let source = source.as_object().ok_or(BridgeError::InvalidShape)?;
    reject_unknown_keys(source, &["format"])?;
    let Some(format) = source.get("format") else {
        return Ok(json!({"type": "text"}));
    };
    let format = format.as_object().ok_or(BridgeError::InvalidShape)?;
    reject_unknown_keys(format, &["type", "name", "description", "schema", "strict"])?;
    let kind = format
        .get("type")
        .and_then(Value::as_str)
        .ok_or(BridgeError::InvalidShape)?;
    match kind {
        "text" => Ok(json!({"type": "text"})),
        "json_object" => Ok(json!({"type": "json_object"})),
        "json_schema" => Ok(json!({
            "type": "json_schema",
            "json_schema": responses_schema_to_chat(format)?,
        })),
        _ => Err(BridgeError::UnsupportedSemantics),
    }
}

/// Flattens the Chat JSON Schema wrapper into the Responses format object.
fn chat_schema_to_responses(source: &Map<String, Value>) -> Result<Value, BridgeError> {
    // Reject unmodeled schema options before copying the required and optional fields.
    reject_unknown_keys(source, &["name", "description", "schema", "strict"])?;
    let name = source
        .get("name")
        .and_then(Value::as_str)
        .ok_or(BridgeError::InvalidShape)?;
    let schema = source
        .get("schema")
        .filter(|value| value.is_object())
        .ok_or(BridgeError::InvalidShape)?;
    let mut result = Map::new();
    result.insert("type".to_owned(), Value::String("json_schema".to_owned()));
    result.insert("name".to_owned(), Value::String(name.to_owned()));
    copy_optional_string(source, &mut result, "description")?;
    result.insert("schema".to_owned(), schema.clone());
    copy_optional_bool(source, &mut result, "strict")?;
    Ok(Value::Object(result))
}

/// Nests the flat Responses JSON Schema format into the Chat wrapper.
fn responses_schema_to_chat(source: &Map<String, Value>) -> Result<Value, BridgeError> {
    // Reject unmodeled format options before reconstructing Chat's nested schema object.
    reject_unknown_keys(source, &["type", "name", "description", "schema", "strict"])?;
    let name = source
        .get("name")
        .and_then(Value::as_str)
        .ok_or(BridgeError::InvalidShape)?;
    let schema = source
        .get("schema")
        .filter(|value| value.is_object())
        .ok_or(BridgeError::InvalidShape)?;
    let mut result = Map::new();
    result.insert("name".to_owned(), Value::String(name.to_owned()));
    copy_optional_string(source, &mut result, "description")?;
    result.insert("schema".to_owned(), schema.clone());
    copy_optional_bool(source, &mut result, "strict")?;
    Ok(Value::Object(result))
}

/// Rejects fields outside one explicitly modeled structured-output object.
fn reject_unknown_keys(source: &Map<String, Value>, allowed: &[&str]) -> Result<(), BridgeError> {
    // Fail closed before any partial output object is assembled.
    if source.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(())
}

/// Copies one optional string field while rejecting an invalid wire type.
fn copy_optional_string(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    field: &str,
) -> Result<(), BridgeError> {
    // Preserve a declared field only after validating its scalar type.
    if let Some(value) = source.get(field) {
        if !value.is_string() {
            return Err(BridgeError::InvalidShape);
        }
        target.insert(field.to_owned(), value.clone());
    }
    Ok(())
}

/// Copies one optional Boolean field while rejecting an invalid wire type.
fn copy_optional_bool(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    field: &str,
) -> Result<(), BridgeError> {
    // Preserve a declared field only after validating its scalar type.
    if let Some(value) = source.get(field) {
        if !value.is_boolean() {
            return Err(BridgeError::InvalidShape);
        }
        target.insert(field.to_owned(), value.clone());
    }
    Ok(())
}
