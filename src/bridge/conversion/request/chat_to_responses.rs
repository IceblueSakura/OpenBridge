//! Chat Completions 请求到 Responses 请求的受限转换。
//!
//! 本模块转换 message content、function schema 与 tool choice，并维护 Chat tool call/result
//! 到 Responses input items 的局部 identity ledger。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{allocate_non_stream_item_id, copy_fields, required_string, validate_arguments},
};

/// 将一个已通过顶层校验的 Chat 请求转换为 Responses 请求。
pub(in crate::bridge::conversion) fn chat_request_to_responses(
    source: &Map<String, Value>,
    upstream_model: &str,
) -> Result<Value, BridgeError> {
    // 转换 Chat messages，并验证 tool call/result 的局部 identity ledger。
    let messages = source
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);
    let tools_present = source.get("tools").is_some();
    let input = chat_messages_to_responses(messages, stream, tools_present)?;

    // 复制两协议共同字段，并转换 function schema 与输出 token 字段。
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("input".to_owned(), input);
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &["parallel_tool_calls", "temperature", "top_p"],
    );
    if let Some(max_tokens) = source
        .get("max_completion_tokens")
        .or_else(|| source.get("max_tokens"))
    {
        result.insert("max_output_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(chat_tool_to_responses)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            chat_tool_choice_to_responses(tool_choice)?,
        );
    }
    Ok(Value::Object(result))
}

/// 将有序 Chat messages 转换为 Responses input，并校验 call/result identity。
fn chat_messages_to_responses(
    messages: &[Value],
    stream: bool,
    tools_present: bool,
) -> Result<Value, BridgeError> {
    // 对最常见的单条流式文本保留 Responses 简写，其他 history 使用显式 input items。
    if stream && messages.len() == 1 {
        let message = messages[0].as_object().ok_or(BridgeError::InvalidShape)?;
        if message.get("role").and_then(Value::as_str) == Some("user")
            && let Some(text) = message.get("content").and_then(Value::as_str)
        {
            return Ok(Value::String(text.to_owned()));
        }
    }

    // 按消息顺序转换 history，并维护 function call/result identity ledger。
    let mut input = Vec::new();
    let mut known_calls = BTreeMap::<String, (String, String)>::new();
    let mut item_ids = BTreeSet::new();
    for message in messages {
        let message = message.as_object().ok_or(BridgeError::InvalidShape)?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidShape)?;
        match role {
            "assistant" if message.get("tool_calls").is_some() => {
                if message
                    .get("content")
                    .is_some_and(|content| !content.is_null() && content.as_str() != Some(""))
                {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                let calls = message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .ok_or(BridgeError::InvalidShape)?;
                for call in calls {
                    let call = call.as_object().ok_or(BridgeError::InvalidShape)?;
                    let id = required_string(call, "id")?;
                    if call.get("type").and_then(Value::as_str) != Some("function") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    let function = call
                        .get("function")
                        .and_then(Value::as_object)
                        .ok_or(BridgeError::InvalidShape)?;
                    let name = required_string(function, "name")?;
                    let arguments = required_string(function, "arguments")?;
                    validate_arguments(&arguments)?;
                    if known_calls
                        .insert(id.clone(), (name.clone(), arguments.clone()))
                        .is_some()
                    {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    let item_id =
                        allocate_non_stream_item_id(&id, known_calls.len(), &mut item_ids);
                    input.push(json!({
                        "arguments": arguments,
                        "call_id": id,
                        "id": item_id,
                        "name": name,
                        "type": "function_call"
                    }));
                }
            }
            "tool" => {
                let call_id = required_string(message, "tool_call_id")?;
                if !known_calls.contains_key(&call_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = message
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or(BridgeError::InvalidShape)?;
                input.push(json!({
                    "call_id": call_id,
                    "output": output,
                    "type": "function_call_output"
                }));
            }
            "user" | "assistant" | "system" | "developer" => {
                let content = message.get("content").ok_or(BridgeError::InvalidShape)?;
                let converted = chat_content_to_responses(content, tools_present)?;
                input.push(json!({"content": converted, "role": role, "type": "message"}));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    Ok(Value::Array(input))
}

/// 将 Chat message content 转换为 Responses input content。
fn chat_content_to_responses(content: &Value, preserve_string: bool) -> Result<Value, BridgeError> {
    // 先保留允许直接使用的纯文本简写，避免改变最小请求的 wire 形状。
    match content {
        Value::String(text) if preserve_string => Ok(Value::String(text.clone())),
        Value::String(text) => Ok(json!([{"text": text, "type": "input_text"}])),
        Value::Array(parts) => {
            // 再逐项校验并转换为 Responses 的 input_text content part。
            let converted = parts
                .iter()
                .map(|part| {
                    let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                    if part.get("type").and_then(Value::as_str) != Some("text") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    Ok(json!({
                        "text": required_string(part, "text")?,
                        "type": "input_text"
                    }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(converted))
        }
        _ => Err(BridgeError::UnsupportedSemantics),
    }
}

/// 将 Chat function tool schema 展平为 Responses function tool。
fn chat_tool_to_responses(tool: &Value) -> Result<Value, BridgeError> {
    // 校验 Chat tool 的 function 包装层，并只复制已建模的 function 字段。
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let function = tool
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::InvalidShape)?;
    // 添加 Responses 所需的 flat function type 标记。
    let mut result = function.clone();
    result.insert("type".to_owned(), Value::String("function".to_owned()));
    Ok(Value::Object(result))
}

/// 将 Chat tool choice 转换为 Responses tool choice。
fn chat_tool_choice_to_responses(choice: &Value) -> Result<Value, BridgeError> {
    // 直接保留三种两协议共用的字符串选择值。
    if choice
        .as_str()
        .is_some_and(|choice| matches!(choice, "auto" | "none" | "required"))
    {
        return Ok(choice.clone());
    }
    let choice = choice
        .as_object()
        .ok_or(BridgeError::UnsupportedSemantics)?;
    // 校验命名 function 选择并压平 Chat 的 function 包装层。
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let function = choice
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::UnsupportedSemantics)?;
    Ok(json!({"name": required_string(function, "name")?, "type": "function"}))
}
