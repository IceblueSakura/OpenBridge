//! Responses input lowering keeps independent items independent, including tool result JSON.

use super::*;

pub(super) fn encode(
    request: &WireRequest,
    old: &GenerationRequest,
    target: &mut Map<String, Value>,
) -> Result<(), StaticCodecError> {
    let has_instructions = request.source.contains_key("instructions");
    let raw_input = &request.source["input"];
    let raw: Vec<Value> = match raw_input {
        Value::String(text) => vec![json!({"role":"user","content":text})],
        Value::Array(items) => items.clone(),
        _ => return Err(StaticCodecError::InvalidShape),
    };
    if old.input().len() != raw.len() + usize::from(has_instructions) {
        return Err(StaticCodecError::UnsupportedSemantics);
    }
    let mut raw = raw;
    if has_instructions {
        raw.insert(
            0,
            json!({"role":"system","content":request.source["instructions"]}),
        );
    }
    let originals: BTreeMap<_, _> = old
        .input_identities()
        .iter()
        .zip(old.input())
        .zip(&raw)
        .map(|((identity, item), wire)| (identity.id(), (identity, item, wire)))
        .collect();
    let mut used_ids: BTreeSet<String> = raw
        .iter()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect();
    target.remove("instructions");
    let mut input = Vec::new();
    for (position, entries) in groups(&request.semantic).into_iter().enumerate() {
        let [(identity, item)] = entries.as_slice() else {
            return Err(StaticCodecError::UnsupportedSemantics);
        };
        let previous = originals.get(&identity.id());
        if previous.is_some_and(|(old, _, _)| old.group() != identity.group()) {
            return Err(StaticCodecError::UnsupportedSemantics);
        }
        let old_item = previous.map(|(_, item, _)| *item);
        let source = previous.map(|(_, _, wire)| *wire);
        if position == 0
            && has_instructions
            && old.input_identities()[0].id() == identity.id()
            && let InputItem::Instruction(value) = item
            && value.authority() == InstructionAuthority::System
            && matches!(
                value.origin(),
                InstructionOrigin::Downstream | InstructionOrigin::GatewayPolicy
            )
        {
            target.insert("instructions".into(), json!(value.text().as_str()));
            continue;
        }
        if old_item == Some(*item) {
            input.push(source.unwrap().clone());
            continue;
        }
        let value = match item {
            InputItem::Instruction(value) => instruction(
                value,
                match old_item {
                    Some(InputItem::Instruction(old)) => Some(old),
                    _ => None,
                },
                source,
                false,
            )?,
            InputItem::Message(value) => message(
                value,
                match old_item {
                    Some(InputItem::Message(old)) => Some(old),
                    _ => None,
                },
                source,
                false,
            )?,
            InputItem::PriorToolCall(value) => {
                let old = match old_item {
                    Some(InputItem::PriorToolCall(old)) => Some(old),
                    _ => None,
                };
                let mut value = native_call(value, old, source, false)?;
                if source.is_none() {
                    let mut serial = position;
                    loop {
                        let id = format!("ob_input_call_{serial}");
                        if used_ids.insert(id.clone()) {
                            value["id"] = json!(id);
                            break;
                        }
                        serial += 1;
                    }
                }
                value
            }
            InputItem::ToolResult(value) => {
                let mut encoded = source
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                only_fields(&encoded, &["type", "id", "call_id", "output"])?;
                encoded.insert("type".into(), json!("function_call_output"));
                encoded.insert("call_id".into(), json!(value.call_id().as_str()));
                encoded.insert("output".into(), tool_result(value)?);
                Value::Object(encoded)
            }
            _ => return Err(StaticCodecError::UnsupportedSemantics),
        };
        input.push(value);
    }
    // Retain the scalar user-text spelling only when it still represents the complete input.
    let scalar = raw_input.is_string()
        && input.len() == 1
        && input[0]["role"] == "user"
        && input[0]["content"].is_string();
    target.insert(
        "input".into(),
        if scalar {
            input[0]["content"].clone()
        } else {
            Value::Array(input)
        },
    );
    Ok(())
}
