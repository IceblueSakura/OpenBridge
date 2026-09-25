//! Chat request mapping, including explicit assistant message ownership for tool calls.
pub use super::static_response::{decode_chat as decode_response, encode_chat as encode_response};
use super::{
    CodecError, DecodedRequest, Profile, RequestRepresentation, common::*, function_tools,
};
use crate::semantic::task::generation::*;
use crate::semantic::value::Presence;
use serde_json::{Map, Value, json};

pub(super) const FIELDS: &[&str] = &[
    "messages",
    "temperature",
    "top_p",
    "max_completion_tokens",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "reasoning_effort",
    "response_format",
];
pub fn decode_generation(v: &Value) -> Result<DecodedRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    fields(o, FIELDS)?;
    let messages = o
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("messages"))?;
    let mut b = Items::default();
    for message in messages {
        decode_message(&mut b, object(message)?)?;
    }
    let mut controls = controls(o, "max_completion_tokens")?;
    if let Some(v) = o.get("top_p").filter(|v| !v.is_null()) {
        controls = controls.with_top_p(v.as_f64().ok_or(CodecError::Invalid("top_p"))?)?;
    }
    let format = read_presence(o, "response_format", read_response_format)?;
    let settings = GenerationSettings {
        controls,
        text: TextOptions {
            presence: !format.is_absent(),
            format,
            verbosity: Presence::Absent,
        },
        reasoning: super::reasoning::chat_request(o)?,
        ..Default::default()
    };
    let r = GenerationRequest::from_settings(b.items, settings)?;
    Ok(DecodedRequest {
        semantic: function_tools::decode(r, o, Profile::Chat)?,
        fidelity: b.fidelity,
    })
}
/// Chat nests the json_schema body under `json_schema`; the flat Responses shell is parsed apart.
fn read_response_format(v: &Value) -> Result<OutputConstraint, CodecError> {
    let o = object(v)?;
    Ok(match string(o, "type")? {
        "text" => {
            fields(o, &["type"])?;
            OutputConstraint::Text
        }
        "json_object" => {
            fields(o, &["type"])?;
            OutputConstraint::JsonObject
        }
        "json_schema" => {
            fields(o, &["type", "json_schema"])?;
            let body = object(
                o.get("json_schema")
                    .ok_or(CodecError::Invalid("json_schema"))?,
            )?;
            fields(body, &["name", "description", "schema", "strict"])?;
            super::settings::read_schema_body(body)?
        }
        _ => return Err(CodecError::Unsupported("response format".into())),
    })
}
fn write_response_format(f: &OutputConstraint) -> Value {
    match f {
        OutputConstraint::Text => json!({"type":"text"}),
        OutputConstraint::JsonObject => json!({"type":"json_object"}),
        OutputConstraint::JsonSchema {
            name,
            description,
            schema,
            strict,
        } => {
            let mut body = Map::new();
            super::settings::write_schema_body(
                &mut body,
                name,
                description.as_ref(),
                schema,
                *strict,
            );
            json!({"type":"json_schema","json_schema":Value::Object(body)})
        }
    }
}
pub(super) fn decode_message(b: &mut Items, m: &Map<String, Value>) -> Result<(), CodecError> {
    fields(
        m,
        &["role", "content", "tool_calls", "tool_call_id", "refusal"],
    )?;
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
                    output: raw_string(m, "content")?.into(),
                    status: None,
                }),
            ));
        }
        "system" | "developer" => {
            if m.contains_key("tool_calls") || m.contains_key("tool_call_id") {
                return Err(CodecError::Invalid("instruction tool fields"));
            }
            let part_id = b.part_id()?;
            b.items.push((
                id,
                Item::Instruction(Instruction {
                    authority: if role == "system" {
                        InstructionAuthority::System
                    } else {
                        InstructionAuthority::Developer
                    },
                    parts: vec![(
                        part_id,
                        crate::semantic::value::Text::allowing_empty(
                            string(m, "content")?,
                            "instruction",
                            MAX_TEXT_BYTES,
                        )
                        .map_err(|_| CodecError::Limit)?,
                    )],
                }),
            ));
        }
        "user" | "assistant" => {
            if m.contains_key("tool_call_id") || (role == "user" && m.contains_key("tool_calls")) {
                return Err(CodecError::Invalid("message tool fields"));
            }
            let calls = m
                .get("tool_calls")
                .filter(|v| !v.is_null())
                .map(|v| v.as_array().ok_or(CodecError::Invalid("tool_calls")))
                .transpose()?;
            if calls.is_some_and(Vec::is_empty) {
                return Err(CodecError::Invalid("empty tool_calls"));
            }
            let refusal = match m.get("refusal") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) if role == "assistant" => Some(
                    crate::semantic::value::Text::allowing_empty(value, "refusal", MAX_TEXT_BYTES)
                        .map_err(|_| CodecError::Limit)?,
                ),
                _ => return Err(CodecError::Invalid("refusal")),
            };
            let parts = match (m.get("content"), refusal) {
                (_, Some(_)) if calls.is_some() => {
                    return Err(CodecError::Invalid("refusal"));
                }
                (Some(Value::String(s)), Some(_)) if !s.is_empty() => {
                    return Err(CodecError::Invalid("refusal"));
                }
                (_, Some(refusal)) => vec![b.refusal(refusal)?],
                (Some(Value::String(s)), None) => vec![b.part(s)?],
                (None | Some(Value::Null), None) if role == "assistant" => Vec::new(),
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
                    status: ItemLifecycle::Completed,
                }),
            ));
            if let Some(calls) = calls {
                for c in calls {
                    let call = tool_call(object(c)?, Profile::Chat, Some(id), None)?;
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
    let mut messages = encode_items(target.semantic.items());
    if let Some(t) = target.semantic.instructions().value() {
        messages.insert(0, json!({"role":"developer","content":t.as_str()}));
    }
    let mut v = json!({"messages": messages});
    let o = v.as_object_mut().expect("object literal");
    write_controls(target.semantic, o, "max_completion_tokens");
    if let Some(p) = target.semantic.controls().top_p() {
        o.insert("top_p".into(), json!(p));
    }
    put_presence(
        o,
        "response_format",
        &target.semantic.text_options().format,
        write_response_format,
    );
    function_tools::encode(target.semantic, Profile::Chat, o);
    super::reasoning::write_request(target.semantic.reasoning(), o, Profile::Chat);
    bounded(&v)?;
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
                messages.push(json!({"role":match i.authority { InstructionAuthority::System => "system", InstructionAuthority::Developer => "developer" },"content":i.parts[0].1.as_str()}));
            }
            Item::Message(m) => {
                standalone_calls = false;
                let mut message =
                    json!({"role":if m.role == MessageRole::User { "user" } else { "assistant" }});
                match m.parts.as_slice() {
                    [] => message["content"] = Value::Null,
                    [part] => match &part.content {
                        ContentPart::Text(t) => message["content"] = json!(t.as_str()),
                        ContentPart::Refusal(t) => {
                            message["content"] = Value::Null;
                            message["refusal"] = json!(t.as_str());
                        }
                        ContentPart::Resource(_) => unreachable!("lowering rejects media"),
                    },
                    _ => unreachable!("lowering rejects multi-part chat text"),
                }
                messages.push(message);
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
            Item::Reasoning(_) | Item::CustomCall(_) | Item::CustomResult(_) => {
                unreachable!("lowering rejects unsupported Chat items")
            }
            Item::ToolResult(r) => {
                standalone_calls = false;
                messages.push(
                    json!({"role":"tool","tool_call_id":r.call_id.as_str(),"content":match &r.output { ToolOutput::Text(s)=>json!(s), ToolOutput::Parts(_)=>unreachable!("lowering rejects Chat tool result parts") }}),
                );
            }
        }
    }
    messages
}
