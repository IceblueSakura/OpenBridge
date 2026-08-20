//! Forwarding-level validation and Public Model projection for Native Embeddings success responses.
//!
//! The complete upstream body is validated before downstream commit. Business vectors are held
//! only in the bounded response value, are never logged, and are serialized without conversion.

use axum::{body::to_bytes, response::Response};

use crate::{
    core::EmbeddingEncoding,
    ingress::response::filtered_upstream_headers,
    observability::RequestObservation,
    pipeline::{
        EmbeddingResponseError, validate_embedding_response_body,
        validate_embedding_response_headers,
    },
    transport::upstream::UpstreamResponse,
};

/// Validates one successful Embeddings response and projects its model for downstream delivery.
#[allow(clippy::too_many_arguments)]
pub(super) async fn validated_embedding_response(
    upstream: UpstreamResponse,
    observation: &RequestObservation,
    public_model: &str,
    upstream_model: &str,
    input_count: u32,
    encoding: EmbeddingEncoding,
    dimensions: u32,
    max_body_bytes: usize,
) -> Result<Response, EmbeddingResponseError> {
    // Validate response metadata before reading or interpreting the successful body.
    validate_embedding_response_headers(upstream.headers())?;
    let status = upstream.status();
    let headers = filtered_upstream_headers(upstream.headers());

    // Read the entire response under its independent pre-commit memory boundary.
    let body = to_bytes(upstream.into_body(), max_body_bytes)
        .await
        .map_err(|_| EmbeddingResponseError)?;
    observation.record_upstream_complete();
    let validated = validate_embedding_response_body(
        &body,
        public_model,
        upstream_model,
        input_count,
        encoding,
        dimensions,
        max_body_bytes,
    )?;

    // Submit only usage from the fully validated success body; never retain input or vector values.
    let (input_tokens, total_tokens) = validated.usage();
    observation.record_embedding_usage(input_tokens, total_tokens);

    // Commit the fully validated, bounded JSON response with only allowlisted upstream headers.
    let mut response = Response::builder()
        .status(status)
        .body(axum::body::Body::from(validated.into_body()))
        .expect("validated upstream status builds a response");
    response.headers_mut().extend(headers);
    Ok(response)
}
