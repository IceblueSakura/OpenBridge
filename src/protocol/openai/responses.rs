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
            "reasoning",
            "include",
        ],
    )?;
    let input = o
        .get("input")
        .and_then(Value::as_array)
        .ok_or(CodecError::Invalid("input"))?;
    let mut b = Items::default();
    decode_items(&mut b, input, false, "completed")?;
    let r = GenerationRequest::new(b.items, controls(o, "max_output_tokens")?)?
        .with_reasoning(super::reasoning::request(o)?);
    Ok(DecodedRequest {
        semantic: function_tools::decode(r, o, Profile::Responses)?,
        fidelity: b.fidelity,
    })
}
pub(super) fn decode_items(
    b: &mut Items,
    input: &[Value],
    response: bool,
    item_status: &str,
) -> Result<(), CodecError> {
    for item in input {
        let o = object(item)?;
        let id = b.id()?;
        let kind = o
            .get("type")
            .map(|v| v.as_str().ok_or(CodecError::Invalid("item type")))
            .transpose()?;
        let item = match kind {
            Some("function_call") => Item::ToolCall(tool_call(
                o,
                Profile::Responses,
                None,
                response.then_some(item_status),
            )?),
            Some("reasoning") => {
                let (reasoning, encrypted) = super::reasoning::decode_item(
                    o,
                    &mut || b.part_id(),
                    !response || item_status == "incomplete",
                )?;
                if let Some(encrypted) = encrypted {
                    b.fidelity.record_replay(
                        id,
                        ReasoningReplay {
                            value: EncryptedReasoning::Final(text(
                                &encrypted,
                                "encrypted reasoning",
                                MAX_TEXT_BYTES,
                            )?),
                            origin: None,
                        },
                        &reasoning,
                    )?;
                }
                Item::Reasoning(reasoning)
            }
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
                if response {
                    accept_status(o, item_status)?;
                }
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
                                let typ = string(p, "type")?;
                                if typ == "refusal" {
                                    if role != MessageRole::Assistant {
                                        return Err(CodecError::Invalid("refusal"));
                                    }
                                    fields(p, &["type", "refusal"])?;
                                    return b.refusal(
                                        crate::semantic::value::Text::allowing_empty(
                                            string(p, "refusal")?,
                                            "refusal",
                                            MAX_TEXT_BYTES,
                                        )
                                        .map_err(|_| CodecError::Limit)?,
                                    );
                                }
                                fields(p, &["type", "text", "annotations"])?;
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
                    let status = match o.get("status") {
                        None | Some(Value::Null) => ItemLifecycle::Completed,
                        Some(Value::String(s)) if s == "completed" => ItemLifecycle::Completed,
                        Some(Value::String(s)) if s == "incomplete" => ItemLifecycle::Incomplete,
                        _ => return Err(CodecError::Invalid("message status")),
                    };
                    Item::Message(Message {
                        role,
                        parts,
                        status,
                    })
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
    super::reasoning::write_request(target.semantic.reasoning(), o, Profile::Responses);
    bounded(&v)?;
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
            Item::Message(m)
                if m.parts.is_empty()
                    && items.iter().any(
                        |(_, item)| matches!(item, Item::ToolCall(c) if c.message == Some(*id)),
                    ) =>
            {
                continue;
            }
            Item::Message(m) => {
                let parts: Vec<_> = m.parts.iter().map(|p| match &p.content {
                    ContentPart::Text(t) => {
                        let mut p = json!({"type":if m.role == MessageRole::User {"input_text"} else {"output_text"},"text":t.as_str()});
                        if response { p["annotations"] = json!([]); } p
                    }
                    ContentPart::Refusal(t) => json!({"type":"refusal","refusal":t.as_str()}),
                    ContentPart::Resource(_) => unreachable!("lowering rejects media"),
                }).collect();
                json!({"type":"message","role":if m.role == MessageRole::User {"user"} else {"assistant"},"content":parts})
            }
            Item::ToolCall(c) => {
                let mut v = json!({"type":"function_call","call_id":c.call_id.as_str(),"name":c.name.as_str(),"arguments":c.arguments});
                if response {
                    v["status"] = json!(match c.status {
                        ItemLifecycle::Completed => "completed",
                        ItemLifecycle::Incomplete => "incomplete",
                    });
                }
                v
            }
            Item::Reasoning(reasoning) => {
                super::reasoning::encode_item(*id, reasoning, fidelity, response)
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
            if let Some(status) = item.lifecycle() {
                v["status"] = json!(match status {
                    ItemLifecycle::Completed => "completed",
                    ItemLifecycle::Incomplete => "incomplete",
                });
            }
        } else if let Some(source_id) = fidelity.response_item_id(*id) {
            // Only a surviving owner is visited. Source records never create output items.
            if v.get("type").is_none() {
                v["type"] = json!("message");
            }
            v["id"] = json!(source_id);
        }
        if !response && item.lifecycle() == Some(ItemLifecycle::Incomplete) {
            v["status"] = json!("incomplete");
        }
        output.push(v);
    }
    output
}
