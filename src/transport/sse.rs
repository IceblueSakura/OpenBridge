use bytes::BytesMut;
use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SseDecodeError {
    #[error("SSE event exceeds the configured size limit")]
    EventTooLarge,
    #[error("SSE field is not valid UTF-8")]
    InvalidUtf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SseEvent {
    event: Option<String>,
    data: String,
    id: Option<String>,
    retry_ms: Option<u64>,
}

impl SseEvent {
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn retry_ms(&self) -> Option<u64> {
        self.retry_ms
    }
}

pub struct SseDecoder {
    max_event_bytes: usize,
    buffered: BytesMut,
    current_bytes: usize,
    current: EventBuilder,
}

impl SseDecoder {
    pub fn new(max_event_bytes: usize) -> Self {
        Self {
            max_event_bytes,
            buffered: BytesMut::new(),
            current_bytes: 0,
            current: EventBuilder::default(),
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, SseDecodeError> {
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

        self.ensure_size_limit()?;
        Ok(events)
    }

    pub fn finish(&mut self) -> Result<Vec<SseEvent>, SseDecodeError> {
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

        self.current_bytes = 0;
        Ok(self.current.take_event().into_iter().collect())
    }

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
    fn apply_line(&mut self, line: &str) {
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

    fn take_event(&mut self) -> Option<SseEvent> {
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
