//! Incremental processing of Native and Bridged upstream SSE bodies.

mod bridge;
mod buffered_responses;
mod liveness;
mod native;
mod precommit;

use std::{collections::BTreeMap, error::Error, io, pin::Pin, time::Duration};

use bytes::{Bytes, BytesMut};
use futures_util::{Stream, StreamExt, stream};
use serde_json::{Map, Value};
use tokio::time::Instant;

use crate::{
    bridge::{BridgePlan, BridgeStreamRenderer, ResponsesStreamState, StreamTerminal},
    observability::{ErrorType, RequestObservation, TimeoutPhase},
    provider::{GenerationProviderAdapter, StreamEventStatus},
    registry::UpstreamTimeoutPolicy,
    transport::{
        is_timeout_error,
        sse::{SseDecoder, SseEvent},
    },
};

type SseBodyError = Box<dyn Error + Send + Sync>;

pub(in crate::ingress) use bridge::bridge_sse_body;
pub(in crate::ingress) use buffered_responses::buffer_responses_sse_body;
pub(in crate::ingress) use liveness::enforce_sse_liveness;
pub(in crate::ingress) use native::validate_sse_body;
pub(in crate::ingress) use precommit::{SsePrecommitError, precommit_sse_body};

use liveness::{SseLivenessDeadline, enforce_sse_liveness_with_state};
#[cfg(test)]
use precommit::{PrecommittedSseBody, PrecommittedSseKind};

#[cfg(test)]
mod liveness_tests {
    use std::{io, time::Duration};

    use axum::body::{Body, to_bytes};
    use bytes::Bytes;
    use futures_util::{StreamExt, stream};

    use crate::{
        observability::{GatewayMetrics, RequestObservation},
        registry::UpstreamTimeoutPolicy,
    };

    use super::{
        PrecommittedSseBody, PrecommittedSseKind, SseLivenessDeadline, enforce_sse_liveness,
    };

    fn paced_body(chunks: Vec<Bytes>, delay: Duration) -> Body {
        Body::from_stream(stream::iter(chunks).then(move |chunk| async move {
            tokio::time::sleep(delay).await;
            Ok::<_, io::Error>(chunk)
        }))
    }

    fn assert_timeout(error: axum::Error) {
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains("TimedOut"), "{diagnostic}");
    }

    fn observation() -> RequestObservation {
        RequestObservation::new(GatewayMetrics::default(), tracing::Span::none())
    }

    #[tokio::test]
    async fn times_out_before_the_first_event() {
        let body = Body::from_stream(stream::pending::<Result<Bytes, io::Error>>());
        let guarded = enforce_sse_liveness(
            body,
            1024,
            Some(UpstreamTimeoutPolicy::new(Duration::from_millis(30))),
            observation(),
        );

        let error = to_bytes(guarded, 4096)
            .await
            .expect_err("a missing first event must reach the liveness deadline");

        assert_timeout(error);
    }

    #[tokio::test]
    async fn precommit_handoff_keeps_the_existing_event_idle_deadline() {
        let event = Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n",
        );
        let body = Body::from_stream(
            stream::once(async move { Ok::<_, io::Error>(event) }).chain(stream::pending()),
        );
        let mut liveness =
            SseLivenessDeadline::new(Some(UpstreamTimeoutPolicy::new(Duration::from_millis(80))));
        liveness.record_framed_event();
        tokio::time::sleep(Duration::from_millis(60)).await;
        let guarded = PrecommittedSseBody {
            body,
            liveness,
            kind: PrecommittedSseKind::Native,
        }
        .into_native_liveness_body(1024, observation());
        let mut source = guarded.into_data_stream();

        source
            .next()
            .await
            .expect("replayed event frame")
            .expect("replayed event bytes");
        let wait_started = std::time::Instant::now();
        let error = source
            .next()
            .await
            .expect("event-idle timeout body error")
            .expect_err("pending post-commit source must time out");

        assert_timeout(error);
        assert!(
            wait_started.elapsed() < Duration::from_millis(60),
            "handoff reset the event-idle deadline"
        );
    }

    #[tokio::test]
    async fn partial_chunks_do_not_reset_the_first_event_deadline() {
        let body = paced_body(
            vec![
                Bytes::from_static(b"event: response.created\n"),
                Bytes::from_static(b"data: {\"type\":\"response.created\"}\n"),
                Bytes::from_static(b"\n"),
            ],
            Duration::from_millis(40),
        );
        let guarded = enforce_sse_liveness(
            body,
            1024,
            Some(UpstreamTimeoutPolicy::new(Duration::from_millis(100))),
            observation(),
        );

        let error = to_bytes(guarded, 4096)
            .await
            .expect_err("fragmentation without a complete event must not keep the stream alive");

        assert_timeout(error);
    }

    #[tokio::test]
    async fn framed_events_reset_the_idle_deadline() {
        let chunks = vec![
            Bytes::from_static(
                b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n",
            ),
            Bytes::from_static(
                b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
            ),
        ];
        let expected = chunks.iter().fold(Vec::new(), |mut bytes, chunk| {
            bytes.extend_from_slice(chunk);
            bytes
        });
        let body = paced_body(chunks, Duration::from_millis(100));
        let guarded = enforce_sse_liveness(
            body,
            1024,
            Some(UpstreamTimeoutPolicy::new(Duration::from_millis(150))),
            observation(),
        );

        let body = to_bytes(guarded, 4096)
            .await
            .expect("each complete event must refresh the idle deadline");

        assert_eq!(body.as_ref(), expected);
    }

    #[tokio::test]
    async fn times_out_after_one_event_becomes_idle() {
        let first = stream::once(async {
            Ok::<_, io::Error>(Bytes::from_static(
                b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n",
            ))
        });
        let body = Body::from_stream(first.chain(stream::pending()));
        let guarded = enforce_sse_liveness(
            body,
            1024,
            Some(UpstreamTimeoutPolicy::new(Duration::from_millis(30))),
            observation(),
        );

        let error = to_bytes(guarded, 4096)
            .await
            .expect_err("a stream without another event must reach the idle deadline");

        assert_timeout(error);
    }
}
