//! Bounded JSON response decoding for administrative probes.

use axum::body::to_bytes;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;

use crate::transport::upstream::UpstreamResponse;

use super::super::{ProbeFailure, ProbeResult};

pub(super) fn canonical_content_type(headers: &HeaderMap) -> Option<String> {
    let media_type = headers
        .get(CONTENT_TYPE)?
        .to_str()
        .ok()?
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase();
    (!media_type.is_empty()
        && media_type.len() <= 64
        && media_type
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'+' | b'.')))
    .then_some(media_type)
}

pub(super) struct JsonResponse {
    pub(super) status: StatusCode,
    pub(super) body: Value,
    pub(super) content_type: Option<String>,
}

/// Reads a successful JSON response within the fixed limit and classifies HTTP failures first.
pub(super) async fn decode_json_response(
    response: UpstreamResponse,
    max_response_bytes: usize,
) -> Result<JsonResponse, ProbeResult> {
    // Read the response body within the configured limit and classify HTTP failures first.
    let status = response.status();
    let content_type = canonical_content_type(response.headers());
    if !status.is_success() {
        return Err(ProbeResult::from_http_status(status));
    }
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await
        .map_err(|_| ProbeResult::inconclusive(Some(status), ProbeFailure::ResponseLimit))?;

    // Accept only valid JSON so an error page cannot be reported as protocol success.
    let body = serde_json::from_slice(&body)
        .map_err(|_| ProbeResult::inconclusive(Some(status), ProbeFailure::InvalidJson))?;
    Ok(JsonResponse {
        status,
        body,
        content_type,
    })
}
