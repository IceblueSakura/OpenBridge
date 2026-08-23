//! Analyzes Chat `file` and Responses `input_file` parts without retaining sensitive values.

use serde_json::Value;
use url::Url;

use crate::{
    core::{ApiProtocol, FileDetail, FileInlineEncoding, FileMediaType},
    pipeline::{
        error::{GenerationCapabilityReason, RequestPlanningError},
        types::FileInputRequirements,
    },
    registry::FileInputSource,
};

use super::image_input::{canonical_base64_decoded_bytes, validate_remote_https_url};

/// Parses file parts only from protocol-defined input positions and freezes bounded facts.
pub(super) fn analyze_file_input(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<FileInputRequirements>, RequestPlanningError> {
    let mut requirements = FileInputRequirements::default();
    let result = match protocol {
        ApiProtocol::ChatCompletions => analyze_chat_files(object, &mut requirements),
        ApiProtocol::Responses => analyze_responses_files(object, &mut requirements),
    };
    let param = match protocol {
        ApiProtocol::ChatCompletions => "messages",
        ApiProtocol::Responses => "input",
    };
    result
        .map(|()| (requirements.part_count > 0).then_some(requirements))
        .map_err(|error| error.locate_multimodal(param, GenerationCapabilityReason::FileInput))
}

fn analyze_chat_files(
    object: &serde_json::Map<String, Value>,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return Ok(());
    };
    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let is_user = message.get("role").and_then(Value::as_str) == Some("user");
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("file") {
                continue;
            }
            if !is_user {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            ingest_chat_file(part, requirements)?;
        }
    }
    Ok(())
}

fn ingest_chat_file(
    part: &Value,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    let part = part
        .as_object()
        .filter(|value| {
            value
                .keys()
                .all(|key| matches!(key.as_str(), "type" | "file"))
        })
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let file = part
        .get("file")
        .and_then(Value::as_object)
        .filter(|value| {
            value
                .keys()
                .all(|key| matches!(key.as_str(), "filename" | "file_data" | "file_id"))
        })
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if file.contains_key("file_id") || !file.contains_key("file_data") {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let filename = required_filename(file.get("filename"), requirements)?;
    let data = file
        .get("file_data")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    ingest_inline(data, filename, requirements)
}

fn analyze_responses_files(
    object: &serde_json::Map<String, Value>,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    let Some(items) = object.get("input").and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        if item.get("type").and_then(Value::as_str) == Some("input_file") {
            ingest_responses_file(item, requirements)?;
            continue;
        }
        let Some(message) = item.as_object() else {
            continue;
        };
        let is_user = message.get("role").and_then(Value::as_str) == Some("user");
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("input_file") {
                continue;
            }
            if !is_user {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            ingest_responses_file(part, requirements)?;
        }
    }
    Ok(())
}

fn ingest_responses_file(
    part: &Value,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    let part = part
        .as_object()
        .filter(|value| {
            value.keys().all(|key| {
                matches!(
                    key.as_str(),
                    "type" | "filename" | "file_data" | "file_url" | "file_id" | "detail"
                )
            })
        })
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let source_count = ["file_data", "file_url", "file_id"]
        .iter()
        .filter(|field| part.contains_key(**field))
        .count();
    if source_count != 1 || part.contains_key("file_id") {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    if let Some(detail) = part.get("detail") {
        let detail = detail
            .as_str()
            .and_then(FileDetail::from_wire)
            .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
        requirements.details.insert(detail);
    }
    increment_part_count(requirements)?;
    if let Some(data) = part.get("file_data") {
        let filename = required_filename(part.get("filename"), requirements)?;
        let data = data
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
        return ingest_inline_without_count(data, filename, requirements);
    }
    let value = part
        .get("file_url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    validate_remote_https_url(value)?;
    let url = Url::parse(value).map_err(|_| RequestPlanningError::InvalidMultimodalInput)?;
    let media_type = FileMediaType::from_filename(url.path())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if let Some(filename) = part.get("filename") {
        let filename = required_filename(Some(filename), requirements)?;
        if FileMediaType::from_filename(filename) != Some(media_type) {
            return Err(RequestPlanningError::InvalidMultimodalInput);
        }
    }
    requirements.sources.insert(FileInputSource::RemoteUrl);
    requirements.media_types.insert(media_type);
    requirements.max_url_length = requirements.max_url_length.max(to_u32(value.len())?);
    Ok(())
}

fn ingest_inline(
    value: &str,
    filename: &str,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    increment_part_count(requirements)?;
    ingest_inline_without_count(value, filename, requirements)
}

fn ingest_inline_without_count(
    value: &str,
    filename: &str,
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    let filename_media = FileMediaType::from_filename(filename)
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let (encoding, media_type, payload) = if let Some(data) = value.strip_prefix("data:") {
        let (metadata, payload) = data
            .split_once(',')
            .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
        let media_type = metadata
            .strip_suffix(";base64")
            .and_then(FileMediaType::from_wire)
            .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
        (FileInlineEncoding::DataUrl, media_type, payload)
    } else {
        (FileInlineEncoding::RawBase64, filename_media, value)
    };
    if media_type != filename_media || payload.is_empty() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let encoded = to_u32(payload.len())?;
    let decoded = canonical_base64_decoded_bytes(payload)?;
    requirements.sources.insert(FileInputSource::InlineData);
    requirements.encodings.insert(encoding);
    requirements.media_types.insert(media_type);
    requirements.max_inline_encoded_bytes = requirements.max_inline_encoded_bytes.max(encoded);
    requirements.max_inline_decoded_bytes = requirements.max_inline_decoded_bytes.max(decoded);
    requirements.total_inline_encoded_bytes = requirements
        .total_inline_encoded_bytes
        .checked_add(encoded)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    requirements.total_inline_decoded_bytes = requirements
        .total_inline_decoded_bytes
        .checked_add(decoded)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

fn required_filename<'a>(
    value: Option<&'a Value>,
    requirements: &mut FileInputRequirements,
) -> Result<&'a str, RequestPlanningError> {
    let filename = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    requirements.max_filename_length = requirements
        .max_filename_length
        .max(to_u32(filename.len())?);
    Ok(filename)
}

fn increment_part_count(
    requirements: &mut FileInputRequirements,
) -> Result<(), RequestPlanningError> {
    requirements.part_count = requirements
        .part_count
        .checked_add(1)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

fn to_u32(value: usize) -> Result<u32, RequestPlanningError> {
    u32::try_from(value).map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)
}
