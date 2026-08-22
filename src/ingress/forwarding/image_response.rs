//! Forwarding-level validation and Public Model projection for Native Images success responses.
//!
//! The complete upstream body is validated before downstream commit. Image URLs are held only in
//! the bounded response value, are never logged, and are serialized without conversion.

use axum::{body::to_bytes, response::Response};

use crate::{
    core::ImagesResponseFormat,
    ingress::response::filtered_upstream_headers,
    observability::RequestObservation,
    pipeline::{
        ImagesResponseError, validate_images_response_body, validate_images_response_headers,
    },
    transport::upstream::UpstreamResponse,
};

/// Validates one successful Images response and projects its model for downstream delivery.
#[allow(clippy::too_many_arguments)]
pub(super) async fn validated_images_response(
    upstream: UpstreamResponse,
    observation: &RequestObservation,
    public_model: &str,
    outputs: u32,
    response_format: ImagesResponseFormat,
    max_body_bytes: usize,
) -> Result<Response, ImagesResponseError> {
    // Validate response metadata before reading or interpreting the successful body.
    validate_images_response_headers(upstream.headers())?;
    let status = upstream.status();
    let headers = filtered_upstream_headers(upstream.headers());

    // Read the entire response under its independent pre-commit memory boundary.
    let body = to_bytes(upstream.into_body(), max_body_bytes)
        .await
        .map_err(|_| ImagesResponseError)?;
    observation.record_upstream_complete();
    let validated = validate_images_response_body(
        &body,
        public_model,
        outputs,
        response_format,
        max_body_bytes,
    )?;

    // Commit the fully validated, bounded JSON response with only allowlisted upstream headers.
    let mut response = Response::builder()
        .status(status)
        .body(axum::body::Body::from(validated.into_body()))
        .expect("validated upstream status builds a response");
    response.headers_mut().extend(headers);
    Ok(response)
}
