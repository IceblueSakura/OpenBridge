//! Client usage scopes reference delivered wire identity; unknown counts stay absent.
use super::{owner, wire_id};
use crate::{
    protocol::{CodecError, fidelity::FidelityRecords, openai::common::*},
    semantic::task::generation::*,
};
use serde_json::{Value, json};
const FIELDS: &[&str] = &[
    "scope",
    "basis",
    "output_relation",
    "total_relation",
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "cached_input_tokens",
    "input_cache_write_tokens",
    "reasoning_tokens",
    "input_image_tokens",
    "input_audio_tokens",
    "output_audio_tokens",
    "input_text_tokens",
    "output_text_tokens",
    "accepted_prediction_tokens",
    "rejected_prediction_tokens",
];
pub(crate) fn standard(reports: &[Usage]) -> bool {
    reports.is_empty()
        || matches!(reports,[usage] if usage.scope==UsageScope::Operation && usage.basis==UsageBasis::Final && usage.output_relation==OutputTokenRelation::IncludesReasoning && usage.total_relation==TotalTokenRelation::InputAndOutput && usage.input_tokens.is_some() && usage.output_tokens.is_some() && usage.total_tokens.is_some() && usage.accepted_prediction_tokens.is_none() && usage.rejected_prediction_tokens.is_none() && usage.input_audio_tokens.is_none() && usage.output_audio_tokens.is_none())
}
pub(crate) fn read(
    root: Option<&Value>,
    items: &[(ItemId, Item)],
    fidelity: &FidelityRecords,
) -> Result<Option<Vec<Usage>>, CodecError> {
    let Some(reports) = root.and_then(|root| root.get("usage")) else {
        return Ok(None);
    };
    let reports = reports
        .as_array()
        .filter(|reports| !reports.is_empty() && reports.len() <= MAX_ITEMS)
        .ok_or(CodecError::Invalid("client usage reports"))?;
    let mut output = Vec::with_capacity(reports.len());
    for report in reports {
        let value = object(report)?;
        fields(value, FIELDS)?;
        let scope = match value.get("scope") {
            Some(Value::String(value)) if value == "operation" => UsageScope::Operation,
            Some(Value::String(value)) if value == "session" => UsageScope::Session,
            Some(Value::Object(value)) => {
                fields(value, &["item"])?;
                UsageScope::Item(owner(string(value, "item")?, items, fidelity)?)
            }
            _ => return Err(CodecError::Invalid("usage scope")),
        };
        let mut usage = Usage {
            scope,
            basis: match string(value, "basis")? {
                "delta" => UsageBasis::Delta,
                "cumulative" => UsageBasis::Cumulative,
                "final" => UsageBasis::Final,
                _ => return Err(CodecError::Invalid("usage basis")),
            },
            output_relation: match string(value, "output_relation")? {
                "unreported" => OutputTokenRelation::Unreported,
                "includes_reasoning" => OutputTokenRelation::IncludesReasoning,
                "excludes_reasoning" => OutputTokenRelation::ExcludesReasoning,
                _ => return Err(CodecError::Invalid("usage output relation")),
            },
            total_relation: match string(value, "total_relation")? {
                "unreported" => TotalTokenRelation::Unreported,
                "input_and_output" => TotalTokenRelation::InputAndOutput,
                "input_output_and_reasoning" => TotalTokenRelation::InputOutputAndReasoning,
                _ => return Err(CodecError::Invalid("usage total relation")),
            },
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            cached_input_tokens: None,
            input_cache_write_tokens: None,
            reasoning_tokens: None,
            input_image_tokens: None,
            input_audio_tokens: None,
            output_audio_tokens: None,
            input_text_tokens: None,
            output_text_tokens: None,
            accepted_prediction_tokens: None,
            rejected_prediction_tokens: None,
        };
        macro_rules! counters {($($field:ident),*)=>{$(usage.$field=value.get(stringify!($field)).map(|value|value.as_u64().ok_or(CodecError::Invalid("usage count"))).transpose()?;)*};}
        counters!(
            input_tokens,
            output_tokens,
            total_tokens,
            cached_input_tokens,
            input_cache_write_tokens,
            reasoning_tokens,
            input_image_tokens,
            input_audio_tokens,
            output_audio_tokens,
            input_text_tokens,
            output_text_tokens,
            accepted_prediction_tokens,
            rejected_prediction_tokens
        );
        usage.validate()?;
        output.push(usage);
    }
    Ok(Some(output))
}
pub(crate) fn write(root: &mut Value, reports: &[Usage], fidelity: &FidelityRecords) {
    if standard(reports) {
        return;
    }
    let reports:Vec<_>=reports.iter().map(|usage| {
        let scope=match usage.scope {UsageScope::Operation=>json!("operation"),UsageScope::Session=>json!("session"),UsageScope::Item(id)=>json!({"item":wire_id(id,fidelity)})};
        let basis=match usage.basis {UsageBasis::Delta=>"delta",UsageBasis::Cumulative=>"cumulative",UsageBasis::Final=>"final"};
        let output=match usage.output_relation {OutputTokenRelation::Unreported=>"unreported",OutputTokenRelation::IncludesReasoning=>"includes_reasoning",OutputTokenRelation::ExcludesReasoning=>"excludes_reasoning"};
        let total=match usage.total_relation {TotalTokenRelation::Unreported=>"unreported",TotalTokenRelation::InputAndOutput=>"input_and_output",TotalTokenRelation::InputOutputAndReasoning=>"input_output_and_reasoning"};
        let mut value=json!({"scope":scope,"basis":basis,"output_relation":output,"total_relation":total});
        macro_rules! counters {($($field:ident),*)=>{$(if let Some(count)=usage.$field {value[stringify!($field)]=json!(count);})*};}
        counters!(input_tokens,output_tokens,total_tokens,cached_input_tokens,input_cache_write_tokens,reasoning_tokens,input_image_tokens,input_audio_tokens,output_audio_tokens,input_text_tokens,output_text_tokens,accepted_prediction_tokens,rejected_prediction_tokens);
        value
    }).collect();
    if root.get(super::FIELD).is_none() {
        root[super::FIELD] = json!({"version":1});
    }
    root[super::FIELD]["usage"] = json!(reports);
    root["usage"] = Value::Null;
}
