//! Chat and Responses image-input requirement analysis.
//!
//! This module freezes source, media, detail, and size facts without retaining image bytes or
//! fetching remote URLs. It owns the inbound URL/local-address policy and canonical Base64 size
//! calculation used by generation preflight.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde_json::Value;
use url::{Host, Url};

use crate::{
    core::{ApiProtocol, ImageDetail, ImageInputSource, ImageMediaType},
    pipeline::{
        error::{GenerationCapabilityReason, RequestPlanningError},
        types::ImageInputRequirements,
    },
};

/// Parses image content parts only from their protocol-defined user-message positions.
pub(super) fn analyze_image_input(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    let result = match protocol {
        ApiProtocol::ChatCompletions => analyze_chat_images(object),
        ApiProtocol::Responses => analyze_responses_images(object),
    };
    let param = match protocol {
        ApiProtocol::ChatCompletions => "messages",
        ApiProtocol::Responses => "input",
    };
    result.map_err(|error| error.locate_multimodal(param, GenerationCapabilityReason::ImageInput))
}

/// Parses Chat `image_url` parts from user messages and freezes their source facts.
fn analyze_chat_images(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut requirements = ImageInputRequirements::default();
    for message in messages {
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("image_url") {
                continue;
            }

            // Image content is valid only in a standard user message.
            if message.get("role").and_then(Value::as_str) != Some("user") {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            let image = part
                .get("image_url")
                .and_then(Value::as_object)
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            let image_url = image
                .get("url")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;

            // Freeze the optional nested Chat detail and the URL/data-URL source.
            ingest_detail(image.get("detail"), &mut requirements)?;
            ingest_image_reference(image_url, &mut requirements)?;
        }
    }
    if requirements.part_count == 0 {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

/// Parses Responses `input_image` parts from user input messages and freezes their source facts.
fn analyze_responses_images(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<ImageInputRequirements>, RequestPlanningError> {
    let Some(items) = object.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut requirements = ImageInputRequirements::default();
    for item in items {
        // A content part cannot appear as a standalone Responses input item.
        if item.get("type").and_then(Value::as_str) == Some("input_image") {
            return Err(RequestPlanningError::InvalidMultimodalInput);
        }
        let Some(parts) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("input_image") {
                continue;
            }

            // Current Native image input accepts only a standard user message content part.
            if item.get("role").and_then(Value::as_str) != Some("user") {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            let image_url = part.get("image_url").filter(|value| !value.is_null());
            let file_id = part.get("file_id").filter(|value| !value.is_null());
            if usize::from(image_url.is_some()) + usize::from(file_id.is_some()) != 1 {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }

            // Freeze detail and exactly one standard Responses image source.
            ingest_detail(part.get("detail"), &mut requirements)?;
            if let Some(image_url) = image_url {
                let image_url = image_url
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
                ingest_image_reference(image_url, &mut requirements)?;
            } else {
                let _file_id = file_id
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
                increment_part_count(&mut requirements)?;
                requirements.sources.insert(ImageInputSource::FileId);
            }
        }
    }
    if requirements.part_count == 0 {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

/// Parses an optional explicit image-detail value.
fn ingest_detail(
    value: Option<&Value>,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    let Some(value) = value else {
        return Ok(());
    };
    let detail = value
        .as_str()
        .and_then(ImageDetail::from_wire)
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    requirements.details.insert(detail);
    Ok(())
}

/// Classifies and validates one remote URL or inline image data URL.
fn ingest_image_reference(
    value: &str,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    increment_part_count(requirements)?;
    if value.starts_with("data:") {
        return ingest_data_url(value, requirements);
    }

    // Validate only the inbound URL syntax; the Provider still owns DNS, redirects, and download limits.
    validate_remote_https_url(value)?;
    let length = u32::try_from(value.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
    requirements.sources.insert(ImageInputSource::RemoteUrl);
    requirements.max_url_length = requirements.max_url_length.max(length);
    Ok(())
}

/// Validates one canonical Base64 data URL and records encoded and decoded byte counts.
fn ingest_data_url(
    value: &str,
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    // Split the exact data-URL media declaration from its non-empty Base64 payload.
    let (metadata, payload) = value
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let media_type = metadata
        .strip_suffix(";base64")
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if payload.is_empty() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let encoded_bytes = u32::try_from(payload.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;

    // Validate canonical standard Base64 and derive its exact decoded size without allocating media bytes.
    let decoded_bytes = canonical_base64_decoded_bytes(payload)?;

    // Accumulate bounded request facts without retaining media bytes or the original URL.
    requirements.sources.insert(ImageInputSource::DataUrl);
    match ImageMediaType::from_wire(media_type) {
        Some(media_type) => {
            requirements.media_types.insert(media_type);
        }
        None => requirements.unsupported_media_type = true,
    }
    requirements.max_inline_encoded_bytes =
        requirements.max_inline_encoded_bytes.max(encoded_bytes);
    requirements.max_inline_decoded_bytes =
        requirements.max_inline_decoded_bytes.max(decoded_bytes);
    requirements.total_inline_encoded_bytes = requirements
        .total_inline_encoded_bytes
        .checked_add(encoded_bytes)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    requirements.total_inline_decoded_bytes = requirements
        .total_inline_decoded_bytes
        .checked_add(decoded_bytes)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

/// Validates canonical padded or unpadded standard Base64 and returns its exact decoded byte count.
pub(super) fn canonical_base64_decoded_bytes(payload: &str) -> Result<u32, RequestPlanningError> {
    let bytes = payload.as_bytes();
    let remainder = bytes.len() % 4;
    if remainder == 1 {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Locate at most two terminal padding bytes and reject padding in the encoded body.
    let padding = if bytes.ends_with(b"==") {
        2
    } else if bytes.ends_with(b"=") {
        1
    } else {
        0
    };
    let content_length = bytes.len() - padding;
    if content_length == 0
        || (padding > 0 && remainder != 0)
        || bytes[..content_length]
            .iter()
            .any(|value| base64_sextet(*value).is_none())
        || bytes[content_length..].iter().any(|value| *value != b'=')
    {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Require zero unused bits so alternate encodings of the same bytes cannot pass as canonical.
    let final_sextet = base64_sextet(bytes[content_length - 1])
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if ((padding == 2 || (padding == 0 && remainder == 2)) && final_sextet & 0b1111 != 0)
        || ((padding == 1 || (padding == 0 && remainder == 3)) && final_sextet & 0b11 != 0)
    {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }

    // Compute the decoded size only after the alphabet, padding, and unused-bit checks succeed.
    let decoded = bytes
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_add(remainder.saturating_sub(1)))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    u32::try_from(decoded).map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)
}

/// Returns the six-bit value for one standard Base64 alphabet byte.
const fn base64_sextet(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Increments the total image-part count with checked arithmetic.
fn increment_part_count(
    requirements: &mut ImageInputRequirements,
) -> Result<(), RequestPlanningError> {
    requirements.part_count = requirements
        .part_count
        .checked_add(1)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    Ok(())
}

/// Applies the inbound absolute-HTTPS and local-address policy without fetching the URL.
pub(super) fn validate_remote_https_url(value: &str) -> Result<(), RequestPlanningError> {
    let url = Url::parse(value).map_err(|_| RequestPlanningError::InvalidMultimodalInput)?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let host = url
        .host()
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let blocked = match host {
        Host::Domain(domain) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
        }
        Host::Ipv4(address) => !is_public_ipv4(address),
        Host::Ipv6(address) => !is_public_ipv6(address),
    };
    if blocked {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    Ok(())
}

/// Returns whether an IPv4 literal is outside local, reserved, documentation, and multicast ranges.
fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_multicast()
        || first == 0
        || (first == 100 && (64..=127).contains(&second))
        || (first == 198 && (18..=19).contains(&second))
        || first >= 240)
}

/// Returns whether an IPv6 literal is outside local, reserved, documentation, and multicast ranges.
fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::canonical_base64_decoded_bytes;

    #[test]
    fn canonical_base64_counts_padded_and_unpadded_payloads() {
        for (payload, decoded_bytes) in
            [("Zg==", 1), ("Zg", 1), ("Zm8=", 2), ("Zm8", 2), ("Zm9v", 3)]
        {
            assert_eq!(
                canonical_base64_decoded_bytes(payload).unwrap(),
                decoded_bytes
            );
        }
    }

    #[test]
    fn canonical_base64_rejects_invalid_length_padding_and_unused_bits() {
        for payload in ["A", "Zh", "Zm9", "Zg=", "Z==="] {
            assert!(
                canonical_base64_decoded_bytes(payload).is_err(),
                "{payload}"
            );
        }
    }
}
