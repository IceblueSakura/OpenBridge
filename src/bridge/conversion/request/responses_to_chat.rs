//! Responses 请求到 Chat Completions 请求的受限转换。
//!
//! 本模块展开 input items、包装 function schema 与 tool choice，并按 wire 顺序维护 function
//! call/output ledger，拒绝重复、未知或漂移的 identity。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::super::{
    BridgeError,
    shared::{copy_fields, required_string, validate_arguments},
};

/// 将一个已通过顶层校验的 Responses 请求转换为 Chat 请求。
pub(in crate::bridge::conversion) fn responses_request_to_chat(
    source: &Map<String, Value>,
    upstream_model: &str,
    reasoning_supported: bool,
) -> Result<Value, BridgeError> {
    // 将 Responses input 展开为 Chat messages，并校验 call/output ledger。
    let input = source.get("input").ok_or(BridgeError::InvalidShape)?;
    let messages = responses_input_to_chat(input, reasoning_supported)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);

    // 复制共同字段，并把 flat function schema 包装为 Chat function 对象。
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("messages".to_owned(), Value::Array(messages));
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &["parallel_tool_calls", "temperature", "top_p"],
    );
    if let Some(effort) = responses_reasoning_effort(source)? {
        if !reasoning_supported && effort != "none" {
            return Err(BridgeError::UnsupportedSemantics);
        }
        result.insert("reasoning_effort".to_owned(), Value::String(effort));
    }
    if let Some(max_tokens) = source.get("max_output_tokens") {
        result.insert("max_completion_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(responses_tool_to_chat)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            responses_tool_choice_to_chat(tool_choice)?,
        );
    }
    Ok(Value::Object(result))
}

