//! Closed function/custom tool syntax; active orchestration features are outside this profile.
use super::{CodecError, Profile, common::*};
use crate::semantic::task::generation::*;
use serde_json::{Map, Value, json};
fn inactive(o: &Map<String, Value>) -> Result<(), CodecError> {
    for key in ["async", "defer_loading"] {
        if o.get(key).is_some_and(|v| v.as_bool() != Some(false)) {
            return Err(CodecError::Unsupported(key.into()));
        }
    }
    if o.get("allowed_callers")
        .is_some_and(|v| !v.is_null() && v != &json!(["direct"]))
    {
        return Err(CodecError::Unsupported("allowed_callers".into()));
    }
    Ok(())
}
pub(super) fn read_settings(
    o: &Map<String, Value>,
    profile: Profile,
    settings: &mut GenerationSettings,
) -> Result<(), CodecError> {
    settings.tools = o
        .get("tools")
        .filter(|v| !v.is_null())
        .map(|v| {
            let a = v.as_array().ok_or(CodecError::Invalid("tools"))?;
            if a.len() > MAX_TOOLS {
                return Err(CodecError::Limit);
            }
            a.iter()
                .map(|v| read_tool(object(v)?, profile))
                .collect::<Result<Vec<_>, CodecError>>()
        })
        .transpose()?;
    settings.tool_choice = o
        .get("tool_choice")
        .filter(|v| !v.is_null())
        .map(|v| read_choice(v, profile))
        .transpose()?;
    settings.parallel_tool_calls = nullable_bool(o, "parallel_tool_calls")?;
    settings.validate()?;
    Ok(())
}
pub(super) fn read_tool(
    o: &Map<String, Value>,
    profile: Profile,
) -> Result<ToolDefinition, CodecError> {
    if string(o, "type")? == "custom" && profile == Profile::Responses {
        fields(
            o,
            &[
                "type",
                "name",
                "description",
                "format",
                "async",
                "defer_loading",
                "allowed_callers",
            ],
        )?;
        inactive(o)?;
        let format = o
            .get("format")
            .map(|v| {
                let f = object(v)?;
                Ok::<_, CodecError>(match string(f, "type")? {
                    "text" => {
                        fields(f, &["type"])?;
                        CustomFormat::Text
                    }
                    "grammar" => {
                        fields(f, &["type", "syntax", "definition"])?;
                        CustomFormat::Grammar {
                            syntax: match string(f, "syntax")? {
                                "lark" => GrammarSyntax::Lark,
                                "regex" => GrammarSyntax::Regex,
                                _ => return Err(CodecError::Invalid("grammar syntax")),
                            },
                            definition: text(string(f, "definition")?, "grammar", MAX_TEXT_BYTES)?,
                        }
                    }
                    _ => return Err(CodecError::Invalid("custom format")),
                })
            })
            .transpose()?;
        return Ok(ToolDefinition::Custom(CustomTool {
            name: text(string(o, "name")?, "custom tool name", 128)?,
            description: o
                .get("description")
                .filter(|v| !v.is_null())
                .map(|_| raw_string(o, "description"))
                .transpose()?,
            format,
        }));
    }
    if string(o, "type")? != "function" {
        return Err(CodecError::Unsupported("tool kind".into()));
    }
    let f = if profile == Profile::Chat {
        fields(o, &["type", "function"])?;
        object(o.get("function").ok_or(CodecError::Invalid("function"))?)?
    } else {
        o
    };
    fields(
        f,
        if profile == Profile::Chat {
            &["name", "description", "parameters", "strict"]
        } else {
            &[
                "type",
                "name",
                "description",
                "parameters",
                "strict",
                "output_schema",
                "async",
                "defer_loading",
                "allowed_callers",
            ]
        },
    )?;
    inactive(f)?;
    Ok(ToolDefinition::Function(FunctionTool {
        name: text(string(f, "name")?, "function name", 128)?,
        description: f
            .get("description")
            .filter(|v| !v.is_null())
            .map(|_| raw_string(f, "description"))
            .transpose()?,
        parameters: f.get("parameters").filter(|v| !v.is_null()).cloned(),
        strict: match nullable_bool(f, "strict")? {
            Some(v) => FunctionStrictness::Explicit(v),
            None => FunctionStrictness::Omitted(strict_default(profile)),
        },
        output_schema: f.get("output_schema").filter(|v| !v.is_null()).cloned(),
    }))
}
fn reference(v: &Value) -> Result<ToolReference, CodecError> {
    let o = object(v)?;
    fields(o, &["type", "name"])?;
    Ok(ToolReference {
        kind: match string(o, "type")? {
            "function" => ToolKind::Function,
            "custom" => ToolKind::Custom,
            _ => return Err(CodecError::Unsupported("tool reference".into())),
        },
        name: text(string(o, "name")?, "tool reference", 128)?,
    })
}
fn read_choice(v: &Value, profile: Profile) -> Result<ToolChoice, CodecError> {
    Ok(match v.as_str() {
        Some("auto") => ToolChoice::Auto,
        Some("none") => ToolChoice::None,
        Some("required") => ToolChoice::Required,
        Some(_) => return Err(CodecError::Invalid("tool choice")),
        None => {
            let o = object(v)?;
            if profile == Profile::Chat {
                fields(o, &["type", "function"])?;
                if string(o, "type")? != "function" {
                    return Err(CodecError::Unsupported("tool choice".into()));
                }
                let f = object(
                    o.get("function")
                        .ok_or(CodecError::Invalid("function choice"))?,
                )?;
                fields(f, &["name"])?;
                ToolChoice::Specific(text(string(f, "name")?, "chosen tool", 128)?)
            } else if string(o, "type")? == "allowed_tools" {
                fields(o, &["type", "mode", "tools"])?;
                let required = match string(o, "mode")? {
                    "auto" => false,
                    "required" => true,
                    _ => return Err(CodecError::Invalid("allowed tools mode")),
                };
                let a = o
                    .get("tools")
                    .and_then(Value::as_array)
                    .ok_or(CodecError::Invalid("allowed tools"))?;
                if a.len() > MAX_TOOLS {
                    return Err(CodecError::Limit);
                }
                ToolChoice::Allowed {
                    required,
                    tools: a.iter().map(reference).collect::<Result<_, _>>()?,
                }
            } else {
                let r = reference(v)?;
                match r.kind {
                    ToolKind::Function => ToolChoice::Specific(r.name),
                    ToolKind::Custom => ToolChoice::Custom(r.name),
                }
            }
        }
    })
}
pub(super) fn decode(
    r: GenerationRequest,
    o: &Map<String, Value>,
    profile: Profile,
) -> Result<GenerationRequest, CodecError> {
    let mut s = r.settings().clone();
    read_settings(o, profile, &mut s)?;
    Ok(r.with_settings(s)?)
}
pub(super) fn encode(r: &GenerationRequest, profile: Profile, o: &mut Map<String, Value>) {
    write_settings(r.settings(), profile, o)
}
pub(super) fn write_settings(s: &GenerationSettings, profile: Profile, o: &mut Map<String, Value>) {
    if let Some(tools) = &s.tools {
        o.insert(
            "tools".into(),
            json!(
                tools
                    .iter()
                    .map(|t| write_tool(t, profile))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if let Some(c) = &s.tool_choice {
        o.insert("tool_choice".into(), write_choice(c, profile));
    }
    if let Some(v) = s.parallel_tool_calls {
        o.insert("parallel_tool_calls".into(), json!(v));
    }
}
pub(super) fn write_tool(t: &ToolDefinition, profile: Profile) -> Value {
    match t {
        ToolDefinition::Function(t) => {
            let mut f = json!({"name":t.name.as_str()});
            if let Some(d) = &t.description {
                f["description"] = json!(d);
            }
            if let Some(p) = &t.parameters {
                f["parameters"] = p.clone();
            }
            if let Some(p) = &t.output_schema {
                f["output_schema"] = p.clone();
            }
            if let FunctionStrictness::Explicit(b) = t.strict {
                f["strict"] = json!(b);
            }
            if profile == Profile::Chat {
                json!({"type":"function","function":f})
            } else {
                f["type"] = json!("function");
                f
            }
        }
        ToolDefinition::Custom(t) => {
            let mut v = json!({"type":"custom","name":t.name.as_str()});
            if let Some(d) = &t.description {
                v["description"] = json!(d);
            }
            if let Some(format) = &t.format {
                v["format"] = match format {
                    CustomFormat::Text => json!({"type":"text"}),
                    CustomFormat::Grammar { syntax, definition } => {
                        json!({"type":"grammar","syntax":match syntax{GrammarSyntax::Lark=>"lark",GrammarSyntax::Regex=>"regex"},"definition":definition.as_str()})
                    }
                };
            }
            v
        }
    }
}
pub(super) fn write_choice(c: &ToolChoice, profile: Profile) -> Value {
    match c {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::None => json!("none"),
        ToolChoice::Required => json!("required"),
        ToolChoice::Specific(n) if profile == Profile::Chat => {
            json!({"type":"function","function":{"name":n.as_str()}})
        }
        ToolChoice::Specific(n) => json!({"type":"function","name":n.as_str()}),
        ToolChoice::Custom(n) => json!({"type":"custom","name":n.as_str()}),
        ToolChoice::Allowed { required, tools } => {
            json!({"type":"allowed_tools","mode":if *required{"required"}else{"auto"},"tools":tools.iter().map(|r|json!({"type":match r.kind{ToolKind::Function=>"function",ToolKind::Custom=>"custom"},"name":r.name.as_str()})).collect::<Vec<_>>()})
        }
    }
}
