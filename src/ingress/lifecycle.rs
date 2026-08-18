//! Authenticated HTTP content logging plus cancellation, terminal, and first-output lifecycles.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use axum::{extract::Request, response::Response};
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http_body::{Body as HttpBody, Frame, SizeHint};

use crate::observability::{FirstOutputCapture, RequestObservation};

/// Retains a bounded prefix and total byte count for one explicitly enabled local body snapshot.
struct BodyLogCapture {
    bytes: Vec<u8>,
    total_bytes: usize,
    limit: usize,
    truncated: bool,
}

impl BodyLogCapture {
    /// Creates an empty capture under an existing non-zero runtime body boundary.
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            total_bytes: 0,
            limit,
            truncated: false,
        }
    }

    /// Appends as much of one chunk as the capture boundary permits.
    fn record(&mut self, chunk: &Bytes) {
        // Count the full observed stream even after the retained prefix reaches its limit.
        self.total_bytes = self.total_bytes.saturating_add(chunk.len());

        // Retain only the bounded prefix and mark every omitted suffix explicitly.
        let remaining = self.limit.saturating_sub(self.bytes.len());
        let retained = remaining.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..retained]);
        if retained < chunk.len() {
            self.truncated = true;
        }
    }
}

/// Captures a request whose middleware future is cancelled before a response body exists.
pub(super) struct RequestLifecycleGuard {
    observation: Option<RequestObservation>,
}

impl RequestLifecycleGuard {
    /// Creates a lifecycle guard still owned by the request future.
    pub(super) fn new(observation: RequestObservation) -> Self {
        Self {
            observation: Some(observation),
        }
    }

    /// Transfers cancellation and terminal-state responsibility after the response-body wrapper is created.
    pub(super) fn handoff_to_body(&mut self) {
        self.observation.take();
    }
}

impl Drop for RequestLifecycleGuard {
    fn drop(&mut self) {
        // Close the request here when cancellation occurs during pending send, backoff, or handler work before a body wrapper exists.
        if let Some(observation) = self.observation.take() {
            observation.cancel();
        }
    }
}

/// Wraps an authenticated downstream request body only when local body logging is enabled.
pub(super) fn observe_request_body(
    request: &mut Request,
    observation: RequestObservation,
    max_request_body_bytes: usize,
) {
    // Leave the ordinary extractor path allocation-free when request body logging is disabled.
    if !observation.logs_request_body() {
        return;
    }

    // Preserve every frame while retaining at most the already enforced request-body limit.
    let body = std::mem::replace(request.body_mut(), axum::body::Body::empty());
    if body.is_end_stream() {
        observation.log_request_body(&[], 0, true, false);
        return;
    }
    *request.body_mut() = axum::body::Body::new(DownstreamRequestBodyObserver::new(
        body,
        observation,
        max_request_body_bytes,
    ));
}

/// Preserves request frames and emits one body snapshot at EOF, error, or drop.
struct DownstreamRequestBodyObserver {
    body: axum::body::Body,
    observation: RequestObservation,
    capture: Option<BodyLogCapture>,
    finished: bool,
}

impl DownstreamRequestBodyObserver {
    /// Creates a transparent authenticated-request wrapper with an empty bounded capture.
    fn new(
        body: axum::body::Body,
        observation: RequestObservation,
        max_request_body_bytes: usize,
    ) -> Self {
        Self {
            body,
            observation,
            capture: Some(BodyLogCapture::new(max_request_body_bytes)),
            finished: false,
        }
    }

    /// Emits the single request-body event with its actual completion boundary.
    fn complete(&mut self, complete: bool) {
        // Prevent EOF, error, and Drop paths from emitting the same request snapshot twice.
        if self.finished {
            return;
        }

        // Move the bounded capture into one terminal local event before marking the wrapper complete.
        if let Some(capture) = self.capture.take() {
            self.observation.log_request_body(
                &capture.bytes,
                capture.total_bytes,
                complete,
                capture.truncated,
            );
        }
        self.finished = true;
    }
}

