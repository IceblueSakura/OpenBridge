//! Forwarding-level validation and Public Model projection for Native Images success responses.
//!
//! The complete upstream body is validated before downstream commit. Image URLs are held only in
//! the bounded response value and never enter ordinary telemetry; explicitly enabled bounded
//! downstream content logging still observes the final client response by global policy.

use axum::{body::Body, response::Response};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;

use crate::{
    core::ImagesResponseFormat,
    ingress::response::filtered_upstream_headers,
    observability::{ErrorType, RequestObservation},
    pipeline::{validate_images_response_body, validate_images_response_headers},
    transport::upstream::UpstreamResponse,
};

/// Closed failure category for the bounded Images response lifecycle before downstream commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImagesResponseLifecycleError {
    InvalidHeaders,
    TooLarge,
    BodyTransport,
    InvalidContract,
}

impl ImagesResponseLifecycleError {
    /// Maps lifecycle failures into the fixed telemetry taxonomy without retaining body content.
    pub(super) const fn error_type(self) -> ErrorType {
        match self {
            Self::BodyTransport => ErrorType::UpstreamBodyTransport,
            Self::InvalidHeaders | Self::TooLarge | Self::InvalidContract => {
                ErrorType::InvalidUpstreamResponse
            }
        }
    }
}

/// Validates one successful Images response and projects its model for downstream delivery.
#[allow(clippy::too_many_arguments)]
pub(super) async fn validated_images_response(
    upstream: UpstreamResponse,
    observation: &RequestObservation,
    public_model: &str,
    outputs: u32,
    response_format: ImagesResponseFormat,
    max_body_bytes: usize,
) -> Result<Response, ImagesResponseLifecycleError> {
    // Validate response metadata before reading or interpreting the successful body.
    validate_images_response_headers(upstream.headers())
        .map_err(|_| ImagesResponseLifecycleError::InvalidHeaders)?;
    let status = upstream.status();
    let headers = filtered_upstream_headers(upstream.headers());

    // Read the entire response under its independent pre-commit memory boundary.
    let body = read_bounded_images_body(upstream.into_body(), max_body_bytes, |chunk| {
        observation.record_upstream_chunk(chunk);
    })
    .await?;
    let validated = validate_images_response_body(
        &body,
        public_model,
        outputs,
        response_format,
        max_body_bytes,
    )
    .map_err(|_| ImagesResponseLifecycleError::InvalidContract)?;
    observation.record_upstream_complete();
    let (image_count, output_width, output_height) = validated.image_usage();
    observation.record_images_usage(image_count, output_width, output_height);

    // Commit the fully validated, bounded JSON response with only allowlisted upstream headers.
    let mut response = Response::builder()
        .status(status)
        .body(axum::body::Body::from(validated.into_body()))
        .expect("validated upstream status builds a response");
    response.headers_mut().extend(headers);
    Ok(response)
}

/// Reads one Images response body with distinct overflow and transport failure outcomes.
async fn read_bounded_images_body(
    body: Body,
    max_body_bytes: usize,
    mut observe_chunk: impl FnMut(&Bytes),
) -> Result<Bytes, ImagesResponseLifecycleError> {
    let mut source = body.into_data_stream();
    let mut captured = BytesMut::with_capacity(max_body_bytes.min(8 * 1024));
    while let Some(chunk) = source.next().await {
        let chunk = chunk.map_err(|_| ImagesResponseLifecycleError::BodyTransport)?;
        observe_chunk(&chunk);
        if captured.len().saturating_add(chunk.len()) > max_body_bytes {
            return Err(ImagesResponseLifecycleError::TooLarge);
        }
        captured.extend_from_slice(&chunk);
    }
    Ok(captured.freeze())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use bytes::Bytes;
    use futures_util::stream;

    use super::{ImagesResponseLifecycleError, read_bounded_images_body};
    use crate::observability::ErrorType;

    #[tokio::test]
    async fn bounded_images_body_distinguishes_overflow_from_transport_failure() {
        assert!(matches!(
            read_bounded_images_body(Body::from("12345"), 4, |_| {}).await,
            Err(ImagesResponseLifecycleError::TooLarge)
        ));

        let body = Body::from_stream(stream::once(async {
            Err::<Bytes, _>(std::io::Error::other("synthetic body failure"))
        }));
        assert!(matches!(
            read_bounded_images_body(body, 16, |_| {}).await,
            Err(ImagesResponseLifecycleError::BodyTransport)
        ));
        assert_eq!(
            ImagesResponseLifecycleError::BodyTransport.error_type(),
            ErrorType::UpstreamBodyTransport
        );
        assert_eq!(
            ImagesResponseLifecycleError::TooLarge.error_type(),
            ErrorType::InvalidUpstreamResponse
        );
    }
}
