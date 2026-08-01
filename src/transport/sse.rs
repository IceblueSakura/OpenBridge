//! 增量 SSE framing decoder。
//!
//! 网络 chunk 不是 UTF-8、行或 SSE event 边界。本 decoder 只负责把 byte stream 组织成
//! 完整 `SseEvent`：支持 CRLF、注释、多行 `data:` 和 event size 上限；具体 event 的协议
//! 含义由 `ProviderAdapter::classify_sse_event` 判定。

use bytes::BytesMut;
use thiserror::Error;

/// SSE framing 或单事件大小校验失败。
#[derive(Debug, Error, Eq, PartialEq)]
pub enum SseDecodeError {
    #[error("SSE event exceeds the configured size limit")]
    EventTooLarge,
    #[error("SSE field is not valid UTF-8")]
    InvalidUtf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// 一个已完成 SSE framing 的事件。
pub struct SseEvent {
    event: Option<String>,
    data: String,
    id: Option<String>,
    retry_ms: Option<u64>,
}

impl SseEvent {
    /// 返回可选的 SSE event 名称。
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    /// 返回拼接后的 data 字段。
    pub fn data(&self) -> &str {
        &self.data
    }

    /// 返回可选的 SSE id。
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// 返回可选的 retry 毫秒值。
    pub fn retry_ms(&self) -> Option<u64> {
        self.retry_ms
    }
}

/// 保留未完成行/事件状态的增量 decoder。
///
/// `max_event_bytes` 按正在组装的 SSE event 计量而不是按网络 chunk 计量，防止攻击者通过
/// 无限分片规避内存限制。
pub struct SseDecoder {
    max_event_bytes: usize,
    buffered: BytesMut,
    current_bytes: usize,
    current: EventBuilder,
}

impl SseDecoder {
    /// 创建一个限制单事件大小的增量 decoder。
    pub fn new(max_event_bytes: usize) -> Self {
        Self {
            max_event_bytes,
            buffered: BytesMut::new(),
            current_bytes: 0,
            current: EventBuilder::default(),
        }
    }

    /// 向 decoder 写入网络 chunk，并返回已经完成的 event。
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

    /// 标记输入结束，并返回 EOF 前已完成的 event。
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