/// 将 Responses input 展开为有序 Chat messages，并校验 call/result identity。
fn responses_input_to_chat(
    input: &Value,
    reasoning_supported: bool,
) -> Result<Vec<Value>, BridgeError> {
    if let Some(text) = input.as_str() {
        return Ok(vec![json!({"content": text, "role": "user"})]);
    }
    let items = input.as_array().ok_or(BridgeError::InvalidShape)?;
    let mut messages = Vec::new();
    let mut calls = BTreeMap::<String, Value>::new();
    let mut call_order = Vec::new();
    let mut emitted_calls = false;
    let mut seen_results = BTreeSet::new();
    let mut item_ids = BTreeSet::new();
    let mut pending_reasoning = String::new();

    // 先按 wire 顺序建立 call ledger，message/output 转换时保持原顺序。
    for item in items {
        let item = item.as_object().ok_or(BridgeError::InvalidShape)?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                let role = required_string(item, "role")?;
                if !pending_reasoning.is_empty() && role != "assistant" {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                if !calls.is_empty() && !emitted_calls {
                    messages.push(chat_assistant_tool_message(
                        &call_order,
                        &calls,
                        (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
                    ));
                    pending_reasoning.clear();
                    emitted_calls = true;
                }
                messages.push(responses_message_to_chat(
                    item,
                    (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
                )?);
                pending_reasoning.clear();
            }
            Some("reasoning") => {
                if !reasoning_supported {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                if emitted_calls {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                pending_reasoning.push_str(&responses_reasoning_item_text(item)?);
            }
            Some("function_call") => {
                if emitted_calls {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let call_id = required_string(item, "call_id")?;
                let item_id = required_string(item, "id")?;
                if !item_ids.insert(item_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let name = required_string(item, "name")?;
                let arguments = required_string(item, "arguments")?;
                validate_arguments(&arguments)?;
                if calls
                    .insert(
                        call_id.clone(),
                        json!({
                            "function": {"arguments": arguments, "name": name},
                            "id": call_id,
                            "type": "function"
                        }),
                    )
                    .is_some()
                {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                call_order.push(call_id);
            }
            Some("function_call_output") => {
                if !emitted_calls {
                    if calls.is_empty() {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    messages.push(chat_assistant_tool_message(
                        &call_order,
                        &calls,
                        (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
                    ));
                    pending_reasoning.clear();
                    emitted_calls = true;
                }
                let call_id = required_string(item, "call_id")?;
                if !calls.contains_key(&call_id) || !seen_results.insert(call_id.clone()) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = item.get("output").ok_or(BridgeError::InvalidShape)?;
                let output = output
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| output.to_string());
                messages.push(json!({
                    "content": output,
                    "role": "tool",
                    "tool_call_id": call_id
                }));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    if !calls.is_empty() && !emitted_calls {
        messages.push(chat_assistant_tool_message(
            &call_order,
            &calls,
            (!pending_reasoning.is_empty()).then_some(pending_reasoning.as_str()),
        ));
        pending_reasoning.clear();
    }
    if !pending_reasoning.is_empty() {
        messages.push(json!({
            "content": Value::Null,
            "reasoning_content": pending_reasoning,
            "role": "assistant"
        }));
    }
    Ok(messages)
}

/// 按原始 call 顺序构造 Chat assistant tool-call message。
fn chat_assistant_tool_message(
    order: &[String],
    calls: &BTreeMap<String, Value>,
    reasoning: Option<&str>,
) -> Value {
    let mut message = json!({
        "content": Value::Null,
        "role": "assistant",
        "tool_calls": order.iter().filter_map(|id| calls.get(id)).cloned().collect::<Vec<_>>()
    });
    if let Some(reasoning) = reasoning {
        message["reasoning_content"] = Value::String(reasoning.to_owned());
    }
    message
}

/// 将一个 Responses message item 转换为 Chat message。
fn responses_message_to_chat(
    item: &Map<String, Value>,
    reasoning: Option<&str>,
) -> Result<Value, BridgeError> {
    // 先读取并保留 Responses message 的角色字段。
    let role = required_string(item, "role")?;
    let content = item.get("content").ok_or(BridgeError::InvalidShape)?;
    // 再把字符串或 input_text parts 合并为 Chat 的单一 content 字符串。
    let content = match content {
        Value::String(text) => Value::String(text.clone()),
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                if part.get("type").and_then(Value::as_str) != Some("input_text") {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                text.push_str(&required_string(part, "text")?);
            }
            Value::String(text)
        }
        _ => return Err(BridgeError::UnsupportedSemantics),
    };
    let mut message = json!({"content": content, "role": role});
    if let Some(reasoning) = reasoning {
        message["reasoning_content"] = Value::String(reasoning.to_owned());
    }
    Ok(message)
}

/// 解析 Responses reasoning 配置，并转换为 Chat 的 reasoning_effort。
fn responses_reasoning_effort(source: &Map<String, Value>) -> Result<Option<String>, BridgeError> {
    // 只读取 Responses 标准 reasoning 对象，并拒绝未建模的子字段。
    let object_effort = source
        .get("reasoning")
        .filter(|value| !value.is_null())
        .map(|value| {
            let object = value.as_object().ok_or(BridgeError::InvalidShape)?;
            if object.keys().any(|key| key != "effort") {
                return Err(BridgeError::UnsupportedSemantics);
            }
            object
                .get("effort")
                .filter(|value| !value.is_null())
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .ok_or(BridgeError::InvalidShape)
                })
                .transpose()
        })
        .transpose()?
        .flatten();
    Ok(object_effort)
}

/// 提取 plain Responses reasoning item，拒绝无法由 Chat 表示的 opaque continuation。
fn responses_reasoning_item_text(item: &Map<String, Value>) -> Result<String, BridgeError> {
    reject_encrypted_reasoning(item)?;
    let mut text = String::new();
    for field in ["content", "summary"] {
        let Some(parts) = item.get(field).and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
            let expected = if field == "content" {
                "reasoning_text"
            } else {
                "summary_text"
            };
            if part.get("type").and_then(Value::as_str) != Some(expected) {
                return Err(BridgeError::UnsupportedSemantics);
            }
            text.push_str(&required_string(part, "text")?);
        }
    }
    Ok(text)
}

/// 拒绝无法由 Chat 明文表示的 Responses opaque reasoning continuation。
fn reject_encrypted_reasoning(item: &Map<String, Value>) -> Result<(), BridgeError> {
    let Some(value) = item
        .get("encrypted_content")
        .filter(|value| !value.is_null())
    else {
        return Ok(());
    };
    let content = value.as_str().ok_or(BridgeError::InvalidShape)?;
    if content.is_empty() {
        Ok(())
    } else {
        Err(BridgeError::UnsupportedSemantics)
    }
}

/// 将 Responses function tool schema 包装为 Chat function tool。
fn responses_tool_to_chat(tool: &Value) -> Result<Value, BridgeError> {
    // 校验 Responses flat function tool，并移除目标协议不使用的 type 字段。
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let mut function = tool.clone();
    function.remove("type");
    Ok(json!({"function": function, "type": "function"}))
}

/// 将 Responses tool choice 转换为 Chat tool choice。
fn responses_tool_choice_to_chat(choice: &Value) -> Result<Value, BridgeError> {
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
    // 校验命名 function 选择并重新包装为 Chat function 对象。
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(json!({
        "function": {"name": required_string(choice, "name")?},
        "type": "function"
    }))
}
