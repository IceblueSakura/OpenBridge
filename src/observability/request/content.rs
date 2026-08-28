//! Local downstream HTTP content snapshot policy and JSONL submission.
//!
//! This observer owns only startup-frozen content switches, one request identity, and the dedicated
//! JSONL writer. It does not own tracing, metrics, Provider attempts, or request terminal state.

use http::{HeaderMap, Method, StatusCode};

use crate::config::HttpLoggingConfig;

use super::super::http_jsonl::{HttpJsonlWriter, JsonlRecord};

/// Per-request local content observer isolated from telemetry lifecycle state.
pub(super) struct DownstreamContentObserver {
    request_id: String,
    policy: HttpLoggingConfig,
    writer: Option<HttpJsonlWriter>,
}

impl DownstreamContentObserver {
    pub(super) fn new(
        request_id: String,
        policy: HttpLoggingConfig,
        writer: Option<HttpJsonlWriter>,
    ) -> Self {
        Self {
            request_id,
            policy,
            writer,
        }
    }

    pub(super) fn log_request_headers(&self, method: &Method, path: &str, headers: &HeaderMap) {
        if self.policy.request_headers()
            && let Some(writer) = &self.writer
        {
            writer.try_enqueue(JsonlRecord::request_headers(
                &self.request_id,
                method.as_str(),
                path,
                headers,
            ));
        }
    }

    pub(super) fn logs_request_body(&self) -> bool {
        self.policy.request_body()
    }

    pub(super) fn log_request_body(
        &self,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) {
        if self.policy.request_body()
            && let Some(writer) = &self.writer
        {
            writer.try_enqueue(JsonlRecord::request_body(
                &self.request_id,
                bytes,
                total_bytes,
                complete,
                truncated,
            ));
        }
    }

    pub(super) fn log_response_headers(&self, status: StatusCode, headers: &HeaderMap) {
        if self.policy.response_headers()
            && let Some(writer) = &self.writer
        {
            writer.try_enqueue(JsonlRecord::response_headers(
                &self.request_id,
                status.as_u16(),
                headers,
            ));
        }
    }

    pub(super) fn logs_response_body(&self) -> bool {
        self.policy.response_body()
    }

    pub(super) fn log_response_body(
        &self,
        bytes: &[u8],
        total_bytes: usize,
        complete: bool,
        truncated: bool,
    ) {
        if self.policy.response_body()
            && let Some(writer) = &self.writer
        {
            writer.try_enqueue(JsonlRecord::response_body(
                &self.request_id,
                bytes,
                total_bytes,
                complete,
                truncated,
            ));
        }
    }
}
