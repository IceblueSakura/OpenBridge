//! One acknowledged frame in flight. A chain owns one permit/deadline/trace;
//! each candidate owns a fresh decoder/delivery, and visibility forbids advancement.
#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
use super::{
    ApiError, BoundCandidate, BoundEntry, Limits, Runtime,
    diagnostics::{Outcome, Stage, Trace},
};
use crate::{
    adapter::Request,
    execution::{Attempt, DeliveryState, ResponseDelivery, fallback::may_advance},
    protocol::openai::sse::{Obfuscation, SseLimits},
};
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use futures_util::Stream;
use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, mpsc, oneshot, watch},
    task::AbortHandle,
    time::Instant,
};
struct Chunk {
    bytes: Bytes,
    ack: oneshot::Sender<()>,
}
enum Message {
    Chunk(Chunk),
    Error(ApiError),
}
#[derive(Default)]
struct Status {
    failed: AtomicBool,
    complete: AtomicBool,
    published: AtomicBool,
}
struct Output {
    first: Option<Message>,
    rx: mpsc::Receiver<Message>,
    status: Arc<Status>,
    abort: AbortHandle,
    ended: bool,
}
impl Stream for Output {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.ended {
            return Poll::Ready(None);
        }
        if self.status.failed.load(Ordering::Acquire) {
            self.ended = true;
            return Poll::Ready(Some(Err(std::io::Error::other("upstream stream failed"))));
        }
        let item = if let Some(first) = self.first.take() {
            Poll::Ready(Some(first))
        } else {
            self.rx.poll_recv(cx)
        };
        match item {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Message::Chunk(chunk))) => {
                if chunk.ack.send(()).is_err() {
                    self.ended = true;
                    Poll::Ready(Some(Err(std::io::Error::other("delivery cancelled"))))
                } else {
                    Poll::Ready(Some(Ok(chunk.bytes)))
                }
            }
            Poll::Ready(Some(Message::Error(_))) => {
                self.ended = true;
                Poll::Ready(Some(Err(std::io::Error::other("upstream stream failed"))))
            }
            Poll::Ready(None) => {
                self.ended = true;
                if self.status.complete.load(Ordering::Acquire) {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Err(std::io::Error::other("unfinished delivery"))))
                }
            }
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.abort.abort();
    }
}
struct Lane<'a> {
    tx: &'a mpsc::Sender<Message>,
    state: &'a Status,
}
async fn send_frames(
    lane: &Lane<'_>,
    delivery: &mut ResponseDelivery,
    frames: Vec<Bytes>,
    trace: &mut Trace,
) -> Result<(), ApiError> {
    trace.stage(Stage::Delivery);
    for bytes in frames {
        let size = bytes.len();
        let (ack, seen) = oneshot::channel();
        // Freeze advancement before publication, closing the recv/timeout race.
        // This is not delivery commit; handoff acknowledgement still owns commit.
        lane.state.published.store(true, Ordering::Release);
        lane.tx
            .send(Message::Chunk(Chunk { bytes, ack }))
            .await
            .map_err(|_| ApiError::upstream())?;
        seen.await.map_err(|_| ApiError::upstream())?;
        // Only HTTP body handoff acknowledges commit; queueing cannot do so.
        delivery.commit().map_err(|_| ApiError::upstream())?;
        trace.handed_off(size);
    }
    Ok(())
}
async fn produce(
    mut upstream: reqwest::Response,
    attempt: &mut Attempt,
    delivery: &mut ResponseDelivery,
    lane: &Lane<'_>,
    stream: bool,
    limit: usize,
    trace: &mut Trace,
) -> Result<(), ApiError> {
    let mut used_bytes = 0usize;
    while let Some(chunk) = upstream.chunk().await.map_err(|e| {
        ApiError::transport(if e.is_timeout() {
            crate::provider::ErrorClass::Timeout
        } else {
            crate::provider::ErrorClass::Upstream
        })
    })? {
        trace.stage(Stage::Intake);
        trace.received(chunk.len());
        used_bytes = used_bytes.saturating_add(chunk.len());
        if used_bytes > limit {
            return Err(ApiError::upstream());
        }
        let mut rest = chunk.as_ref();
        while !rest.is_empty() {
            let (used, events) = attempt.push(rest).map_err(|_| ApiError::upstream())?;
            if used == 0 {
                return Err(ApiError::upstream());
            }
            rest = &rest[used..];
            if stream {
                trace.stage(Stage::Projection);
                let frames = delivery
                    .encode_events(attempt, &events)
                    .map_err(|_| ApiError::upstream())?;
                send_frames(lane, delivery, frames, trace).await?;
                trace.stage(Stage::Intake);
            }
        }
    }
    trace.stage(Stage::Terminal);
    attempt.finish().map_err(|_| ApiError::upstream())?;
    trace.stage(Stage::Projection);
    let frames = if stream {
        delivery
            .finish_stream(attempt)
            .map_err(|_| ApiError::upstream())?
    } else {
        let bytes = delivery
            .encode_json(attempt)
            .map_err(|_| ApiError::upstream())?;
        if bytes.len() > limit {
            return Err(ApiError::upstream());
        }
        vec![Bytes::from(bytes)]
    };
    send_frames(lane, delivery, frames, trace).await?;
    delivery
        .complete(attempt)
        .map_err(|_| ApiError::upstream())?;
    Ok(())
}
async fn produce_candidate(
    entry: &BoundEntry,
    candidate: &BoundCandidate,
    request: &Request,
    upstream: reqwest::Response,
    limits: &Limits,
    lane: &Lane<'_>,
    trace: &mut Trace,
) -> Result<(), ApiError> {
    trace.stage(Stage::ResponseHead);
    let status = upstream.status().as_u16();
    trace.head(status, upstream.headers());
    if !(200..300).contains(&status) {
        return Err(ApiError::status(status));
    }
    let content_type = upstream
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let media: mime::Mime = content_type.parse().map_err(|_| ApiError::upstream())?;
    let stream = request.delivery.streaming();
    let is_stream = media.type_() == mime::TEXT && media.subtype() == "event-stream";
    let is_json = media.type_() == mime::APPLICATION
        && (media.subtype() == mime::JSON || media.suffix() == Some(mime::JSON));
    if (stream && !is_stream) || (!stream && !is_json) {
        return Err(ApiError::upstream());
    }
    let limit = limits
        .response_bytes
        .min(candidate.endpoint.execution.response_body_limit);
    let sse = SseLimits {
        max_event_bytes: limits.event_bytes.min(limit),
        max_wire_bytes: limit,
        max_events: limits.max_events,
        max_obfuscation_bytes: (limit / 4).min(4 << 20),
    };
    let mut attempt = Attempt::new(candidate.endpoint.adapter(), limit, sse);
    attempt
        .begin(status, content_type)
        .map_err(|_| ApiError::upstream())?;
    let options = request
        .delivery
        .options
        .value()
        .cloned()
        .unwrap_or_default();
    let padding = if stream && options.obfuscation() {
        let mut seed = [0; 32];
        getrandom::fill(&mut seed).map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "entropy_unavailable",
            )
        })?;
        Obfuscation::Seeded(seed)
    } else {
        Obfuscation::Disabled
    };
    let mut delivery = ResponseDelivery::new(
        entry.client.clone(),
        entry.downstream.clone(),
        entry.public.id.as_str(),
        sse,
        options,
        padding,
    );
    let result = produce(
        upstream,
        &mut attempt,
        &mut delivery,
        lane,
        stream,
        limit,
        trace,
    )
    .await;
    if result.is_err() {
        attempt.cancel();
        delivery.cancel();
    }
    result
}
enum Upstreams {
    Http {
        runtime: Arc<Runtime>,
        candidates: Vec<Arc<BoundCandidate>>,
    },
    #[cfg(test)]
    Observed {
        candidate: Arc<BoundCandidate>,
        response: reqwest::Response,
    },
}
#[allow(clippy::too_many_arguments)]
async fn produce_chain(
    source: Upstreams,
    entry: &BoundEntry,
    request: &Request,
    limits: &Limits,
    deadline: Instant,
    tx: &mpsc::Sender<Message>,
    state: &Status,
    trace: &mut Trace,
) -> Result<(), ApiError> {
    let lane = Lane { tx, state };
    let (runtime, candidates, mut observed) = match source {
        Upstreams::Http {
            runtime,
            candidates,
        } => (Some(runtime), candidates, None),
        #[cfg(test)]
        Upstreams::Observed {
            candidate,
            response,
        } => (None, vec![candidate], Some(response)),
    };
    for (index, candidate) in candidates
        .iter()
        .enumerate()
        .take(entry.policy.max_attempts)
    {
        if Instant::now() >= deadline {
            return Err(ApiError::timeout());
        }
        let attempt_deadline = deadline
            .min(Instant::now() + Duration::from_millis(candidate.endpoint.execution.timeout_ms));
        let result = tokio::time::timeout_at(attempt_deadline, async {
            let upstream = if let Some(response) = observed.take() {
                response
            } else {
                let runtime = runtime.as_ref().ok_or(ApiError::upstream())?;
                trace.stage(Stage::Prepare);
                let prepared = crate::execution::prepare(
                    &candidate.endpoint,
                    &candidate.provider,
                    &candidate.secret,
                    request,
                )
                .map_err(|_| ApiError::invalid())?;
                trace.stage(Stage::Connect);
                runtime
                    .transport
                    .send(
                        prepared,
                        attempt_deadline.saturating_duration_since(Instant::now()),
                    )
                    .await
                    .map_err(ApiError::transport)?
            };
            produce_candidate(entry, candidate, request, upstream, limits, &lane, trace).await
        })
        .await
        .unwrap_or_else(|_| Err(ApiError::timeout()));
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                let visibility = if state.published.load(Ordering::Acquire) {
                    DeliveryState::Committed
                } else {
                    DeliveryState::Uncommitted
                };
                let advance = error.fallback.is_some_and(|class| {
                    may_advance(
                        &entry.policy,
                        visibility,
                        class,
                        index + 1,
                        Instant::now() < deadline,
                    )
                });
                if !advance || index + 1 == candidates.len() {
                    return Err(error);
                }
            }
        }
    }
    Err(ApiError::upstream())
}
pub(super) async fn exchange(
    runtime: Arc<Runtime>,
    entry: Arc<BoundEntry>,
    request: Request,
    deadline: Instant,
    permit: OwnedSemaphorePermit,
    trace: Trace,
) -> Result<Response, ApiError> {
    let candidates = entry.eligible(&request)?;
    let limits = runtime.limits.clone();
    let shutdown = runtime.shutdown.subscribe();
    respond_source(
        entry,
        request,
        Upstreams::Http {
            runtime,
            candidates,
        },
        limits,
        deadline,
        shutdown,
        permit,
        trace,
    )
    .await
}
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn respond(
    entry: Arc<BoundEntry>,
    candidate: Arc<BoundCandidate>,
    request: Request,
    upstream: reqwest::Response,
    limits: &Limits,
    deadline: Instant,
    shutdown: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
    trace: Trace,
) -> Result<Response, ApiError> {
    respond_source(
        entry,
        request,
        Upstreams::Observed {
            candidate,
            response: upstream,
        },
        limits.clone(),
        deadline,
        shutdown,
        permit,
        trace,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
async fn respond_source(
    entry: Arc<BoundEntry>,
    request: Request,
    source: Upstreams,
    limits: Limits,
    deadline: Instant,
    mut shutdown: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
    mut trace: Trace,
) -> Result<Response, ApiError> {
    let stream = request.delivery.streaming();
    let (tx, mut rx) = mpsc::channel(1);
    let state = Arc::new(Status::default());
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        trace.outcome(Outcome::Interrupted);
        let _permit = permit;
        let result = if *shutdown.borrow() {
            Err(ApiError::shutdown())
        } else {
            tokio::select! {
                biased;
                _=shutdown.changed()=>Err(ApiError::shutdown()),
                _=tokio::time::sleep_until(deadline)=>Err(ApiError::timeout()),
                _=tx.closed()=>Err(ApiError::upstream()),
                result=produce_chain(source,&entry,&request,&limits,deadline,&tx,&worker_state,&mut trace)=>result,
            }
        };
        match result {
            Ok(()) => {
                trace.stage(Stage::Complete);
                trace.outcome(Outcome::Complete);
                worker_state.complete.store(true, Ordering::Release);
            }
            Err(error) => {
                trace.outcome(match error.code {
                    "upstream_timeout" => Outcome::Timeout,
                    "shutting_down" => Outcome::Shutdown,
                    _ => Outcome::Error,
                });
                worker_state.failed.store(true, Ordering::Release);
                let _ = tx.try_send(Message::Error(error));
            }
        }
    });
    let abort = worker.abort_handle();
    let first = match rx.recv().await {
        Some(Message::Chunk(chunk)) if !state.failed.load(Ordering::Acquire) => {
            Message::Chunk(chunk)
        }
        Some(Message::Error(error)) => return Err(error),
        _ => return Err(ApiError::upstream()),
    };
    // Conservatively freeze the candidate when HTTP response ownership leaves
    // this layer; encoding/queueing alone still never calls delivery.commit().
    state.published.store(true, Ordering::Release);
    let body = Body::from_stream(Output {
        first: Some(first),
        rx,
        status: state,
        abort,
        ended: false,
    });
    Response::builder()
        .header(
            "content-type",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(body)
        .map_err(|_| ApiError::upstream())
}
