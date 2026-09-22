//! Protocol shape and presence for client-executed function declarations and policy.
use super::{CodecError, Profile, common::*};
use crate::semantic::task::generation::*;
use serde_json::{Map, Value, json};

pub(super) fn decode(
    mut r: GenerationRequest,
    o: &Map<String, Value>,
    profile: Profile,
) -> Result<GenerationRequest, CodecError> {
    let tools = o
        .get("tools")
        .map(|v| {
            let a = v.as_array().ok_or(CodecError::Invalid("tools"))?;
            if a.len() > MAX_TOOLS {
                return Err(CodecError::Limit);
            }
            a.iter()
                .map(|t| {
                    let outer = object(t)?;
                    if string(outer, "type")? != "function" {
                        return Err(CodecError::Unsupported("tool kind".into()));
                    }
                    let f = match profile {
                        Profile::Chat => {
                            fields(outer, &["type", "function"])?;
                            object(
                                outer
                                    .get("function")
                                    .ok_or(CodecError::Invalid("function"))?,
                            )?
                        }
                        Profile::Responses => outer,
                    };
                    fields(
                        f,
                        if profile == Profile::Chat {
                            &["name", "description", "parameters", "strict"]
                        } else {
                            &["type", "name", "description", "parameters", "strict"]
                        },
                    )?;
                    let strict = match optional_bool(f, "strict")? {
                        Some(b) => FunctionStrictness::Explicit(b),
                        None => FunctionStrictness::Omitted(strict_default(profile)),
                    };
                    let description = f
                        .get("description")
                        .map(|_| raw_string(f, "description"))
                        .transpose()?;
                    let parameters = f.get("parameters").cloned();
                    Ok(ToolDefinition::Function(FunctionTool {
                        name: text(string(f, "name")?, "function name", 128)?,
                        description,
                        parameters,
                        strict,
                    }))
                })
                .collect::<Result<Vec<_>, CodecError>>()
        })
        .transpose()?;
    let choice = o
        .get("tool_choice")
        .map(|v| match v.as_str() {
            Some("none") => Ok(ToolChoice::None),
            Some("auto") => Ok(ToolChoice::Auto),
            Some("required") => Ok(ToolChoice::Required),
            Some(_) => Err(CodecError::Invalid("tool_choice")),
            None => {
                let c = object(v)?;
                if string(c, "type")? != "function" {
                    return Err(CodecError::Unsupported("tool choice kind".into()));
                }
                let name = match profile {
                    Profile::Chat => {
                        fields(c, &["type", "function"])?;
                        let f = object(
                            c.get("function")
                                .ok_or(CodecError::Invalid("function choice"))?,
                        )?;
                        fields(f, &["name"])?;
                        string(f, "name")?
                    }
                    Profile::Responses => {
                        fields(c, &["type", "name"])?;
                        string(c, "name")?
                    }
                };
                Ok(ToolChoice::Specific(text(name, "chosen function", 128)?))
            }
        })
        .transpose()?;
    r = r.with_tool_settings(tools, choice, optional_bool(o, "parallel_tool_calls")?)?;
    Ok(r)
}
pub(super) fn encode(r: &GenerationRequest, profile: Profile, o: &mut Map<String, Value>) {
    if r.tools_present() {
        let tools: Vec<_> = r
            .tools()
            .iter()
            .map(|ToolDefinition::Function(t)| {
                let mut f = json!({"name":t.name.as_str()});
                let object = f.as_object_mut().expect("object literal");
                if let Some(d) = &t.description {
                    object.insert("description".into(), json!(d));
                }
                if let Some(p) = &t.parameters {
                    object.insert("parameters".into(), p.clone());
                }
                if let FunctionStrictness::Explicit(b) = t.strict {
                    object.insert("strict".into(), json!(b));
                }
                if profile == Profile::Chat {
                    json!({"type":"function","function":f})
                } else {
                    object.insert("type".into(), json!("function"));
                    f
                }
            })
            .collect();
        o.insert("tools".into(), json!(tools));
    }
    if let Some(choice) = r.tool_choice() {
        let v = match choice {
            ToolChoice::None => json!("none"),
            ToolChoice::Auto => json!("auto"),
            ToolChoice::Required => json!("required"),
            ToolChoice::Specific(n) if profile == Profile::Chat => {
                json!({"type":"function","function":{"name":n.as_str()}})
            }
            ToolChoice::Specific(n) => json!({"type":"function","name":n.as_str()}),
        };
        o.insert("tool_choice".into(), v);
    }
    if let Some(b) = r.parallel_tool_calls() {
        o.insert("parallel_tool_calls".into(), json!(b));
    }
}
