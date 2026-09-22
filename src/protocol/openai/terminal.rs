//! Typed terminal details, shared by static and event codecs.
use super::{CodecError, common::*};
use crate::semantic::task::generation::*;
use serde_json::{Map, Value, json};
pub(super) fn decode_details(o: &Map<String, Value>) -> Result<TerminalDetails, CodecError> {
    let error = o
        .get("error")
        .filter(|v| !v.is_null())
        .map(|v| {
            let e = object(v)?;
            fields(e, &["code", "message", "param"])?;
            Ok::<_, CodecError>(ResponseError {
                code: text(string(e, "code")?, "error code", 128)?,
                message: crate::semantic::value::Text::allowing_empty(
                    string(e, "message")?,
                    "error message",
                    MAX_TEXT_BYTES,
                )
                .map_err(|_| CodecError::Limit)?,
                param: e
                    .get("param")
                    .filter(|v| !v.is_null())
                    .map(|v| {
                        text(
                            v.as_str().ok_or(CodecError::Invalid("param"))?,
                            "param",
                            256,
                        )
                    })
                    .transpose()?,
            })
        })
        .transpose()?;
    let incomplete = o
        .get("incomplete_details")
        .filter(|v| !v.is_null())
        .map(|v| {
            let d = object(v)?;
            fields(d, &["reason"])?;
            Ok::<_, CodecError>(match string(d, "reason")? {
                "max_output_tokens" => IncompleteReason::MaxOutputTokens,
                "content_filter" => IncompleteReason::ContentFilter,
                s => IncompleteReason::Other(text(s, "incomplete reason", 128)?),
            })
        })
        .transpose()?;
    Ok(TerminalDetails { error, incomplete })
}
pub(super) fn encode_error(error: Option<&ResponseError>) -> Value {
    error
        .map(|e| {
            let mut v = json!({"code":e.code.as_str(),"message":e.message.as_str()});
            if let Some(p) = &e.param {
                v["param"] = json!(p.as_str());
            }
            v
        })
        .unwrap_or(Value::Null)
}
pub(super) fn encode_incomplete(reason: Option<&IncompleteReason>) -> Value {
    reason.map(|r|json!({"reason":match r {IncompleteReason::MaxOutputTokens=>"max_output_tokens",IncompleteReason::ContentFilter=>"content_filter",IncompleteReason::Other(t)=>t.as_str()}})).unwrap_or(Value::Null)
}
