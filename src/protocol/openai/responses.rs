//! Responses task payload codec. HTTP request bindings are owned by `envelope`.
pub use super::static_response::{
    decode_responses as decode_response, encode_responses as encode_response,
};
use super::{CodecError, DecodedRequest, Profile, RequestRepresentation, common::*, settings};
use crate::{
    protocol::fidelity::FidelityRecords,
    semantic::{task::generation::*, value::Text},
};
use serde_json::{Map, Value, json};
pub fn decode_generation(v: &Value) -> Result<DecodedRequest, CodecError> {
    bounded(v)?;
    let o = object(v)?;
    let allowed: Vec<_> = settings::FIELDS.iter().copied().chain(["input"]).collect();
    fields(o, &allowed)?;
    let mut b = Items::default();
    match o.get("input") {
        Some(Value::String(s)) => {
            let id = b.id()?;
            let part = b.part(s)?;
            b.items.push((
                id,
                Item::Message(Message {
                    role: MessageRole::User,
                    parts: vec![part],
                    status: ItemLifecycle::Completed,
                }),
            ));
        }
        Some(Value::Array(a)) => decode_items(&mut b, a, false, "completed")?,
        None if o.get("instructions").is_some_and(Value::is_string) => {}
        _ => return Err(CodecError::Invalid("input")),
    }
    Ok(DecodedRequest {
        semantic: GenerationRequest::from_settings(b.items, settings::read(o)?)?,
        fidelity: b.fidelity,
    })
}
pub(super) fn status(
    o: &Map<String, Value>,
    default: ItemLifecycle,
) -> Result<ItemLifecycle, CodecError> {
    match o.get("status") {
        None => Ok(default),
        Some(Value::String(s)) => match s.as_str() {
            "completed" => Ok(ItemLifecycle::Completed),
            "incomplete" => Ok(ItemLifecycle::Incomplete),
            "in_progress" => Ok(ItemLifecycle::InProgress),
            _ => Err(CodecError::Invalid("item status")),
        },
        _ => Err(CodecError::Invalid("item status")),
    }
}
pub(super) fn status_label(s: ItemLifecycle) -> &'static str {
    match s {
        ItemLifecycle::Completed => "completed",
        ItemLifecycle::Incomplete => "incomplete",
        ItemLifecycle::InProgress => "in_progress",
    }
}
pub(super) fn direct(o: &Map<String, Value>) -> Result<(), CodecError> {
    if o.get("namespace")
        .is_some_and(|v| !v.is_null() && v.as_str() != Some(""))
    {
        return Err(CodecError::Unsupported("tool namespace".into()));
    }
    if o.get("async").is_some_and(|v| v.as_bool() != Some(false)) {
        return Err(CodecError::Unsupported("async tool".into()));
    }
    if o.get("caller")
        .is_some_and(|v| !v.is_null() && v != &json!({"type":"direct"}))
    {
        return Err(CodecError::Unsupported("programmatic caller".into()));
    }
    Ok(())
}
fn output(b: &mut Items, v: &Value) -> Result<ToolOutput, CodecError> {
    if let Some(s) = v.as_str() {
        if s.len() > MAX_TEXT_BYTES {
            return Err(CodecError::Limit);
        }
        return Ok(ToolOutput::Text(s.into()));
    }
    let a = v.as_array().ok_or(CodecError::Invalid("tool output"))?;
    if a.len() > MAX_ITEMS {
        return Err(CodecError::Limit);
    }
    let mut p = vec![];
    for v in a {
        let o = object(v)?;
        fields(o, &["type", "text", "prompt_cache_breakpoint"])?;
        if string(o, "type")? != "input_text" {
            return Err(CodecError::Unsupported("tool result media".into()));
        }
        let id = b.part_id()?;
        record_input_form(b, id, o)?;
        p.push((
            id,
            Text::allowing_empty(string(o, "text")?, "tool output", MAX_TEXT_BYTES)
                .map_err(|_| CodecError::Limit)?,
        ));
    }
    Ok(ToolOutput::Parts(p))
}
fn record_input_form(b: &mut Items, id: PartId, o: &Map<String, Value>) -> Result<(), CodecError> {
    b.fidelity.record_input_text(id)?;
    if let Some(v) = o.get("prompt_cache_breakpoint") {
        if v != &json!({"mode":"explicit"}) {
            return Err(CodecError::Invalid("prompt cache breakpoint"));
        }
        b.fidelity.record_cache_breakpoint(id)?;
    }
    Ok(())
}
fn input_part(id: PartId, text: &str, fidelity: &FidelityRecords) -> Value {
    let mut v = json!({"type":"input_text","text":text});
    if fidelity.cache_breakpoint(id) {
        v["prompt_cache_breakpoint"] = json!({"mode":"explicit"});
    }
    v
}
pub(super) fn decode_items(
    b: &mut Items,
    input: &[Value],
    response: bool,
    item_status: &str,
) -> Result<(), CodecError> {
    for value in input {
        let o = object(value)?;
        let id = b.id()?;
        let typ = o
            .get("type")
            .map(|v| v.as_str().ok_or(CodecError::Invalid("item type")))
            .transpose()?;
        let item = match typ {
            Some("function_call") => {
                direct(o)?;
                Item::ToolCall(tool_call(
                    o,
                    Profile::Responses,
                    None,
                    response.then_some(item_status),
                )?)
            }
            Some("custom_tool_call") => {
                fields(
                    o,
                    &[
                        "type",
                        "id",
                        "call_id",
                        "name",
                        "input",
                        "namespace",
                        "caller",
                        "async",
                    ],
                )?;
                direct(o)?;
                Item::CustomCall(CustomCall {
                    call_id: text(string(o, "call_id")?, "call id", 256)?,
                    name: text(string(o, "name")?, "custom name", 128)?,
                    input: raw_string(o, "input")?,
                })
            }
            Some("function_call_output" | "custom_tool_call_output") if !response => {
                fields(
                    o,
                    &[
                        "type",
                        "id",
                        "call_id",
                        "output",
                        "status",
                        "name",
                        "namespace",
                        "caller",
                    ],
                )?;
                direct(o)?;
                let call_id = string(o, "call_id")?;
                if let Some(name) = o.get("name").filter(|v| !v.is_null()) {
                    let expected = b.items.iter().find_map(|(_, i)| match i {
                        Item::ToolCall(c) if c.call_id.as_str() == call_id => Some(c.name.as_str()),
                        Item::CustomCall(c) if c.call_id.as_str() == call_id => {
                            Some(c.name.as_str())
                        }
                        _ => None,
                    });
                    if name.as_str() != expected {
                        return Err(CodecError::Invalid("tool result name"));
                    }
                }
                let r = ToolResult {
                    call_id: text(call_id, "call id", 256)?,
                    output: output(
                        b,
                        o.get("output").ok_or(CodecError::Invalid("tool output"))?,
                    )?,
                    status: if o.contains_key("status") {
                        Some(status(o, ItemLifecycle::Completed)?)
                    } else {
                        None
                    },
                };
                if typ == Some("custom_tool_call_output") {
                    if r.status.is_some() {
                        return Err(CodecError::Unsupported("custom result status".into()));
                    }
                    Item::CustomResult(r)
                } else {
                    Item::ToolResult(r)
                }
            }
            Some("reasoning") => {
                let (r, encrypted) = super::reasoning::decode_item(
                    o,
                    &mut || b.part_id(),
                    !response || item_status != "completed",
                )?;
                if let Some(t) = encrypted {
                    b.fidelity.record_replay(
                        id,
                        ReasoningReplay {
                            value: if r.status == ItemLifecycle::InProgress {
                                EncryptedReasoning::Partial(text(
                                    &t,
                                    "encrypted reasoning",
                                    MAX_TEXT_BYTES,
                                )?)
                            } else {
                                EncryptedReasoning::Final(text(
                                    &t,
                                    "encrypted reasoning",
                                    MAX_TEXT_BYTES,
                                )?)
                            },
                            origin: None,
                        },
                        &r,
                    )?;
                }
                Item::Reasoning(r)
            }
            Some("message") | None => {
                fields(
                    o,
                    if typ.is_none() {
                        &["role", "content"]
                    } else {
                        &["type", "id", "role", "content", "status"]
                    },
                )?;
                let role = string(o, "role")?;
                let content = o.get("content").ok_or(CodecError::Invalid("content"))?;
                let values = if let Some(s) = content.as_str() {
                    if response {
                        return Err(CodecError::Invalid("output content"));
                    }
                    vec![
                        json!({"type":if role=="assistant"{"output_text"}else{"input_text"},"text":s}),
                    ]
                } else {
                    content
                        .as_array()
                        .ok_or(CodecError::Invalid("content parts"))?
                        .clone()
                };
                if values.len() > MAX_ITEMS {
                    return Err(CodecError::Limit);
                }
                if role == "system" || role == "developer" {
                    // Instruction IR has no lifecycle owner; only complete forms normalize.
                    if status(o, ItemLifecycle::Completed)? != ItemLifecycle::Completed {
                        return Err(CodecError::Unsupported("instruction lifecycle".into()));
                    }
                    let mut parts = vec![];
                    for v in &values {
                        let p = object(v)?;
                        fields(p, &["type", "text", "prompt_cache_breakpoint"])?;
                        if string(p, "type")? != "input_text" {
                            return Err(CodecError::Invalid("instruction part"));
                        }
                        let part_id = b.part_id()?;
                        record_input_form(b, part_id, p)?;
                        parts.push((
                            part_id,
                            Text::allowing_empty(string(p, "text")?, "instruction", MAX_TEXT_BYTES)
                                .map_err(|_| CodecError::Limit)?,
                        ));
                    }
                    Item::Instruction(Instruction {
                        authority: if role == "system" {
                            InstructionAuthority::System
                        } else {
                            InstructionAuthority::Developer
                        },
                        parts,
                    })
                } else {
                    let role = match role {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        _ => return Err(CodecError::Unsupported("message role".into())),
                    };
                    let mut parts = vec![];
                    for v in &values {
                        let p = object(v)?;
                        let kind = string(p, "type")?;
                        let content = match kind {
                            "refusal" if role == MessageRole::Assistant => {
                                fields(p, &["type", "refusal"])?;
                                ContentPart::Refusal(
                                    Text::allowing_empty(
                                        string(p, "refusal")?,
                                        "refusal",
                                        MAX_TEXT_BYTES,
                                    )
                                    .map_err(|_| CodecError::Limit)?,
                                )
                            }
                            "input_text" if !response => {
                                fields(p, &["type", "text", "prompt_cache_breakpoint"])?;
                                ContentPart::Text(
                                    Text::allowing_empty(
                                        string(p, "text")?,
                                        "text",
                                        MAX_TEXT_BYTES,
                                    )
                                    .map_err(|_| CodecError::Limit)?
                                    .into(),
                                )
                            }
                            "output_text" if role == MessageRole::Assistant => {
                                ContentPart::Text(super::text::read(p, !response)?)
                            }
                            _ => return Err(CodecError::Unsupported("content part".into())),
                        };
                        let part_id = b.part_id()?;
                        if kind == "input_text" {
                            record_input_form(b, part_id, p)?;
                        }
                        parts.push(Part {
                            id: part_id,
                            content,
                        });
                    }
                    let status = status(o, ItemLifecycle::Completed)?;
                    if response && item_status == "completed" && status != ItemLifecycle::Completed
                    {
                        return Err(CodecError::Invalid("completed item"));
                    }
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
    let mut v = json!({"input":encode_items(target.semantic.items(),target.fidelity,false)});
    settings::write(
        target.semantic.settings(),
        v.as_object_mut().expect("object"),
    );
    bounded(&v)?;
    Ok(v)
}
fn output_wire(o: &ToolOutput, fidelity: &FidelityRecords) -> Value {
    match o {
        ToolOutput::Text(s) => json!(s),
        ToolOutput::Parts(p) => json!(
            p.iter()
                .map(|(id, t)| input_part(*id, t.as_str(), fidelity))
                .collect::<Vec<_>>()
        ),
    }
}
pub(super) fn encode_items(
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
    response: bool,
) -> Vec<Value> {
    let mut out = vec![];
    for (id, item) in items {
        let mut v = match item {
            Item::Instruction(i) => {
                json!({"role":match i.authority{InstructionAuthority::System=>"system",InstructionAuthority::Developer=>"developer"},"content":if let [(id,t)]=i.parts.as_slice() && !fidelity.cache_breakpoint(*id){json!(t.as_str())}else{json!(i.parts.iter().map(|(id,t)|input_part(*id,t.as_str(),fidelity)).collect::<Vec<_>>())}})
            }
            Item::Message(m)
                if m.parts.is_empty()
                    && items
                        .iter()
                        .any(|(_, i)| matches!(i,Item::ToolCall(c) if c.message==Some(*id))) =>
            {
                continue;
            }
            Item::Message(m) => {
                json!({"type":"message","role":if m.role==MessageRole::User{"user"}else{"assistant"},"content":m.parts.iter().map(|p|match &p.content{ContentPart::Text(t)=>if !response && (m.role==MessageRole::User || fidelity.input_text_form(p.id) && t.is_plain()){input_part(p.id,t.as_str(),fidelity)}else{super::text::write(t,"output_text",response)},ContentPart::Refusal(t)=>json!({"type":"refusal","refusal":t.as_str()}),ContentPart::Resource(_)=>unreachable!("lowering rejects media")}).collect::<Vec<_>>()})
            }
            Item::ToolCall(c) => {
                json!({"type":"function_call","call_id":c.call_id.as_str(),"name":c.name.as_str(),"arguments":c.arguments})
            }
            Item::CustomCall(c) => {
                json!({"type":"custom_tool_call","call_id":c.call_id.as_str(),"name":c.name.as_str(),"input":c.input})
            }
            Item::ToolResult(r) | Item::CustomResult(r) => {
                let mut v = json!({"type":if matches!(item,Item::CustomResult(_)){"custom_tool_call_output"}else{"function_call_output"},"call_id":r.call_id.as_str(),"output":output_wire(&r.output,fidelity)});
                if let Some(s) = r.status {
                    v["status"] = json!(status_label(s));
                }
                v
            }
            Item::Reasoning(r) => super::reasoning::encode_item(*id, r, fidelity, response),
        };
        if response {
            v["id"] = json!(
                fidelity
                    .response_item_id(*id)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("item_{}", id.get()))
            );
            if let Some(s) = item.lifecycle() {
                v["status"] = json!(status_label(s));
            }
        } else {
            if let Some(wire) = fidelity.response_item_id(*id) {
                if v.get("type").is_none() {
                    v["type"] = json!("message");
                }
                v["id"] = json!(wire);
            }
            if item
                .lifecycle()
                .is_some_and(|s| s != ItemLifecycle::Completed)
            {
                v["status"] = json!(status_label(item.lifecycle().expect("lifecycle")));
            }
        }
        out.push(v);
    }
    out
}
