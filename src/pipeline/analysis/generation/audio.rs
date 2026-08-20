//! Chat Native audio request analysis.
//!
//! This module records only bounded source, format, role, and size facts. It never retains audio
//! bytes, fetches URLs, decodes media, or selects a Provider Route.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde_json::Value;
use url::{Host, Url};

use crate::core::{ApiProtocol, AsrLanguage, AudioFormat, AudioInputSource};

use super::super::super::{
    error::RequestPlanningError,
    types::{
        AudioInputRequirements, GeneratedAudioMessageShape, InputAudioMessageShape,
        RequestedAsrLanguage, RequestedAsrOptions, RequestedAudio, RequestedAudioDelivery,
        RequestedVoice,
    },
};

/// Extracts one closed Chat audio request shape without assigning a business task.
pub(super) fn analyze_audio(
    protocol: ApiProtocol,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<RequestedAudio>, RequestPlanningError> {
    if protocol == ApiProtocol::Responses {
        return Ok(None);
    }

    // Parse resource, control, delivery, and message shapes without consulting a Model or Route.
    let input = analyze_chat_audio_input(object)?;
    let generated = analyze_chat_audio_output(object)?;
    let asr_options = analyze_asr_options(object)?;
    let input_message_shape = classify_input_audio_message_shape(object);
    let generated_message_shape = classify_generated_audio_message_shape(object);

    // Reject mutually exclusive wire families before preflight interprets their task semantics.
    match (input, generated, asr_options) {
        (Some(resources), None, asr_options) => Ok(Some(RequestedAudio::Input {
            resources,
            message_shape: input_message_shape,
            asr_options,
        })),
        (None, None, RequestedAsrOptions::Present { language }) => {
            Ok(Some(RequestedAudio::Input {
                resources: AudioInputRequirements::default(),
                message_shape: input_message_shape,
                asr_options: RequestedAsrOptions::Present { language },
            }))
        }
        (None, None, RequestedAsrOptions::Absent) => Ok(None),
        (None, Some((delivery, voice)), RequestedAsrOptions::Absent) => {
            Ok(Some(RequestedAudio::Generated {
                delivery,
                message_shape: generated_message_shape,
                voice,
            }))
        }
        (Some(_), Some(_), _) | (None, Some(_), RequestedAsrOptions::Present { .. }) => {
            Err(RequestPlanningError::InvalidMultimodalInput)
        }
    }
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
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("input_audio") {
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
) -> Result<Option<(RequestedAudioDelivery, RequestedVoice)>, RequestPlanningError> {
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
        return Ok(None);
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
    let voice = match audio.get("voice").filter(|value| !value.is_null()) {
        None => RequestedVoice::Unspecified,
        Some(Value::String(value)) if value.starts_with("data:") => {
            let mut requirements = AudioInputRequirements::default();
            ingest_audio_reference(value, None, &mut requirements)?;
            RequestedVoice::ReferenceVoice(requirements)
        }
        Some(Value::String(value)) if !value.is_empty() => RequestedVoice::Preset(value.to_owned()),
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
            RequestedVoice::ReferenceVoice(requirements)
        }
        Some(_) => return Err(RequestPlanningError::InvalidMultimodalInput),
    };
    Ok(Some((RequestedAudioDelivery { format }, voice)))
}

/// Parses the optional ASR control without deciding whether the selected model is ASR.
fn analyze_asr_options(
    object: &serde_json::Map<String, Value>,
) -> Result<RequestedAsrOptions, RequestPlanningError> {
    let Some(options) = object.get("asr_options").filter(|value| !value.is_null()) else {
        return Ok(RequestedAsrOptions::Absent);
    };
    let options = options
        .as_object()
        .ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let language = match options.get("language").filter(|value| !value.is_null()) {
        None => None,
        Some(Value::String(value)) => Some(match value.as_str() {
            "auto" => RequestedAsrLanguage::Known(AsrLanguage::Auto),
            "zh" => RequestedAsrLanguage::Known(AsrLanguage::Zh),
            "en" => RequestedAsrLanguage::Known(AsrLanguage::En),
            _ => RequestedAsrLanguage::Unsupported,
        }),
        Some(_) => return Err(RequestPlanningError::InvalidMultimodalInput),
    };
    Ok(RequestedAsrOptions::Present { language })
}

/// Classifies whether the complete envelope is exactly one user audio-only message.
fn classify_input_audio_message_shape(
    object: &serde_json::Map<String, Value>,
) -> InputAudioMessageShape {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return InputAudioMessageShape::GeneralConversation;
    };
    let [message] = messages.as_slice() else {
        return InputAudioMessageShape::GeneralConversation;
    };
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return InputAudioMessageShape::GeneralConversation;
    }
    let Some(parts) = message.get("content").and_then(Value::as_array) else {
        return InputAudioMessageShape::GeneralConversation;
    };
    let [part] = parts.as_slice() else {
        return InputAudioMessageShape::GeneralConversation;
    };
    if part.get("type").and_then(Value::as_str) == Some("input_audio") {
        InputAudioMessageShape::SingleUserAudioOnly
    } else {
        InputAudioMessageShape::GeneralConversation
    }
}

