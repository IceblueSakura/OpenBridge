//! Pure HTTP/SSE boundary for the stateless Responses profile.
//! Socket I/O, authentication, cancellation and commit remain caller responsibilities.
//! One consume call yields at most one framed payload; it never buffers a whole wire stream.
use super::{
    CodecError, DecodedResponse, Profile, ResponseMetadata,
    events::{EventDecoder, EventEncoder},
};
use crate::{
    lowering::generation::GenerationRepresentationContract,
    protocol::fidelity::FidelityRecords,
    semantic::{
        task::generation::*,
        value::{ReplayOrigin, json_size},
    },
    transport::sse::{SseDecodeError, SseDecoder, SseEvent},
};
use bytes::Bytes;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug)]
pub struct SseLimits {
    pub max_event_bytes: usize,
    pub max_wire_bytes: usize,
    pub max_events: usize,
    /// Aggregate decoded UTF-8 padding bytes; independent of wire and semantic budgets.
    /// Zero forbids nonempty padding without disabling unpadded streams.
    pub max_obfuscation_bytes: usize,
}
impl Default for SseLimits {
    fn default() -> Self {
        Self {
            max_event_bytes: MAX_TOTAL_BYTES + 256,
            max_wire_bytes: 64 << 20,
            max_events: 1_000_000,
            max_obfuscation_bytes: 4 << 20,
        }
    }
}
impl SseLimits {
    fn validate(self) -> Result<(), SseError> {
        if self.max_event_bytes == 0 || self.max_wire_bytes == 0 || self.max_events == 0 {
            return Err(SseError::Limit);
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum SseError {
    #[error("invalid Responses HTTP status or content type")]
    Http,
    #[error("Responses SSE budget exceeded")]
    Limit,
    #[error("SSE event and JSON type disagree")]
    EventType,
    #[error("SSE decoder is closed or rejected")]
    Closed,
    #[error(transparent)]
    Framing(#[from] SseDecodeError),
    #[error(transparent)]
    Codec(#[from] CodecError),
}
pub struct ResponsesSseDecoder {
    framing: SseDecoder,
    codec: EventDecoder,
    limits: SseLimits,
    wire_bytes: usize,
    events: usize,
    obfuscation_bytes: usize,
    closed: bool,
    rejected: bool,
}
impl ResponsesSseDecoder {
    pub fn new(
        status: u16,
        content_type: &str,
        limits: SseLimits,
        origin: Option<ReplayOrigin>,
    ) -> Result<Self, SseError> {
        limits.validate()?;
        let media: mime::Mime = content_type.parse().map_err(|_| SseError::Http)?;
        if status != 200
            || media.type_() != mime::TEXT
            || media.subtype() != "event-stream"
            || media
                .params()
                .any(|(k, v)| k == mime::CHARSET && v != mime::UTF_8)
        {
            return Err(SseError::Http);
        }
        let mut codec = EventDecoder::new(Profile::Responses);
        if let Some(origin) = origin {
            codec = codec.with_replay_origin(origin);
        }
        Ok(Self {
            framing: SseDecoder::new(limits.max_event_bytes),
            codec,
            limits,
            wire_bytes: 0,
            events: 0,
            obfuscation_bytes: 0,
            closed: false,
            rejected: false,
        })
    }
    /// The caller retains and revisits unconsumed bytes, respecting downstream backpressure.
    pub fn consume(&mut self, chunk: &[u8]) -> Result<(usize, Vec<StreamEvent>), SseError> {
        if self.closed || self.rejected {
            return Err(SseError::Closed);
        }
        let result = (|| {
            let remaining = self.limits.max_wire_bytes.saturating_sub(self.wire_bytes);
            let available = chunk.len().min(remaining.saturating_add(1));
            let (event, n) = self.framing.push_until_event(&chunk[..available])?;
            self.wire_bytes = self.wire_bytes.checked_add(n).ok_or(SseError::Limit)?;
            if self.wire_bytes > self.limits.max_wire_bytes {
                return Err(SseError::Limit);
            }
            let events = if let Some(event) = event {
                self.frame(event)?
            } else {
                vec![]
            };
            Ok((n, events))
        })();
        if result.is_err() {
            self.rejected = true;
        }
        result
    }
    fn frame(&mut self, frame: SseEvent) -> Result<Vec<StreamEvent>, SseError> {
        self.events += 1;
        if self.events > self.limits.max_events {
            return Err(SseError::Limit);
        }
        if frame.data().is_empty() {
            return Ok(vec![]);
        }
        let mut payload: Value =
            serde_json::from_str(frame.data()).map_err(|_| CodecError::Invalid("SSE JSON"))?;
        let object = payload
            .as_object_mut()
            .ok_or(CodecError::Invalid("SSE payload"))?;
        let declared = frame.event().filter(|s| !s.is_empty());
        match (declared, object.get("type")) {
            (Some(header), Some(Value::String(body))) if header == body => {}
            (Some(header), None) => {
                object.insert("type".into(), json!(header));
            }
            (None, Some(Value::String(_))) => {}
            _ => return Err(SseError::EventType),
        }
        if let Some(padding) = payload.get("obfuscation") {
            let bytes = padding
                .as_str()
                .ok_or(CodecError::Invalid("obfuscation"))?
                .len();
            charge_padding(
                &mut self.obfuscation_bytes,
                bytes,
                self.limits.max_obfuscation_bytes,
            )?;
        }
        super::envelope::validate_stream_payload(&payload)?;
        Ok(self.codec.push(&payload)?)
    }
    pub fn metadata(&self) -> Option<&ResponseMetadata> {
        self.codec.metadata()
    }
    pub fn fidelity(&self) -> &FidelityRecords {
        self.codec.fidelity()
    }
    /// A clean socket EOF is successful only after both framing and the typed terminal validate.
    pub fn finish(&mut self) -> Result<(), SseError> {
        if self.closed || self.rejected {
            return Err(SseError::Closed);
        }
        self.closed = true;
        let result = (|| {
            self.framing.finish_strict()?;
            self.codec.finish()?;
            Ok(())
        })();
        if result.is_err() {
            self.rejected = true;
        }
        result
    }
    pub fn materialize(&self) -> Result<DecodedResponse, SseError> {
        if !self.closed || self.rejected {
            return Err(SseError::Closed);
        }
        Ok(self.codec.materialize()?)
    }
}
/// Supply an unpredictable per-response seed from the trusted I/O boundary in non-test use.
/// The codec itself uses no random source, clock, filesystem, credential or network client.
pub enum Obfuscation {
    Disabled,
    Seeded([u8; 32]),
}
pub struct ResponsesSseEncoder {
    codec: EventEncoder,
    limits: SseLimits,
    wire_bytes: usize,
    events: usize,
    obfuscation_bytes: usize,
    padding: Obfuscation,
    rejected: bool,
}
impl ResponsesSseEncoder {
    pub fn new(
        metadata: ResponseMetadata,
        contract: GenerationRepresentationContract,
        limits: SseLimits,
        padding: Obfuscation,
    ) -> Result<Self, SseError> {
        limits.validate()?;
        metadata.context.validate_complete()?;
        Ok(Self {
            codec: EventEncoder::new(Profile::Responses, metadata)?.with_contract(contract),
            limits,
            wire_bytes: 0,
            events: 0,
            obfuscation_bytes: 0,
            padding,
            rejected: false,
        })
    }
    pub fn update_metadata(&mut self, metadata: ResponseMetadata) -> Result<(), SseError> {
        if self.rejected {
            return Err(SseError::Closed);
        }
        let result = self.codec.update_metadata(metadata);
        if result.is_err() {
            self.rejected = true;
        }
        Ok(result?)
    }
    pub fn encode(
        &mut self,
        event: &StreamEvent,
        fidelity: &FidelityRecords,
    ) -> Result<Vec<Bytes>, SseError> {
        if self.rejected {
            return Err(SseError::Closed);
        }
        let result = (|| {
            let values = self.codec.encode(event, fidelity)?;
            let mut frames = vec![];
            for mut value in values {
                self.events += 1;
                if self.events > self.limits.max_events {
                    return Err(SseError::Limit);
                }
                let typ = value["type"]
                    .as_str()
                    .ok_or(CodecError::Invalid("event type"))?
                    .to_owned();
                if typ.ends_with(".delta")
                    && let Obfuscation::Seeded(seed) = &self.padding
                {
                    value["obfuscation"] = json!("");
                    let size = json_size(&value, MAX_TOTAL_BYTES).map_err(|_| SseError::Limit)?;
                    let limit = self
                        .limits
                        .max_event_bytes
                        .saturating_sub(typ.len() + 16)
                        .min(MAX_TOTAL_BYTES);
                    if size > limit {
                        return Err(SseError::Limit);
                    }
                    let padded = size.saturating_add(1023) / 1024 * 1024;
                    let n = padded.min(limit) - size;
                    // Charge before allocating; exhaustion rejects rather than silently removing
                    // requested obfuscation or emitting a successful terminal after a partial error.
                    charge_padding(
                        &mut self.obfuscation_bytes,
                        n,
                        self.limits.max_obfuscation_bytes,
                    )?;
                    let mut pad = String::with_capacity(n);
                    let mut block = 0u64;
                    while pad.len() < n {
                        let mut h = Sha256::new();
                        h.update(seed);
                        h.update((self.events as u64).to_le_bytes());
                        h.update(block.to_le_bytes());
                        for b in h.finalize() {
                            use std::fmt::Write;
                            write!(&mut pad, "{b:02x}").expect("String formatting");
                        }
                        block += 1;
                    }
                    pad.truncate(n);
                    value["obfuscation"] = json!(pad);
                }
                super::envelope::validate_stream_payload(&value)?;
                let frame = encode_frame(&value, self.limits.max_event_bytes)?;
                self.wire_bytes = self
                    .wire_bytes
                    .checked_add(frame.len())
                    .ok_or(SseError::Limit)?;
                if self.wire_bytes > self.limits.max_wire_bytes {
                    return Err(SseError::Limit);
                }
                frames.push(frame);
            }
            Ok(frames)
        })();
        if result.is_err() {
            self.rejected = true;
        }
        result
    }
    pub fn finish(&self) -> Result<(), SseError> {
        if self.rejected {
            return Err(SseError::Closed);
        }
        Ok(self.codec.finish()?)
    }
}
fn charge_padding(total: &mut usize, bytes: usize, maximum: usize) -> Result<(), SseError> {
    let next = total.checked_add(bytes).ok_or(SseError::Limit)?;
    if next > maximum {
        return Err(SseError::Limit);
    }
    *total = next;
    Ok(())
}

/// Encode exactly one payload with a matching event name and a complete LF delimiter.
pub fn encode_frame(payload: &Value, max_bytes: usize) -> Result<Bytes, SseError> {
    let typ = payload
        .get("type")
        .and_then(Value::as_str)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
        })
        .ok_or(SseError::EventType)?;
    let prefix = format!("event: {typ}\ndata: ");
    let size = json_size(payload, max_bytes).map_err(|_| SseError::Limit)?;
    if prefix.len().saturating_add(size).saturating_add(2) > max_bytes {
        return Err(SseError::Limit);
    }
    let mut wire = Vec::with_capacity(prefix.len() + size + 2);
    wire.extend_from_slice(prefix.as_bytes());
    serde_json::to_writer(&mut wire, payload).map_err(|_| CodecError::Invalid("event JSON"))?;
    wire.extend_from_slice(b"\n\n");
    Ok(Bytes::from(wire))
}
