//! One acknowledged frame in flight. The worker owns the upstream/deadline;
//! HTTP body polling owns visibility acknowledgement, and drop cancels the worker.
#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
use super::{ApiError, BoundEntry, Limits};
use crate::{
    adapter::Request,
    execution::{Attempt, ResponseDelivery},
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
async fn send_frames(
    tx: &mpsc::Sender<Message>,
    delivery: &mut ResponseDelivery,
    frames: Vec<Bytes>,
) -> Result<(), ApiError> {
    for bytes in frames {
        let (ack, seen) = oneshot::channel();
        tx.send(Message::Chunk(Chunk { bytes, ack }))
            .await
            .map_err(|_| ApiError::upstream())?;
        seen.await.map_err(|_| ApiError::upstream())?;
        // The HTTP body has handed this frame to server transport. Queueing alone
        // cannot reach this point. Peer receipt is not observable at this layer.
        delivery.commit().map_err(|_| ApiError::upstream())?;
    }
    Ok(())
}
async fn produce(
    mut upstream: reqwest::Response,
    attempt: &mut Attempt,
    delivery: &mut ResponseDelivery,
    tx: &mpsc::Sender<Message>,
    stream: bool,
    limit: usize,
) -> Result<(), ApiError> {
    let mut used_bytes = 0usize;
    while let Some(chunk) = upstream.chunk().await.map_err(|e| {
        if e.is_timeout() {
            ApiError::timeout()
        } else {
            ApiError::upstream()
        }
    })? {
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
                let frames = delivery
                    .encode_events(attempt, &events)
                    .map_err(|_| ApiError::upstream())?;
                send_frames(tx, delivery, frames).await?;
            }
        }
    }
    attempt.finish().map_err(|_| ApiError::upstream())?;
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
    send_frames(tx, delivery, frames).await?;
    delivery
        .complete(attempt)
        .map_err(|_| ApiError::upstream())?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn respond(
    entry: Arc<BoundEntry>,
    request: Request,
    upstream: reqwest::Response,
    limits: &Limits,
    deadline: Instant,
    mut shutdown: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
) -> Result<Response, ApiError> {
    let status = upstream.status().as_u16();
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
        .min(entry.endpoint.execution.response_body_limit);
    let sse = SseLimits {
        max_event_bytes: limits.event_bytes.min(limit),
        max_wire_bytes: limit,
        max_events: limits.max_events,
        max_obfuscation_bytes: (limit / 4).min(4 << 20),
    };
    let mut attempt = Attempt::new(entry.endpoint.adapter(), limit, sse);
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
    let (tx, mut rx) = mpsc::channel(1);
    let state = Arc::new(Status::default());
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        let _permit = permit;
        let stopped = *shutdown.borrow();
        let result = if stopped {
            Err(ApiError::shutdown())
        } else {
            tokio::select! {
                biased;
                _=shutdown.changed()=>Err(ApiError::shutdown()),
                _=tokio::time::sleep_until(deadline)=>Err(ApiError::timeout()),
                _=tx.closed()=>Err(ApiError::upstream()),
                result=produce(upstream,&mut attempt,&mut delivery,&tx,stream,limit)=>result,
            }
        };
        match result {
            Ok(()) => worker_state.complete.store(true, Ordering::Release),
            Err(error) => {
                attempt.cancel();
                delivery.cancel();
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
