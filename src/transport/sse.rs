//! Incremental SSE framing decoder.
//!
//! Network chunks are not UTF-8, line, or SSE-event boundaries. This decoder only organizes the
//! byte stream into complete `SseEvent` values with CRLF, comments, multiline `data:`, and event
//! size-limit support. `GenerationProviderAdapter::classify_sse_event` determines event semantics.

use bytes::BytesMut;
use thiserror::Error;

/// SSE framing or single-event size validation failed.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum SseDecodeError {
    /// The current event's accumulated bytes exceed the configured limit.
    #[error("SSE event exceeds the configured size limit")]
    EventTooLarge,
    /// An SSE field is not valid UTF-8.
    #[error("SSE field is not valid UTF-8")]
    InvalidUtf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Fully framed SSE event.
pub struct SseEvent {
    event: Option<String>,
    data: String,
    id: Option<String>,
    retry_ms: Option<u64>,
}

impl SseEvent {
    /// Returns the optional SSE event name.
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    /// Returns the concatenated data field.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Returns the optional SSE ID.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Returns the optional retry value in milliseconds.
    pub fn retry_ms(&self) -> Option<u64> {
        self.retry_ms
    }
}

/// Incremental decoder retaining incomplete line/event state.
///
/// `max_event_bytes` is measured per assembled SSE event rather than per network chunk, preventing
/// attackers from bypassing memory limits with unlimited fragmentation.
pub struct SseDecoder {
    max_event_bytes: usize,
    buffered: BytesMut,
    current_bytes: usize,
    current: EventBuilder,
}

impl SseDecoder {
    /// Creates an incremental decoder with a per-event size limit.
    pub fn new(max_event_bytes: usize) -> Self {
        Self {
            max_event_bytes,
            buffered: BytesMut::new(),
            current_bytes: 0,
            current: EventBuilder::default(),
        }
    }

    /// Writes a network chunk to the decoder and returns completed events.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, SseDecodeError> {
        // Buffer the chunk and split complete SSE lines at newline boundaries.
        self.buffered.extend_from_slice(chunk);
        let mut events = Vec::new();

        while let Some(newline) = self.buffered.iter().position(|byte| *byte == b'\n') {
            let mut raw_line = self.buffered.split_to(newline + 1);
            self.current_bytes = self
                .current_bytes
                .checked_add(raw_line.len())
                .ok_or(SseDecodeError::EventTooLarge)?;
            if self.current_bytes > self.max_event_bytes {
                return Err(SseDecodeError::EventTooLarge);
            }

            raw_line.truncate(raw_line.len() - 1);
            if raw_line.last() == Some(&b'\r') {
                raw_line.truncate(raw_line.len() - 1);
            }

            // Process blank-line event termination, comment lines, and ordinary UTF-8 fields.
            if raw_line.is_empty() {
                if let Some(event) = self.current.take_event() {
                    events.push(event);
                }
                self.current_bytes = 0;
                continue;
            }

            if raw_line.starts_with(b":") {
                continue;
            }

            let line = std::str::from_utf8(&raw_line).map_err(|_| SseDecodeError::InvalidUtf8)?;
            self.current.apply_line(line);
        }

        // Check whether an incomplete line/event exceeds the total size limit.
        self.ensure_size_limit()?;
        Ok(events)
    }

    /// Consumes at most the raw prefix ending at the first complete non-empty SSE event.
    ///
    /// The returned byte count lets ingress buffer exactly one event and retain additional events
    /// from the same network chunk for post-commit delivery without reserialization.
    pub(crate) fn push_until_event(
        &mut self,
        chunk: &[u8],
    ) -> Result<(Option<SseEvent>, usize), SseDecodeError> {
        let mut consumed = 0_usize;
        while let Some(relative_newline) = chunk[consumed..].iter().position(|byte| *byte == b'\n')
        {
            let end = consumed + relative_newline + 1;
            let mut events = self.push(&chunk[consumed..end])?;
            consumed = end;
            if let Some(event) = events.pop() {
                debug_assert!(events.is_empty());
                return Ok((Some(event), consumed));
            }
        }
        if consumed < chunk.len() {
            debug_assert!(self.push(&chunk[consumed..])?.is_empty());
        }
        Ok((None, chunk.len()))
    }

    /// Marks input complete and returns events completed before EOF.
    pub fn finish(&mut self) -> Result<Vec<SseEvent>, SseDecodeError> {
        // Process content remaining before EOF without a newline as the final line.
        if !self.buffered.is_empty() {
            self.current_bytes = self
                .current_bytes
                .checked_add(self.buffered.len())
                .ok_or(SseDecodeError::EventTooLarge)?;
            if self.current_bytes > self.max_event_bytes {
                return Err(SseDecodeError::EventTooLarge);
            }

            let mut raw_line = self.buffered.split();
            if raw_line.last() == Some(&b'\r') {
                raw_line.truncate(raw_line.len() - 1);
            }
            if !raw_line.starts_with(b":") {
                let line =
                    std::str::from_utf8(&raw_line).map_err(|_| SseDecodeError::InvalidUtf8)?;
                self.current.apply_line(line);
            }
        }

        // Clear counters and return only the final event with fields already collected.
        self.current_bytes = 0;
        Ok(self.current.take_event().into_iter().collect())
    }

    /// Checks whether the buffer for an incomplete line remains within the per-event limit.
    fn ensure_size_limit(&self) -> Result<(), SseDecodeError> {
        if self.current_bytes.saturating_add(self.buffered.len()) > self.max_event_bytes {
            Err(SseDecodeError::EventTooLarge)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct EventBuilder {
    event: Option<String>,
    data_lines: Vec<String>,
    id: Option<String>,
    retry_ms: Option<u64>,
    has_fields: bool,
}

impl EventBuilder {
    /// Parses one SSE field and retains only fields required by the current protocol boundary.
    fn apply_line(&mut self, line: &str) {
        // Parse the field/value and ignore SSE fields not modeled by the protocol.
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);

        match field {
            "event" => {
                self.event = Some(value.to_owned());
                self.has_fields = true;
            }
            "data" => {
                self.data_lines.push(value.to_owned());
                self.has_fields = true;
            }
            "id" if !value.contains('\0') => {
                self.id = Some(value.to_owned());
                self.has_fields = true;
            }
            "retry" => {
                if let Ok(retry_ms) = value.parse() {
                    self.retry_ms = Some(retry_ms);
                    self.has_fields = true;
                }
            }
            _ => {}
        }
    }

    /// Finalizes the current event at a blank line or EOF and resets builder state.
    fn take_event(&mut self) -> Option<SseEvent> {
        // Empty events produce no output; complete events transfer field ownership once.
        if !self.has_fields {
            return None;
        }

        Some(SseEvent {
            event: self.event.take(),
            data: std::mem::take(&mut self.data_lines).join("\n"),
            id: self.id.take(),
            retry_ms: self.retry_ms.take(),
        })
        .inspect(|_| self.has_fields = false)
    }
}

#[cfg(test)]
mod tests {
    use super::SseDecoder;

    #[test]
    fn precommit_decode_returns_exactly_one_raw_event_prefix() {
        let payload = b": keepalive\n\nevent: first\ndata: one\n\nevent: second\ndata: two\n\n";
        let expected = b": keepalive\n\nevent: first\ndata: one\n\n";
        let mut decoder = SseDecoder::new(128);

        let (event, consumed) = decoder.push_until_event(payload).unwrap();

        assert_eq!(event.unwrap().event(), Some("first"));
        assert_eq!(&payload[..consumed], expected);
        assert_eq!(&payload[consumed..], b"event: second\ndata: two\n\n");
    }
}
