//! JSONL record schema v1 for authenticated downstream HTTP content snapshots.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use http::HeaderMap;
use serde::Serialize;

use super::redaction;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotKind {
    RequestHeaders,
    RequestBody,
    ResponseHeaders,
    ResponseBody,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HeaderEntry {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct JsonlRecord {
    schema_version: u32,
    timestamp: String,
    request_id: String,
    kind: SnapshotKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<Vec<HeaderEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    captured_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<bool>,
}

impl JsonlRecord {
    pub(crate) fn request_headers(
        request_id: &str,
        method: &str,
        path: &str,
        headers: &HeaderMap,
    ) -> Self {
        Self::headers(
            request_id,
            SnapshotKind::RequestHeaders,
            Some(method),
            Some(path),
            None,
            headers,
        )
    }

    pub(crate) fn response_headers(request_id: &str, status: u16, headers: &HeaderMap) -> Self {
        Self::headers(
            request_id,
            SnapshotKind::ResponseHeaders,
            None,
            None,
            Some(status),
            headers,
        )
    }

    fn headers(
        request_id: &str,
        kind: SnapshotKind,
        method: Option<&str>,
        path: Option<&str>,
        status: Option<u16>,
        headers: &HeaderMap,
    ) -> Self {
        Self {
            schema_version: 1,
            timestamp: timestamp(),
            request_id: request_id.to_owned(),
            kind,
            method: method.map(str::to_owned),
            path: path.map(str::to_owned),
            status,
            headers: Some(redaction::redact_headers(headers)),
            body_base64: None,
            body_text: None,
            captured_bytes: None,
            observed_bytes: None,
            complete: None,
            truncated: None,
        }
    }

    pub(crate) fn request_body(
        request_id: &str,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) -> Self {
        Self::body(
            request_id,
            SnapshotKind::RequestBody,
            bytes,
            total_bytes,
            complete,
            truncated,
        )
    }

    pub(crate) fn response_body(
        request_id: &str,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) -> Self {
        Self::body(
            request_id,
            SnapshotKind::ResponseBody,
            bytes,
            total_bytes,
            complete,
            truncated,
        )
    }

    fn body(
        request_id: &str,
        kind: SnapshotKind,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) -> Self {
        Self {
            schema_version: 1,
            timestamp: timestamp(),
            request_id: request_id.to_owned(),
            kind,
            method: None,
            path: None,
            status: None,
            headers: None,
            body_base64: Some(BASE64.encode(bytes)),
            body_text: std::str::from_utf8(bytes).ok().map(str::to_owned),
            captured_bytes: Some(bytes.len()),
            observed_bytes: Some(total_bytes),
            complete: Some(complete),
            truncated: Some(truncated),
        }
    }

    pub(crate) fn to_jsonl_line(&self) -> Vec<u8> {
        // This schema contains only serializable owned primitives, so serialization is infallible.
        let mut line = serde_json::to_vec(self).expect("HTTP JSONL record must serialize");
        line.push(b'\n');
        line
    }
}

fn timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
