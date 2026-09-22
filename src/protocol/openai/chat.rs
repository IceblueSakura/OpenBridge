//! Chat request mapping, including explicit assistant message ownership for tool calls.
pub use super::static_response::{decode_chat as decode_response, encode_chat as encode_response};
use super::{
    CodecError, DecodedRequest, Profile, RequestRepresentation, common::*, function_tools,
};
use crate::semantic::task::generation::*;
use serde_json::{Map, Value, json};

pub fn decode_generation(v: &Value) -> Result<DecodedRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    fields(
        o,
        &[
            "messages",
            "temperature",
            "max_completion_tokens",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
        ],
    )?;
    let messages = o
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("messages"))?;
    let mut b = Items::default();
    for message in messages {
        decode_message(&mut b, object(message)?)?;
    }
    let r = GenerationRequest::new(b.items, controls(o, "max_completion_tokens")?)?;
    Ok(DecodedRequest {
        semantic: function_tools::decode(r, o, Profile::Chat)?,
        fidelity: b.fidelity,
    })
}
pub(super) fn decode_message(b: &mut Items, m: &Map<String, Value>) -> Result<(), CodecError> {
    fields(m, &["role", "content", "tool_calls", "tool_call_id"])?;
    let role = string(m, "role")?;
    let id = b.id()?;
    match role {
        "tool" => {
            if m.contains_key("tool_calls") {
                return Err(CodecError::Invalid("tool result"));
            }
            b.items.push((
                id,
                Item::ToolResult(ToolResult {
                    call_id: text(string(m, "tool_call_id")?, "call_id", 256)?,
                    output: raw_string(m, "content")?,
                }),
            ));
        }
        "system" | "developer" => {
            if m.contains_key("tool_calls") || m.contains_key("tool_call_id") {
                return Err(CodecError::Invalid("instruction tool fields"));
            }
            b.items.push((
                id,
                Item::Instruction(Instruction {
                    authority: if role == "system" {
                        InstructionAuthority::System
                    } else {
                        InstructionAuthority::Developer
                    },
                    text: text(string(m, "content")?, "instruction", MAX_TEXT_BYTES)?,
                }),
            ));
        }
        "user" | "assistant" => {
            if m.contains_key("tool_call_id") || (role == "user" && m.contains_key("tool_calls")) {
                return Err(CodecError::Invalid("message tool fields"));
            }
            let calls = m
                .get("tool_calls")
                .map(|v| v.as_array().ok_or(CodecError::Invalid("tool_calls")))
                .transpose()?;
            if calls.is_some_and(Vec::is_empty) {
                return Err(CodecError::Invalid("empty tool_calls"));
            }
            let parts = match m.get("content") {
                Some(Value::String(s)) => vec![b.part(s)?],
                None | Some(Value::Null) if calls.is_some() => Vec::new(),
                _ => return Err(CodecError::Unsupported("message content".into())),
            };
            b.items.push((
                id,
                Item::Message(Message {
                    role: if role == "user" {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    },
                    parts,
                }),
            ));
            if let Some(calls) = calls {
                for c in calls {
                    let call = tool_call(object(c)?, Profile::Chat, Some(id))?;
                    let cid = b.id()?;
                    b.items.push((cid, Item::ToolCall(call)));
                }
            }
        }
        _ => return Err(CodecError::Unsupported("message role".into())),
    }
    Ok(())
}
pub fn encode_generation(target: &RequestRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Chat {
        return Err(CodecError::ProfileMismatch);
    }
    let mut v = json!({"messages": encode_items(target.semantic.items())});
    let o = v.as_object_mut().expect("object literal");
    write_controls(target.semantic, o, "max_completion_tokens");
    function_tools::encode(target.semantic, Profile::Chat, o);
    Ok(v)
}
pub(super) fn call_wire(c: &ToolCall) -> Value {
    json!({"id":c.call_id.as_str(),"type":"function","function":{"name":c.name.as_str(),"arguments":c.arguments}})
}
pub(super) fn encode_items(items: &[(ItemId, Item)]) -> Vec<Value> {
    let mut messages = Vec::<Value>::new();
    let mut standalone_calls = false;
    for (_, item) in items {
        match item {
            Item::Instruction(i) => {
                standalone_calls = false;
                messages.push(json!({"role":match i.authority { InstructionAuthority::System => "system", InstructionAuthority::Developer => "developer" },"content":i.text.as_str()}));
            }
            Item::Message(m) => {
                standalone_calls = false;
                let content = if m.parts.is_empty() {
                    Value::Null
                } else {
                    // Lowering admits only one text part, so boundaries cannot be silently collapsed.
                    match &m.parts[0].content {
                        ContentPart::Text(t) => json!(t.as_str()),
                        ContentPart::Resource(_) => unreachable!("lowering rejects media"),
                    }
                };
                messages.push(json!({"role":if m.role == MessageRole::User { "user" } else { "assistant" },"content":content}));
            }
            Item::ToolCall(c) => {
                if c.message.is_none() && !standalone_calls {
                    messages.push(json!({"role":"assistant","content":null}));
                }
                let message = messages
                    .last_mut()
                    .expect("validated call owner or standalone message");
                let calls = message
                    .as_object_mut()
                    .expect("message")
                    .entry("tool_calls")
                    .or_insert_with(|| json!([]));
                calls.as_array_mut().expect("calls").push(call_wire(c));
                standalone_calls = c.message.is_none();
            }
            Item::ToolResult(r) => {
                standalone_calls = false;
                messages.push(
                    json!({"role":"tool","tool_call_id":r.call_id.as_str(),"content":r.output}),
                );
            }
        }
    }
    messages
}