/// Classifies the complete envelope into the generated-audio text arrangements supported today.
fn classify_generated_audio_message_shape(
    object: &serde_json::Map<String, Value>,
) -> GeneratedAudioMessageShape {
    let Some(messages) = object.get("messages").and_then(Value::as_array) else {
        return GeneratedAudioMessageShape::Other;
    };
    match messages.as_slice() {
        [assistant] if is_text_only_message(assistant, "assistant") => {
            GeneratedAudioMessageShape::AssistantTextOnly
        }
        [user, assistant]
            if is_text_only_message(user, "user")
                && is_text_only_message(assistant, "assistant") =>
        {
            GeneratedAudioMessageShape::UserTextThenAssistantText
        }
        _ => GeneratedAudioMessageShape::Other,
    }
}

/// Returns whether one message has the expected role and contains only non-empty text.
fn is_text_only_message(message: &Value, expected_role: &str) -> bool {
    if message.get("role").and_then(Value::as_str) != Some(expected_role) {
        return false;
    }
    match message.get("content") {
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(parts)) => {
            !parts.is_empty()
                && parts.iter().all(|part| {
                    part.get("type").and_then(Value::as_str) == Some("text")
                        && part
                            .get("text")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.is_empty())
                })
        }
        _ => false,
    }
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
        let source = requirements
            .sources
            .entry(AudioInputSource::RemoteUrl)
            .or_default();
        source.max_url_length = source.max_url_length.max(length);
        if let Some(format) = declared_format {
            source.formats.insert(format);
        }
        return Ok(());
    }
    let format = declared_format.ok_or(RequestPlanningError::InvalidMultimodalInput)?;
    let encoded = u32::try_from(value.len())
        .map_err(|_| RequestPlanningError::MultimodalInputLimitExceeded)?;
    let decoded = canonical_base64_decoded_bytes(value)?;
    record_inline_size(
        requirements,
        AudioInputSource::Base64,
        format,
        encoded,
        decoded,
    )
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
    let format = declared_format.unwrap_or(inferred);
    record_inline_size(
        requirements,
        AudioInputSource::DataUrl,
        format,
        encoded,
        decoded,
    )
}

/// Accumulates checked inline encoded and decoded sizes without retaining the payload.
fn record_inline_size(
    requirements: &mut AudioInputRequirements,
    source: AudioInputSource,
    format: AudioFormat,
    encoded: u32,
    decoded: u32,
) -> Result<(), RequestPlanningError> {
    let source_requirements = requirements.sources.entry(source).or_default();
    source_requirements.formats.insert(format);
    source_requirements.max_inline_encoded_bytes =
        source_requirements.max_inline_encoded_bytes.max(encoded);
    source_requirements.max_inline_decoded_bytes =
        source_requirements.max_inline_decoded_bytes.max(decoded);
    source_requirements.total_inline_encoded_bytes = source_requirements
        .total_inline_encoded_bytes
        .checked_add(encoded)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
    source_requirements.total_inline_decoded_bytes = source_requirements
        .total_inline_decoded_bytes
        .checked_add(decoded)
        .ok_or(RequestPlanningError::MultimodalInputLimitExceeded)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_inline_sources_share_one_cumulative_budget() {
        let mut requirements = AudioInputRequirements::default();
        record_inline_size(
            &mut requirements,
            AudioInputSource::DataUrl,
            AudioFormat::Wav,
            6,
            4,
        )
        .unwrap();
        record_inline_size(
            &mut requirements,
            AudioInputSource::Base64,
            AudioFormat::Wav,
            6,
            4,
        )
        .unwrap();

        assert_eq!(requirements.total_inline_encoded_bytes, 12);
        assert_eq!(requirements.total_inline_decoded_bytes, 8);
        assert_eq!(
            requirements.sources[&AudioInputSource::DataUrl].total_inline_encoded_bytes,
            6
        );
        assert_eq!(
            requirements.sources[&AudioInputSource::Base64].total_inline_encoded_bytes,
            6
        );
    }
}
