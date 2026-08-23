//! Transparent observation of non-streaming Provider response bodies.
//!
//! Successful bodies collect a bounded JSON copy for usage and terminal observation. Non-success
//! bodies only preserve typed timeout attribution. Both modes forward every raw frame unchanged
//! and never retain or log a business body after completion.

use axum::body::Body;
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde_json::Value;

use crate::{
    observability::{TimeoutPhase, request::RequestObservation},
    transport::is_timeout_error,
};

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
        observe_json: true,
        finished: false,
    })
}

/// Transparently preserves a non-success body while retaining only typed timeout attribution.
pub(crate) fn observe_timeout_body(body: Body, observation: RequestObservation) -> Body {
    Body::new(ProviderBodyObserver {
        body,
        observation,
        bytes: Vec::new(),
        limit: 0,
        truncated: false,
        observe_json: false,
        finished: false,
    })
}

struct ProviderBodyObserver {
    body: Body,
    observation: RequestObservation,
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    observe_json: bool,
    finished: bool,
}

impl ProviderBodyObserver {
    /// Parses usage and submits the Provider-attempt terminal when the raw upstream body completes.
    fn complete(&mut self) {
        // Prevent the final frame and later EOF from submitting the same attempt twice.
        if self.finished {
            return;
        }
        if self.observe_json {
            if !self.truncated
                && let Ok(value) = serde_json::from_slice::<Value>(&self.bytes)
            {
                self.observation.record_upstream_value(&value);
            }
            self.observation.record_upstream_complete();
        }
        self.finished = true;
    }

    /// Submits a failure terminal at the upstream body-error boundary.
    fn fail(&mut self, error: &axum::Error) {
        if self.finished {
            return;
        }
        if is_timeout_error(error) {
            self.observation
                .record_stream_timeout(TimeoutPhase::NonStreamingTotal);
        } else if self.observe_json {
            self.observation.record_upstream_failure();
        }
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
                if let Some(chunk) = frame.data_ref()
                    && observer.observe_json
                {
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
                observer.fail(&error);
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

#[cfg(test)]
mod tests {
    use std::io;

    use axum::body::{Body, to_bytes};
    use bytes::Bytes;
    use futures_util::stream;

    use crate::observability::{GatewayMetrics, RequestObservation, TimeoutPhase};

    use super::{observe_json_body, observe_timeout_body};

    fn observation() -> RequestObservation {
        RequestObservation::new(GatewayMetrics::default(), tracing::Span::none())
    }

    #[tokio::test]
    async fn non_streaming_body_timeout_keeps_a_typed_phase() {
        let observation = observation();
        let body = Body::from_stream(stream::once(async {
            Err::<Bytes, io::Error>(io::Error::new(io::ErrorKind::TimedOut, "synthetic timeout"))
        }));

        let error = to_bytes(observe_timeout_body(body, observation.clone()), 1024)
            .await
            .expect_err("the synthetic body must fail");

        assert!(format!("{error:?}").contains("TimedOut"));
        assert_eq!(
            observation.timeout_context_for_test(),
            Some((TimeoutPhase::NonStreamingTotal, false, None))
        );
    }

    #[tokio::test]
    async fn non_timeout_body_errors_do_not_gain_timeout_context() {
        let observation = observation();
        let body = Body::from_stream(stream::once(async {
            Err::<Bytes, io::Error>(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "synthetic reset",
            ))
        }));

        let _ = to_bytes(observe_timeout_body(body, observation.clone()), 1024)
            .await
            .expect_err("the synthetic body must fail");

        assert_eq!(observation.timeout_context_for_test(), None);
    }

    #[tokio::test]
    async fn successful_json_body_uses_the_same_typed_timeout_detection() {
        let observation = observation();
        let body = Body::from_stream(stream::once(async {
            Err::<Bytes, io::Error>(io::Error::new(io::ErrorKind::TimedOut, "synthetic timeout"))
        }));

        let _ = to_bytes(observe_json_body(body, observation.clone(), 1024), 1024)
            .await
            .expect_err("the synthetic body must fail");

        assert_eq!(
            observation.timeout_context_for_test(),
            Some((TimeoutPhase::NonStreamingTotal, false, None))
        );
    }
}
