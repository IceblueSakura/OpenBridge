//! Transparent observation of successful non-streaming Provider response bodies.
//!
//! The wrapper forwards every raw frame unchanged while collecting only a bounded JSON copy for
//! usage and terminal observation. It never retains or logs a business body after completion.

use axum::body::Body;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde_json::Value;

use crate::observability::request::RequestObservation;

/// Transparently observes a non-SSE upstream body and parses bounded JSON usage.
pub(crate) fn observe_json_body(
    body: Body,
    observation: RequestObservation,
    max_json_body_bytes: usize,
) -> Body {
    Body::new(ProviderBodyObserver {
        body,
        observation,
        bytes: Vec::new(),
        limit: max_json_body_bytes,
        truncated: false,
        finished: false,
    })
}

struct ProviderBodyObserver {
    body: Body,
    observation: RequestObservation,
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    finished: bool,
}

impl ProviderBodyObserver {
    /// Parses usage and submits the Provider-attempt terminal when the raw upstream body completes.
    fn complete(&mut self) {
        // Prevent the final frame and later EOF from submitting the same attempt twice.
        if self.finished {
            return;
        }
        if !self.truncated
            && let Ok(value) = serde_json::from_slice::<Value>(&self.bytes)
        {
            self.observation.record_upstream_value(&value);
        }
        self.observation.record_upstream_complete();
        self.finished = true;
    }

    /// Submits a failure terminal at the upstream body-error boundary.
    fn fail(&mut self) {
        if self.finished {
            return;
        }
        self.observation.record_upstream_failure();
        self.finished = true;
    }
}

impl HttpBody for ProviderBodyObserver {
    type Data = Bytes;
    type Error = axum::Error;

    /// Forwards raw frames and records the upstream first byte and bounded JSON usage.
    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.as_mut().get_mut();
        match std::pin::Pin::new(&mut observer.body).poll_frame(context) {
            std::task::Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    observer.observation.record_upstream_chunk(chunk);
                    if !observer.truncated
                        && observer.bytes.len().saturating_add(chunk.len()) <= observer.limit
                    {
                        observer.bytes.extend_from_slice(chunk);
                    } else {
                        observer.bytes.clear();
                        observer.truncated = true;
                    }
                }
                // A known-length upstream body can complete after the final frame without waiting for another EOF through nested wrappers.
                if observer.body.is_end_stream() {
                    observer.complete();
                }
                std::task::Poll::Ready(Some(Ok(frame)))
            }
            std::task::Poll::Ready(Some(Err(error))) => {
                observer.fail();
                std::task::Poll::Ready(Some(Err(error)))
            }
            std::task::Poll::Ready(None) => {
                observer.complete();
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    /// Reports body completion only after real upstream EOF or error.
    fn is_end_stream(&self) -> bool {
        self.finished
    }

    /// Preserves the raw body size hint.
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}
