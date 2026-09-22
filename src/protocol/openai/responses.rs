//! Responses mapping keeps call identity separate from optional source wire item identity.
pub use super::static_response::{
    decode_responses as decode_response, encode_responses as encode_response,
};
use super::{
    CodecError, DecodedRequest, Profile, RequestRepresentation, common::*, function_tools,
};
use crate::{protocol::fidelity::FidelityRecords, semantic::task::generation::*};
use serde_json::{Value, json};

pub fn decode_generation(v: &Value) -> Result<DecodedRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    fields(
        o,
        &[
            "input",
            "temperature",
            "max_output_tokens",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
        ],
    )?;
    let input = o
        .get("input")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("input"))?;
    let mut b = Items::default();
    decode_items(&mut b, input, false)?;
    let r = GenerationRequest::new(b.items, controls(o, "max_output_tokens")?)?;
    Ok(DecodedRequest {
        semantic: function_tools::decode(r, o, Profile::Responses)?,
        fidelity: b.fidelity,
    })
}
pub(super) fn decode_items(
    b: &mut Items,
    input: &[Value],
    response: bool,
) -> Result<(), CodecError> {
    for item in input {
        let o = object(item)?;
        let id = b.id()?;
        let kind = o
            .get("type")
            .map(|v| v.as_str().ok_or(CodecError::Invalid("item type")))
            .transpose()?;
        let item = match kind {
            Some("function_call") => Item::ToolCall(tool_call(o, Profile::Responses, None)?),
            Some("function_call_output") if !response => {
                fields(o, &["type", "call_id", "output", "id"])?;
                Item::ToolResult(ToolResult {
                    call_id: text(string(o, "call_id")?, "call_id", 256)?,
                    output: raw_string(o, "output")?,
                })
            }
            Some("message") | None => {
                // Shorthand with extra item metadata is ambiguous and is deliberately rejected.
                fields(
                    o,
                    if kind.is_none() {
                        &["role", "content"]
                    } else {
                        &["type", "id", "role", "content", "status"]
                    },
                )?;
                completed_item(o)?;
                let role = string(o, "role")?;
                if role == "system" || role == "developer" {
                    Item::Instruction(Instruction {
                        authority: if role == "system" {
                            InstructionAuthority::System
                        } else {
                            InstructionAuthority::Developer
                        },
                        text: text(string(o, "content")?, "instruction", MAX_TEXT_BYTES)?,
                    })
                } else {
                    let role = match role {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        _ => return Err(CodecError::Unsupported("message role".into())),
                    };
                    let content = o.get("content").ok_or(CodecError::Invalid("content"))?;
                    let parts = if let Some(s) = content.as_str() {
                        if response {
                            return Err(CodecError::Invalid("output content"));
                        }
                        vec![b.part(s)?]
                    } else {
                        content
                            .as_array()
                            .ok_or(CodecError::Invalid("content parts"))?
                            .iter()
                            .map(|p| {
                                let p = object(p)?;
                                fields(p, &["type", "text", "annotations"])?;
                                let typ = string(p, "type")?;
                                if typ
                                    != if role == MessageRole::User {
                                        "input_text"
                                    } else {
                                        "output_text"
                                    }
                                {
                                    return Err(CodecError::Unsupported(
                                        "content part type".into(),
                                    ));
                                }
                                if let Some(a) = p.get("annotations")
                                    && !a.as_array().is_some_and(Vec::is_empty)
                                {
                                    return Err(CodecError::Unsupported("annotations".into()));
                                }
                                b.part(string(p, "text")?)
                            })
                            .collect::<Result<Vec<_>, CodecError>>()?
                    };
                    Item::Message(Message { role, parts })
                }
            }
            _ => return Err(CodecError::Unsupported("Responses item kind".into())),
        };
        b.record_id(id, o)?;
        b.items.push((id, item));
    }
    Ok(())
}
pub fn encode_generation(target: &RequestRepresentation<'_>) -> Result<Value, CodecError> {
    if target.profile != Profile::Responses {
        return Err(CodecError::ProfileMismatch);
    }
    let mut v = json!({"input":encode_items(target.semantic.items(), target.fidelity, false)});
    let o = v.as_object_mut().expect("object literal");
    write_controls(target.semantic, o, "max_output_tokens");
    function_tools::encode(target.semantic, Profile::Responses, o);
    Ok(v)
}
pub(super) fn encode_items(
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
    response: bool,
) -> Vec<Value> {
    let mut output = Vec::new();
    for (id, item) in items {
        let mut v = match item {
            Item::Instruction(i) => {
                json!({"role":match i.authority { InstructionAuthority::System => "system", InstructionAuthority::Developer => "developer" },"content":i.text.as_str()})
            }
            Item::Message(m) if m.parts.is_empty() => continue, // An empty Chat owner carries grouping only; its calls remain semantic items.
            Item::Message(m) => {
                let parts: Vec<_> = m.parts.iter().map(|p| match &p.content {
                    ContentPart::Text(t) => {
                        let mut p = json!({"type":if m.role == MessageRole::User {"input_text"} else {"output_text"},"text":t.as_str()});
                        if response { p["annotations"] = json!([]); } p
                    }
                    ContentPart::Resource(_) => unreachable!("lowering rejects media"),
                }).collect();
                json!({"type":"message","role":if m.role == MessageRole::User {"user"} else {"assistant"},"content":parts})
            }
            Item::ToolCall(c) => {
                json!({"type":"function_call","call_id":c.call_id.as_str(),"name":c.name.as_str(),"arguments":c.arguments})
            }
            Item::ToolResult(r) => {
                json!({"type":"function_call_output","call_id":r.call_id.as_str(),"output":r.output})
            }
        };
        if response {
            v["id"] = json!(
                fidelity
                    .response_item_id(*id)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("item_{}", id.get()))
            );
            v["status"] = json!("completed");
        } else if let Some(source_id) = fidelity.response_item_id(*id) {
            // Only a surviving owner is visited. Source records never create output items.
            if v.get("type").is_none() {
                v["type"] = json!("message");
            }
            v["id"] = json!(source_id);
        }
        output.push(v);
    }
    output
}
