//! First-event and inter-event SSE liveness deadlines.

use super::*;

pub(super) struct SseLivenessDeadline {
    deadline: Option<Instant>,
    event_idle: Option<Duration>,
    phase: TimeoutPhase,
}

impl SseLivenessDeadline {
    pub(super) fn new(policy: Option<UpstreamTimeoutPolicy>) -> Self {
        Self {
            deadline: policy.map(|policy| Instant::now() + policy.first_stream_event()),
            event_idle: policy.map(UpstreamTimeoutPolicy::stream_event_idle),
            phase: TimeoutPhase::FirstEvent,
        }
    }

    pub(super) async fn next<S>(
        &self,
        mut source: Pin<&mut S>,
    ) -> Result<Option<S::Item>, TimeoutPhase>
    where
        S: Stream + ?Sized,
    {
        match self.deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, source.next())
                .await
                .map_err(|_| self.phase),
            None => Ok(source.next().await),
        }
    }

    pub(super) fn record_framed_events(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        if let Some(event_idle) = self.event_idle {
            self.deadline = Some(Instant::now() + event_idle);
            self.phase = TimeoutPhase::EventIdle;
        }
    }

    pub(super) fn record_framed_event(&mut self) {
        self.record_framed_events(1);
    }
}

/// Applies first-event and inter-event idle deadlines without changing SSE bytes or EOF semantics.
pub(in crate::ingress) fn enforce_sse_liveness(
    body: axum::body::Body,
    max_sse_event_bytes: usize,
    policy: Option<UpstreamTimeoutPolicy>,
    observation: RequestObservation,
) -> axum::body::Body {
    let Some(policy) = policy else {
        return body;
    };

    enforce_sse_liveness_with_state(
        body,
        max_sse_event_bytes,
        SseLivenessDeadline::new(Some(policy)),
        false,
        observation,
    )
}

pub(super) fn enforce_sse_liveness_with_state(
    body: axum::body::Body,
    max_sse_event_bytes: usize,
    liveness: SseLivenessDeadline,
    skip_replayed_prefix_events: bool,
    observation: RequestObservation,
) -> axum::body::Body {
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            liveness,
            skip_replayed_prefix_events,
            false,
        ),
        move |(mut source, mut decoder, mut liveness, mut skip_replayed_events, finished)| {
            let observation = observation.clone();
            async move {
                if finished {
                    return None;
                }
                let next = match liveness.next(source.as_mut()).await {
                    Ok(next) => next,
                    Err(phase) => {
                        observation.record_stream_timeout(phase);
                        return Some((
                            Err::<Bytes, SseBodyError>(Box::new(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "upstream SSE liveness deadline elapsed",
                            ))),
                            (source, decoder, liveness, skip_replayed_events, true),
                        ));
                    }
                };
                match next {
                    Some(Ok(chunk)) => {
                        let events = match decoder.push(&chunk) {
                            Ok(events) => events,
                            Err(_) => {
                                return Some((
                                    Err::<Bytes, SseBodyError>(Box::new(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "upstream SSE stream is invalid",
                                    ))),
                                    (source, decoder, liveness, skip_replayed_events, true),
                                ));
                            }
                        };
                        if skip_replayed_events {
                            skip_replayed_events = false;
                        } else {
                            liveness.record_framed_events(events.len());
                        }
                        Some((
                            Ok(chunk),
                            (source, decoder, liveness, skip_replayed_events, false),
                        ))
                    }
                    Some(Err(error)) => {
                        if is_timeout_error(&error) {
                            observation.record_stream_timeout(TimeoutPhase::StreamTotal);
                        }
                        Some((
                            Err::<Bytes, SseBodyError>(Box::new(error)),
                            (source, decoder, liveness, skip_replayed_events, true),
                        ))
                    }
                    None => None,
                }
            }
        },
    );
    axum::body::Body::from_stream(stream)
}
