//! Named wire mappings and classified router facts. No provider identity branches.
//! Task values are decoded by the shared codecs; unknown fields remain errors.
use super::{CodecError, Profile, common::bounded};
use crate::protocol::adaptation::Adaptation;
use serde_json::{Map, Value};
use std::{borrow::Cow, collections::BTreeMap};

pub(crate) type Extras = BTreeMap<String, Value>;
const BUDGET: usize = 4096;

pub(crate) fn check_budget(values: &Extras) -> Result<(), CodecError> {
    let mut used = 0usize;
    for (key, value) in values {
        used = used.saturating_add(key.len()).saturating_add(
            crate::semantic::value::json_size(value, BUDGET).map_err(|_| CodecError::Limit)?,
        );
        if used > BUDGET {
            return Err(CodecError::Limit);
        }
    }
    Ok(())
}
fn money(value: &Value) -> bool {
    value.is_null() || value.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0)
}
fn take(o: &mut Map<String, Value>, name: &str, path: &str, extras: &mut Extras) {
    if let Some(value) = o.shift_remove(name) {
        extras.insert(path.into(), value);
    }
}
/// Only the explicit billing schema is opaque fidelity; token counts remain typed.
fn extract(o: &mut Map<String, Value>, profile: Profile) -> Result<Extras, CodecError> {
    let mut extras = Extras::new();
    if let Some(value) = o.get("provider") {
        if !(value.is_null() || value.as_str().is_some_and(|s| !s.is_empty())) {
            return Err(CodecError::Invalid("reported provider"));
        }
        take(o, "provider", "/provider", &mut extras);
    }
    if let Some(usage) = o.get_mut("usage").and_then(Value::as_object_mut) {
        if usage.get("cost").is_some_and(|v| !money(v))
            || usage
                .get("is_byok")
                .is_some_and(|v| !v.is_null() && !v.is_boolean())
        {
            return Err(CodecError::Invalid("billing facts"));
        }
        if let Some(details) = usage.get("cost_details").filter(|v| !v.is_null()) {
            let details = details
                .as_object()
                .ok_or(CodecError::Invalid("cost details"))?;
            for (key, value) in details {
                if ![
                    "upstream_inference_cost",
                    "upstream_inference_prompt_cost",
                    "upstream_inference_completions_cost",
                    "upstream_inference_input_cost",
                    "upstream_inference_output_cost",
                ]
                .contains(&key.as_str())
                    || !money(value)
                {
                    return Err(CodecError::Unsupported("cost detail".into()));
                }
            }
        }
        // The text-only profile can normalize explicit zero modality counts;
        // nonzero, null or malformed counts need a real semantic mapping.
        for (details, names) in [
            (
                "prompt_tokens_details",
                &["audio_tokens", "video_tokens"][..],
            ),
            (
                "completion_tokens_details",
                &["audio_tokens", "image_tokens"][..],
            ),
        ] {
            if let Some(details) = usage.get_mut(details).and_then(Value::as_object_mut) {
                for name in names {
                    if let Some(value) = details.shift_remove(*name)
                        && value.as_u64() != Some(0)
                    {
                        return Err(CodecError::Unsupported("nonzero modality usage".into()));
                    }
                }
            }
        }
        for name in ["cost", "is_byok", "cost_details"] {
            take(usage, name, &format!("/usage/{name}"), &mut extras);
        }
    }
    // Never turn an unrepresentable multi-candidate response into one candidate.
    if profile == Profile::Chat
        && let Some(choices) = o.get_mut("choices").and_then(Value::as_array_mut)
        && choices.len() == 1
        && let Some(choice) = choices[0].as_object_mut()
    {
        if choice
            .get("native_finish_reason")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        {
            return Err(CodecError::Invalid("native finish reason"));
        }
        take(
            choice,
            "native_finish_reason",
            "/choices/0/native_finish_reason",
            &mut extras,
        );
    }
    check_budget(&extras)?;
    Ok(extras)
}
pub(crate) fn decode_message(o: &mut Map<String, Value>) -> Result<(), CodecError> {
    // Nonempty structured reasoning needs a typed owner/replay mapping, not a drop.
    if o.get("reasoning_details")
        .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
    {
        return Ok(()); // The structured codec owns both the details and derived view.
    }
    o.shift_remove("reasoning_details");
    if let Some(reasoning) = o.shift_remove("reasoning") {
        if !reasoning.is_null() && !reasoning.is_string() {
            return Err(CodecError::Invalid("readable reasoning"));
        }
        if o.get("reasoning_content")
            .is_some_and(|old| old != &reasoning)
        {
            return Err(CodecError::Invalid("reasoning alias conflict"));
        }
        o.insert("reasoning_content".into(), reasoning);
    }
    Ok(())
}
pub(crate) fn decode<'a>(
    value: &'a Value,
    profile: Profile,
    adaptation: &Adaptation,
) -> Result<(Cow<'a, Value>, Extras), CodecError> {
    bounded(value)?;
    if !adaptation.rules.routing_extras
        && !adaptation.rules.reasoning_alias
        && !adaptation.rules.responses_reasoning_format
        && !adaptation.rules.chat_stop_diagnostics
        && !adaptation.rules.reported_request_id
        && !adaptation.rules.zero_usage_details
        && !adaptation.rules.inactive_chat_fields
        && !adaptation.rules.responses_usage_detail_view
        && !adaptation.rules.text_usage_total_view
    {
        return Ok((Cow::Borrowed(value), Extras::new()));
    }
    let mut value = value.clone();
    let o = value
        .as_object_mut()
        .ok_or(CodecError::Invalid("response object"))?;
    if let Some(usage) = o.get_mut("usage").and_then(Value::as_object_mut) {
        if profile == Profile::Responses
            && adaptation.rules.responses_usage_detail_view
            && let Some(view) = usage.shift_remove("prompt_tokens_details")
            && (!view.is_object() || usage.get("input_tokens_details") != Some(&view))
        {
            return Err(CodecError::Invalid("duplicate usage details"));
        }
        if profile == Profile::Chat && adaptation.rules.text_usage_total_view {
            for (details, total) in [
                ("prompt_tokens_details", "prompt_tokens"),
                ("completion_tokens_details", "completion_tokens"),
            ] {
                let total = usage.get(total).and_then(Value::as_u64);
                if let Some(details) = usage.get_mut(details).and_then(Value::as_object_mut)
                    && let Some(view) = details.shift_remove("text_tokens")
                    && (view.as_u64().is_none() || view.as_u64() != total)
                {
                    // A real modality breakdown needs typed ownership; never drop it.
                    return Err(CodecError::Unsupported("nonredundant text usage".into()));
                }
            }
        }
    }
    let mut extras = if adaptation.rules.routing_extras {
        extract(o, profile)?
    } else {
        Extras::new()
    };
    if profile == Profile::Chat {
        if adaptation.rules.inactive_chat_fields
            && let Some(choices) = o.get_mut("choices").and_then(Value::as_array_mut)
        {
            for choice in choices {
                for field in ["message", "delta"] {
                    if let Some(message) = choice.get_mut(field).and_then(Value::as_object_mut) {
                        for name in ["audio", "function_call"] {
                            if let Some(value) = message.shift_remove(name)
                                && !value.is_null()
                            {
                                return Err(CodecError::Unsupported(
                                    "active message extension".into(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        if adaptation.rules.reported_request_id {
            if o.get("request_id")
                .is_some_and(|v| !v.is_null() && !v.as_str().is_some_and(|s| !s.is_empty()))
            {
                return Err(CodecError::Invalid("reported request id"));
            }
            take(o, "request_id", "/request_id", &mut extras);
        }
        if adaptation.rules.chat_stop_diagnostics {
            // lastOne is advisory framing metadata, never a semantic terminal.
            if let Some(marker) = o.shift_remove("lastOne")
                && !marker.is_boolean()
            {
                return Err(CodecError::Invalid("lastOne"));
            }
            let static_body = o.get("object").and_then(Value::as_str) == Some("chat.completion");
            if let Some(choices) = o.get_mut("choices").and_then(Value::as_array_mut)
                && choices.len() == 1
                && let Some(choice) = choices[0].as_object_mut()
            {
                if choice
                    .get("matched_stop")
                    .is_some_and(|v| !v.is_null() && v.as_u64().is_none())
                {
                    return Err(CodecError::Invalid("matched stop"));
                }
                take(
                    choice,
                    "matched_stop",
                    "/choices/0/matched_stop",
                    &mut extras,
                );
                if static_body
                    && let Some(delta) = choice.shift_remove("delta")
                    && !delta.is_null()
                {
                    return Err(CodecError::Unsupported("static delta".into()));
                }
            }
        }
        if adaptation.rules.zero_usage_details
            && let Some(usage) = o.get_mut("usage").and_then(Value::as_object_mut)
        {
            for name in ["prompt_tokens_details", "completion_tokens_details"] {
                if let Some(details) = usage.get_mut(name).and_then(Value::as_object_mut) {
                    for name in [
                        "audio_tokens",
                        "image_tokens",
                        "video_tokens",
                        "text_tokens",
                    ] {
                        if let Some(value) = details.shift_remove(name)
                            && value.as_u64() != Some(0)
                        {
                            return Err(CodecError::Unsupported("nonzero unmapped usage".into()));
                        }
                    }
                }
            }
        }
    }
    check_budget(&extras)?;
    if profile == Profile::Chat
        && adaptation.rules.reasoning_alias
        && let Some(choices) = o.get_mut("choices").and_then(Value::as_array_mut)
    {
        for choice in choices {
            for field in ["message", "delta"] {
                if let Some(message) = choice.get_mut(field).and_then(Value::as_object_mut) {
                    decode_message(message)?;
                }
            }
        }
    }
    if profile == Profile::Responses
        && adaptation.rules.responses_reasoning_format
        && let Some(items) = o.get_mut("output").and_then(Value::as_array_mut)
    {
        for item in items {
            reasoning_marker(item)?;
        }
    }
    Ok((Cow::Owned(value), extras))
}
/// A known syntax marker is redundant with the admitted Responses reasoning
/// codec. Unknown formats must not borrow that codec's replay semantics.
pub(crate) fn reasoning_marker(value: &mut Value) -> Result<(), CodecError> {
    if value.get("type").and_then(Value::as_str) == Some("reasoning")
        && let Some(format) = value
            .as_object_mut()
            .expect("object")
            .shift_remove("format")
        && format.as_str() != Some("openai-responses-v1")
    {
        return Err(CodecError::Unsupported("reasoning format".into()));
    }
    Ok(())
}

/// Project the final typed readable reasoning carrier to the selected spelling.
pub(crate) fn encode_message(value: &mut Value) {
    if let Some(o) = value.as_object_mut()
        && let Some(reasoning) = o.shift_remove("reasoning_content")
    {
        o.insert("reasoning".into(), reasoning);
    }
}
pub(crate) fn write_extras(o: &mut Map<String, Value>, values: &Extras) {
    for (path, value) in values {
        match path.as_str() {
            "/provider" | "/request_id" => {
                o.insert(path.trim_start_matches('/').into(), value.clone());
            }
            "/usage/cost" | "/usage/is_byok" | "/usage/cost_details" => {
                if let Some(usage) = o.get_mut("usage").and_then(Value::as_object_mut) {
                    usage.insert(
                        path.rsplit('/').next().expect("fixed path").into(),
                        value.clone(),
                    );
                }
            }
            "/choices/0/native_finish_reason" | "/choices/0/matched_stop" => {
                if let Some(choice) = o
                    .get_mut("choices")
                    .and_then(Value::as_array_mut)
                    .and_then(|a| a.first_mut())
                    .and_then(Value::as_object_mut)
                {
                    choice.insert(
                        path.rsplit('/').next().expect("fixed path").into(),
                        value.clone(),
                    );
                }
            }
            _ => unreachable!("closed classified fields"),
        }
    }
}
