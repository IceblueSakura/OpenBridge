//! Cancellation, terminal-state, and first-output lifecycle for downstream response bodies.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use axum::response::Response;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http_body::{Body as HttpBody, Frame, SizeHint};

use crate::observability::{FirstOutputCapture, RequestObservation};

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

/// Uses a byte-transparent outer stream to finish request observation at real EOF, error, or drop.
pub(super) fn observe_response_body(
    response: &mut Response,
    observation: RequestObservation,
    max_sse_event_bytes: usize,
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
    *response.body_mut() =
        axum::body::Body::new(RequestBodyObserver::new(body, observation, first_output));
}

/// Preserves raw HTTP frames and submits the request terminal at the actual body-consumption boundary.
struct RequestBodyObserver {
    body: axum::body::Body,
    observation: RequestObservation,
    first_output: FirstOutputCapture,
    finished: bool,
}

impl RequestBodyObserver {
    /// Creates a transparent body wrapper with no first byte or terminal state yet.
    fn new(
        body: axum::body::Body,
        observation: RequestObservation,
        first_output: FirstOutputCapture,
    ) -> Self {
        Self {
            body,
            observation,
            first_output,
            finished: false,
        }
    }

    /// Flushes a pending first-output event and submits one successful terminal at real EOF.
    fn complete(&mut self) {
        // At normal EOF, flush the final output event before the request terminal.
        if self.finished {
            return;
        }
        self.first_output.finish(&self.observation);
        self.observation.finish();
        self.finished = true;
    }

    /// Records a failure category and submits one terminal at the body-error boundary.
    fn fail(&mut self, kind: &'static str) {
        // A body error is the final visible boundary; do not wait for another poll to record it.
        if self.finished {
            return;
        }
        self.observation.record_stream_failure(kind);
        self.observation.finish();
        self.finished = true;
    }
}

impl HttpBody for RequestBodyObserver {
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
                observer.fail("body_error");
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

impl Drop for RequestBodyObserver {
    fn drop(&mut self) {
        // Missing an underlying terminal means HTTP transport stopped consuming before the response completed.
        if !self.finished {
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

    use super::RequestBodyObserver;
    use crate::observability::{FirstOutputCapture, GatewayMetrics, RequestObservation};

    #[test]
    fn complete_single_frame_body_finishes_without_a_separate_eof_poll() {
        // Build a complete in-memory body whose underlying end-stream arrives immediately after the first frame.
        let metrics = GatewayMetrics::default();
        let observation = RequestObservation::new(metrics.clone(), tracing::Span::none());
        observation.record_response_ready(http::StatusCode::OK);
        {
            let mut observer = pin!(RequestBodyObserver::new(
                Body::from("complete"),
                observation,
                FirstOutputCapture::None,
            ));
            let mut context = Context::from_waker(noop_waker_ref());

            // After consuming the only data frame, the outer stream inherits the terminal and submits completed immediately.
            assert!(matches!(
                observer.as_mut().poll_frame(&mut context),
                std::task::Poll::Ready(Some(Ok(_)))
            ));
            assert!(observer.is_end_stream());
        }

        // Normal EOF may count only as completed; Drop must not misclassify it as cancelled.
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.requests_completed, 1);
        assert_eq!(snapshot.requests_cancelled, 0);
    }
}
