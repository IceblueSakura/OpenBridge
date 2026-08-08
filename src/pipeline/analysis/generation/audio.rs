//! Chat Native audio request analysis.
//!
//! This module records only bounded source, format, role, and size facts. It never retains audio
//! bytes, fetches URLs, decodes media, or selects a Provider Route.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde_json::Value;
use url::{Host, Url};

use crate::core::{ApiProtocol, AudioFormat, AudioInputSource};

use super::super::super::{
    error::RequestPlanningError,
    types::{AudioInputRequirements, AudioOutputRequirements},
};

/// Bounded audio facts returned by the Chat audio analyzer.
type AudioAnalysis = Result<
    (
        Option<AudioInputRequirements>,
        Option<AudioInputRequirements>,
        Option<AudioOutputRequirements>,
    ),
    RequestPlanningError,
>;

/// Extracts Chat audio input, voice-conditioning, and generated-audio facts.
pub(super) fn analyze_audio(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> AudioAnalysis {
    if protocol == ApiProtocol::Responses {
        return Ok((None, None, None));
    }

    // Parse user input_audio parts and preserve only bounded source/format facts.
    let input = analyze_chat_audio_input(object)?;

    // Parse top-level Chat audio output controls and any reference voice condition.
    let (output, voice_conditioning) = analyze_chat_audio_output(object)?;
    Ok((input, voice_conditioning, output))
}

/// Parses `input_audio` parts from Chat user messages.
fn analyze_chat_audio_input(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<AudioInputRequirements>, RequestPlanningError> {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut requirements = AudioInputRequirements::default();
    for message in messages {
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            if message
                .get("content")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
            {
                requirements.text_part_count = requirements
                    .text_part_count
                    .checked_add(1)
                    .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
            }
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("input_audio") {
                if part.get("type").and_then(Value::as_str) == Some("text")
                    && part
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
                {
                    requirements.text_part_count = requirements
                        .text_part_count
                        .checked_add(1)
                        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
                }
                continue;
            }
            if message.get("role").and_then(Value::as_str) != Some("user") {
                return Err(RequestPlanningError::InvalidMultimodalInput);
            }
            let input_audio = part
                .get("input_audio")
                .and_then(Value::as_object)
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            let data = input_audio
                .get("data")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            let format = input_audio
                .get("format")
                .and_then(Value::as_str)
                .and_then(AudioFormat::from_wire)
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            ingest_audio_reference(data, Some(format), &mut requirements)?;
        }
    }
    if requirements.part_count == 0 {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

/// Parses the Chat top-level `modalities` and `audio` output controls.
fn analyze_chat_audio_output(
    object: &serde_json::Map<String, Value>,
) -> Result<
    (
        Option<AudioOutputRequirements>,
        Option<AudioInputRequirements>,
    ),
    RequestPlanningError,
> {
    let requests_audio_modality = object
        .get("modalities")
        .and_then(Value::as_array)
        .is_some_and(|modalities| {
            modalities
                .iter()
                .any(|value| value.as_str() == Some("audio"))
        });
    let Some(audio) = object.get("audio").filter(|value| !value.is_null()) else {
        if requests_audio_modality {
            return Err(RequestPlanningError::InvalidMultimodalInput);
        }
        return Ok((None, None));
    };
    if !requests_audio_modality {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let audio = audio
        .as_object()
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let format = audio
        .get("format")
        .and_then(Value::as_str)
        .and_then(AudioFormat::from_wire)
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let mut voice_conditioning = None;
    let voice = match audio.get("voice").filter(|value| !value.is_null()) {
        None => None,
        Some(Value::String(value)) if value.starts_with("data:") => {
            let mut requirements = AudioInputRequirements::default();
            ingest_audio_reference(value, None, &mut requirements)?;
            voice_conditioning = Some(requirements);
            None
        }
        Some(Value::String(value)) if !value.is_empty() => Some(value.to_owned()),
        Some(Value::Object(value)) => {
            let data = value
                .get("data")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
            let format = value
                .get("format")
                .and_then(Value::as_str)
                .and_then(AudioFormat::from_wire);
            let mut requirements = AudioInputRequirements::default();
            ingest_audio_reference(data, format, &mut requirements)?;
            voice_conditioning = Some(requirements);
            None
        }
        Some(_) => return Err(RequestPlanningError::InvalidMultimodalInput),
    };
    Ok((
        Some(AudioOutputRequirements {
            format,
            voice,
            voice_description: has_user_text(object),
            assistant_text_count: count_assistant_text(object),
        }),
        voice_conditioning,
    ))
}

/// Counts non-empty assistant target-text messages without retaining their content.
fn count_assistant_text(object: &serde_json::Map<String, Value>) -> u32 {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return 0;
    };
    messages
        .iter()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .map(|message| match message.get("content") {
            Some(Value::String(value)) => u32::from(!value.is_empty()),
            Some(Value::Array(parts)) => u32::from(parts.iter().any(|part| {
                part.get("type").and_then(Value::as_str) == Some("text")
                    && part
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
            })),
            _ => 0,
        })
        .sum()
}

/// Records one remote URL, data URL, or pure Base64 audio source.
fn ingest_audio_reference(
    value: &str,
    declared_format: Option<AudioFormat>,
    requirements: &mut AudioInputRequirements,
) -> Result<(), RequestPlanningError> {
    requirements.part_count = requirements
        .part_count
        .checked_add(1)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    if value.starts_with("data:") {
        return ingest_data_url(value, declared_format, requirements);
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        validate_remote_audio_url(value)?;
        let length = u32::try_from(value.len())
            .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
        requirements.sources.insert(AudioInputSource::RemoteUrl);
        requirements.max_url_length = requirements.max_url_length.max(length);
        if let Some(format) = declared_format {
            requirements.formats.insert(format);
        }
        return Ok(());
    }
    let format = declared_format.ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let encoded = u32::try_from(value.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
    let decoded = canonical_base64_decoded_bytes(value)?;
    requirements.sources.insert(AudioInputSource::Base64);
    requirements.formats.insert(format);
    record_inline_size(requirements, encoded, decoded)
}

/// Validates one canonical Base64 data URL and records its inferred media format.
fn ingest_data_url(
    value: &str,
    declared_format: Option<AudioFormat>,
    requirements: &mut AudioInputRequirements,
) -> Result<(), RequestPlanningError> {
    let (metadata, payload) = value
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let media_type = metadata
        .strip_suffix(";base64")
        .filter(|value| !value.is_empty())
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let inferred = match media_type {
        "audio/wav" | "audio/x-wav" => AudioFormat::Wav,
        "audio/mpeg" | "audio/mp3" => AudioFormat::Mp3,
        "audio/flac" => AudioFormat::Flac,
        "audio/mp4" | "audio/m4a" => AudioFormat::M4a,
        "audio/ogg" => AudioFormat::Ogg,
        _ => return Err(RequestPlanningError::InvalidMultimodalInput),
    };
    if declared_format.is_some_and(|format| format != inferred) {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    if payload.is_empty() {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let encoded = u32::try_from(payload.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
    let decoded = canonical_base64_decoded_bytes(payload)?;
    requirements.sources.insert(AudioInputSource::DataUrl);
    requirements
        .formats
        .insert(declared_format.unwrap_or(inferred));
    record_inline_size(requirements, encoded, decoded)
}

/// Accumulates checked inline encoded and decoded sizes without retaining the payload.
fn record_inline_size(
    requirements: &mut AudioInputRequirements,
    encoded: u32,
    decoded: u32,
) -> Result<(), RequestPlanningError> {
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

/// Validates canonical padded Base64 and returns the exact decoded byte count.
fn canonical_base64_decoded_bytes(payload: &str) -> Result<u32, RequestPlanningError> {
    let bytes = payload.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let padding = if bytes.ends_with(b"==") {
        2
    } else if bytes.ends_with(b"=") {
        1
    } else {
        0
    };
    let content_length = bytes.len() - padding;
    if content_length == 0
        || bytes[..content_length]
            .iter()
            .any(|value| base64_sextet(*value).is_none())
        || bytes[content_length..].iter().any(|value| *value != b'=')
    {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let final_sextet = base64_sextet(bytes[content_length - 1])
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    if (padding == 2 && final_sextet & 0b1111 != 0) || (padding == 1 && final_sextet & 0b11 != 0) {
        return Err(RequestPlanningError::InvalidMultimodalInput);
    }
    let decoded = bytes
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
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

/// Applies the same absolute-HTTPS and local-address rejection policy used for image URLs.
fn validate_remote_audio_url(value: &str) -> Result<(), RequestPlanningError> {
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

/// Returns whether one IPv4 literal is outside local, reserved, and multicast ranges.
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

/// Returns whether one IPv6 literal is outside local, reserved, and multicast ranges.
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

/// Detects whether a user message supplies any non-empty text for voice style/design semantics.
fn has_user_text(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message.get("role").and_then(Value::as_str) == Some("user")
                    && match message.get("content") {
                        Some(Value::String(value)) => !value.is_empty(),
                        Some(Value::Array(parts)) => parts.iter().any(|part| {
                            part.get("type").and_then(Value::as_str) == Some("text")
                                && part
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| !text.is_empty())
                        }),
                        _ => false,
                    }
            })
        })
}