impl HttpBody for DownstreamRequestBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    /// Forwards every request frame while accumulating one bounded local snapshot.
    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.get_mut();
        match Pin::new(&mut observer.body).poll_frame(context) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref()
                    && let Some(capture) = observer.capture.as_mut()
                {
                    capture.record(chunk);
                }
                // A known-length request can end on its final frame without a separate EOF poll.
                if observer.body.is_end_stream() {
                    observer.complete(true);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                observer.complete(false);
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                observer.complete(true);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    /// Reports completion only after the underlying body reaches EOF or error.
    fn is_end_stream(&self) -> bool {
        self.finished
    }

    /// Preserves the incoming body size hint without changing admission behavior.
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

impl Drop for DownstreamRequestBodyObserver {
    fn drop(&mut self) {
        // A handler that stops consuming the request still receives one explicit partial snapshot.
        if !self.finished {
            self.complete(false);
        }
    }
}

/// Uses a byte-transparent outer stream to finish request observation at real EOF, error, or drop.
pub(super) fn observe_response_body(
    response: &mut Response,
    observation: RequestObservation,
    max_sse_event_bytes: usize,
    max_response_body_bytes: usize,
) {
    // Select the directly observable first-output boundary for successful generation responses.
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let first_output = if response.status().is_success() {
        FirstOutputCapture::for_response(
            content_type,
            max_sse_event_bytes,
            observation.observes_non_streaming_generation_output(),
        )
    } else {
        FirstOutputCapture::None
    };
    let body = std::mem::replace(response.body_mut(), axum::body::Body::empty());
    *response.body_mut() = axum::body::Body::new(DownstreamResponseBodyObserver::new(
        body,
        observation,
        first_output,
        max_response_body_bytes,
    ));
}

/// Preserves raw HTTP frames and submits the request terminal at the actual body-consumption boundary.
struct DownstreamResponseBodyObserver {
    body: axum::body::Body,
    observation: RequestObservation,
    first_output: FirstOutputCapture,
    capture: Option<BodyLogCapture>,
    finished: bool,
}

impl DownstreamResponseBodyObserver {
    /// Creates a transparent body wrapper with no first byte or terminal state yet.
    fn new(
        body: axum::body::Body,
        observation: RequestObservation,
        first_output: FirstOutputCapture,
        max_response_body_bytes: usize,
    ) -> Self {
        let capture = observation
            .logs_response_body()
            .then(|| BodyLogCapture::new(max_response_body_bytes));
        Self {
            body,
            observation,
            first_output,
            capture,
            finished: false,
        }
    }

    /// Emits the single response-body snapshot if its independent switch was enabled.
    fn log_body(&mut self, complete: bool) {
        if let Some(capture) = self.capture.take() {
            self.observation.log_response_body(
                &capture.bytes,
                capture.total_bytes,
                complete,
                capture.truncated,
            );
        }
    }

    /// Flushes a pending first-output event and submits one successful terminal at real EOF.
    fn complete(&mut self) {
        // At normal EOF, flush the final output event before the request terminal.
        if self.finished {
            return;
        }
        self.first_output.finish(&self.observation);
        self.log_body(true);
        self.observation.finish();
        self.finished = true;
    }

    /// Records a failure category and submits one terminal at the body-error boundary.
    fn fail(&mut self) {
        // A body error is the final visible boundary; do not wait for another poll to record it.
        if self.finished {
            return;
        }
        self.log_body(false);
        self.observation.record_downstream_failure();
        self.observation.finish();
        self.finished = true;
    }
}

impl HttpBody for DownstreamResponseBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    /// Forwards underlying frames and updates observation state at data, error, or EOF boundaries.
    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.get_mut();
        // Preserve every data/trailer frame; observe first byte and first generated output on data frames.
        match Pin::new(&mut observer.body).poll_frame(context) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    if let Some(capture) = observer.capture.as_mut() {
                        capture.record(chunk);
                    }
                    if !chunk.is_empty() {
                        observer.observation.record_first_body_byte();
                    }
                    observer
                        .first_output
                        .observe_chunk(&observer.observation, chunk);
                }
                // The underlying stream may end after the last data/trailer frame without another transport EOF poll.
                if observer.body.is_end_stream() {
                    observer.complete();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                observer.fail();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                observer.complete();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    /// Reports body completion to Hyper only after EOF or error has been submitted.
    fn is_end_stream(&self) -> bool {
        // Report completion only after EOF or error; otherwise Hyper may skip the final poll and misclassify a complete body as cancelled.
        self.finished
    }

    /// Preserves the underlying body-size hint without buffering stream content.
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

impl Drop for DownstreamResponseBodyObserver {
    fn drop(&mut self) {
        // Missing an underlying terminal means HTTP transport stopped consuming before the response completed.
        if !self.finished {
            self.log_body(false);
            self.observation.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{pin::pin, task::Context};

    use axum::body::Body;
    use futures_util::task::noop_waker_ref;
    use http_body::Body as HttpBody;

    use super::{BodyLogCapture, DownstreamResponseBodyObserver};
    use crate::observability::{FirstOutputCapture, GatewayMetrics, RequestObservation};

    #[test]
    fn complete_single_frame_body_finishes_without_a_separate_eof_poll() {
        // Build a complete in-memory body whose underlying end-stream arrives immediately after the first frame.
        let observation = RequestObservation::new(GatewayMetrics::default(), tracing::Span::none());
        observation.record_response_ready(http::StatusCode::OK);
        {
            let mut observer = pin!(DownstreamResponseBodyObserver::new(
                Body::from("complete"),
                observation,
                FirstOutputCapture::None,
                1_024,
            ));
            let mut context = Context::from_waker(noop_waker_ref());

            // After consuming the only data frame, the outer stream inherits the terminal and submits completed immediately.
            assert!(matches!(
                observer.as_mut().poll_frame(&mut context),
                std::task::Poll::Ready(Some(Ok(_)))
            ));
            assert!(observer.is_end_stream());
        }
    }

    #[test]
    fn body_log_capture_retains_a_bounded_prefix_and_counts_the_full_observed_body() {
        let mut capture = BodyLogCapture::new(5);

        // Cross the capture boundary in a later frame so truncation and total size remain explicit.
        capture.record(&bytes::Bytes::from_static(b"abc"));
        capture.record(&bytes::Bytes::from_static(b"defg"));

        assert_eq!(capture.bytes, b"abcde");
        assert_eq!(capture.total_bytes, 7);
        assert!(capture.truncated);
    }
}
