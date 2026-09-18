//! Sampling-control ownership for Native requests. Wire hints preserve representation, not values.

#[cfg(test)]
mod tests;

use serde_json::{Map, Value, json};

use super::{StaticCodecError, parse_f64, parse_u64};
use crate::{
    core::ApiProtocol,
    ir::generation::{GenerationControls, ParallelToolCalls, StopSequence},
};

/// Decodes controls without collapsing an explicit empty stop list into omission.
pub(super) fn decode(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
    parallel: ParallelToolCalls,
    max_bytes: usize,
) -> Result<GenerationControls, StaticCodecError> {
    // Token aliases retain their existing precedence; migrating their distinct wire contracts is
    // separate from sampling controls.
    let max_output_tokens = match protocol {
        ApiProtocol::ChatCompletions => source
            .get("max_completion_tokens")
            .or_else(|| source.get("max_tokens")),
        ApiProtocol::Responses => source.get("max_output_tokens"),
    }
    .map(parse_u64)
    .transpose()?;
    let temperature = source.get("temperature").map(parse_f64).transpose()?;
    let top_p = source.get("top_p").map(parse_f64).transpose()?;
    let top_k = present(source, "top_k").map(parse_u64).transpose()?;
    let candidate_count = present(source, "n")
        .map(|value| u32::try_from(parse_u64(value)?).map_err(|_| StaticCodecError::InvalidShape))
        .transpose()?;
    let seed = present(source, "seed")
        .map(|value| value.as_i64().ok_or(StaticCodecError::InvalidShape))
        .transpose()?;
    let frequency = present(source, "frequency_penalty")
        .map(parse_f64)
        .transpose()?;
    let presence = present(source, "presence_penalty")
        .map(parse_f64)
        .transpose()?;
    let stop = present(source, "stop")
        .map(|value| decode_stop(value, max_bytes))
        .transpose()?;
    GenerationControls::new(max_output_tokens, candidate_count)
        .and_then(|controls| controls.with_sampling(temperature, top_p, top_k))
        .and_then(|controls| controls.with_penalties(frequency, presence))
        .map(|controls| {
            controls
                .with_seed(seed)
                .with_stop(stop)
                .with_parallel_tool_calls(parallel)
        })
        .map_err(|_| StaticCodecError::InvalidShape)
}

fn present<'a>(source: &'a Map<String, Value>, field: &str) -> Option<&'a Value> {
    source.get(field).filter(|value| !value.is_null())
}

fn decode_stop(value: &Value, max_bytes: usize) -> Result<Vec<StopSequence>, StaticCodecError> {
    if let Some(text) = value.as_str() {
        return Ok(vec![
            StopSequence::new(text, max_bytes).map_err(StaticCodecError::from_validation)?,
        ]);
    }
    value
        .as_array()
        .ok_or(StaticCodecError::InvalidShape)?
        .iter()
        .map(|value| {
            let text = value.as_str().ok_or(StaticCodecError::InvalidShape)?;
            StopSequence::new(text, max_bytes).map_err(StaticCodecError::from_validation)
        })
        .collect()
}

/// Replaces every owned field, including removal. Source values may only preserve equivalent
/// number/string/null spelling; they can never restore an active value removed from the IR.
pub(super) fn encode_native(
    controls: &GenerationControls,
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
) {
    for (field, value) in [
        ("temperature", controls.temperature()),
        ("top_p", controls.top_p()),
        ("frequency_penalty", controls.frequency_penalty()),
        ("presence_penalty", controls.presence_penalty()),
    ] {
        let value = value.map(|value| {
            source
                .get(field)
                .filter(|original| original.as_f64().map(f64::to_bits) == Some(value.to_bits()))
                .cloned()
                .unwrap_or_else(|| json!(value))
        });
        replace(field, value, source, target);
    }
    for (field, value) in [
        ("top_k", controls.top_k().map(Value::from)),
        ("seed", controls.seed().map(Value::from)),
        ("n", controls.candidate_count().map(Value::from)),
    ] {
        replace(field, value, source, target);
    }
    let stop = controls.stop().map(|values| {
        if source.get("stop").is_some_and(Value::is_string) && values.len() == 1 {
            Value::String(values[0].as_str().to_owned())
        } else {
            Value::Array(
                values
                    .iter()
                    .map(|value| Value::String(value.as_str().to_owned()))
                    .collect(),
            )
        }
    });
    replace("stop", stop, source, target);
}

fn replace(
    field: &str,
    value: Option<Value>,
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = value {
        target.insert(field.to_owned(), value);
    } else if source.get(field) == Some(&Value::Null) {
        target.insert(field.to_owned(), Value::Null);
    } else {
        target.remove(field);
    }
}
