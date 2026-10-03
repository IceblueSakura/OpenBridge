//! One acknowledged frame in flight. A chain owns one permit/deadline/trace;
//! each candidate owns a fresh decoder/delivery, and visibility forbids advancement.
#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
#[cfg(test)]
use super::BoundCandidate;
use super::{
    ApiError, BoundEntry, Limits,
    diagnostics::{Outcome, Stage, Trace},
    exchange::{Upstreams, produce_chain},
};
use crate::{adapter::Request, execution::ResponseDelivery};
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
};
use tokio::{
    sync::{OwnedSemaphorePermit, mpsc, oneshot},
    task::AbortHandle,
    time::Instant,
};
pub(super) struct Chunk {
    bytes: Bytes,
    ack: oneshot::Sender<()>,
}
pub(super) enum Message {
    Chunk(Chunk),
    Error(ApiError),
}
#[derive(Default)]
pub(super) struct Status {
    failed: AtomicBool,
    complete: AtomicBool,
    pub(super) published: AtomicBool,
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
pub(super) struct Lane<'a> {
    pub(super) tx: &'a mpsc::Sender<Message>,
    pub(super) state: &'a Status,
}
pub(super) async fn send_frames(
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
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn respond(
    entry: Arc<BoundEntry>,
    candidate: Arc<BoundCandidate>,
    request: Request,
    upstream: reqwest::Response,
    limits: &Limits,
    deadline: Instant,
    shutdown: tokio_util::sync::CancellationToken,
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
pub(super) async fn respond_source(
    entry: Arc<BoundEntry>,
    request: Request,
    source: Upstreams,
    limits: Limits,
    deadline: Instant,
    shutdown: tokio_util::sync::CancellationToken,
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
        let result = tokio::select! {
            biased;
            _=shutdown.cancelled()=>Err(ApiError::shutdown()),
            _=tokio::time::sleep_until(deadline)=>Err(ApiError::timeout()),
            _=tx.closed()=>Err(ApiError::upstream()),
            result=produce_chain(source,&entry,&request,&limits,deadline,&tx,&worker_state,&mut trace)=>result,
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
