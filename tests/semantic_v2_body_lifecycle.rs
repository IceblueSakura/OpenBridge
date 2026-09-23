//! Bounded loopback body lifecycle; no OpenBridge service, Provider, or private config.
#[path = "support/responses_profile.rs"]
mod wire;
use axum::{
    Router,
    body::{Body, Bytes},
    http::header::CONTENT_TYPE,
    routing::get,
};
use futures_util::{StreamExt, stream};
use openbridge::protocol::openai::sse::{ResponsesSseDecoder, SseLimits, encode_frame};
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};

const FIRST: &[u8] = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n";

fn body(rx: mpsc::Receiver<Bytes>) -> Body {
    Body::from_stream(stream::unfold(rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|frame| (Ok::<_, Infallible>(frame), rx))
    }))
}

struct DropSignal(Option<oneshot::Sender<()>>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        if let Some(done) = self.0.take() {
            let _ = done.send(());
        }
    }
}

#[tokio::test]
async fn first_frame_is_readable_without_waiting_for_terminal_and_cancellation_drops_producer() {
    let (release, gate) = oneshot::channel::<()>();
    let (closed, dropped) = oneshot::channel::<()>();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let closed = Arc::new(Mutex::new(Some(closed)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/stream", get(move || {
            let gate = gate.clone();
            let closed = closed.clone();
            async move {
                let gate = gate.lock().unwrap().take().unwrap();
                let closed = closed.lock().unwrap().take().unwrap();
                let (tx, rx) = mpsc::channel(1);
                tokio::spawn(async move {
                    let _signal = DropSignal(Some(closed));
                    if tx.send(Bytes::from_static(FIRST)).await.is_err() { return; }
                    // A closed body must unblock the producer even if the next event is gated.
                    tokio::select! {
                        _ = gate => { let _ = tx.send(Bytes::from_static(b"data: forbidden\n\n")).await; },
                        _ = tx.closed() => {},
                    }
                });
                ([(CONTENT_TYPE, "text/event-stream")], body(rx))
            }
        }))).await.unwrap();
    });
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut response = tokio::time::timeout(
        Duration::from_secs(3),
        client.get(format!("http://{address}/stream")).send(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), 200);
    let first = tokio::time::timeout(Duration::from_secs(3), response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(&first[..], FIRST);
    // The producer is blocked before the second frame; EOF is not fabricated.
    assert!(
        tokio::time::timeout(Duration::from_millis(100), response.chunk())
            .await
            .is_err()
    );
    drop(response);
    tokio::time::timeout(Duration::from_secs(3), dropped)
        .await
        .unwrap()
        .unwrap();
    assert!(release.send(()).is_err());
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn slow_body_consumer_bounds_prefetch_to_one_queued_frame() {
    let (tx, rx) = mpsc::channel(1);
    let (queued, ready) = oneshot::channel();
    let (closed, dropped) = oneshot::channel();
    let sent = Arc::new(AtomicUsize::new(0));
    let progress = sent.clone();
    let producer = tokio::spawn(async move {
        let _signal = DropSignal(Some(closed));
        tx.send(Bytes::from_static(FIRST)).await.unwrap();
        progress.fetch_add(1, Ordering::SeqCst);
        tx.send(Bytes::from_static(FIRST)).await.unwrap();
        progress.fetch_add(1, Ordering::SeqCst);
        let _ = queued.send(());
        assert!(tx.send(Bytes::from_static(FIRST)).await.is_err());
    });
    let body = body(rx);
    let mut stream = body.into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first.as_ref(), FIRST);
    tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sent.load(Ordering::SeqCst), 2);
    drop(stream);
    tokio::time::timeout(Duration::from_secs(3), dropped)
        .await
        .unwrap()
        .unwrap();
    producer.await.unwrap();
}

#[tokio::test]
async fn malformed_or_oversize_frame_after_commit_cannot_become_success() {
    let initial = encode_frame(&wire::events(2)[0], SseLimits::default().max_event_bytes).unwrap();
    let terminal = encode_frame(
        wire::events(2).last().unwrap(),
        SseLimits::default().max_event_bytes,
    )
    .unwrap();
    for oversize in [false, true] {
        let limits = SseLimits {
            max_event_bytes: initial.len() + 4,
            ..SseLimits::default()
        };
        let bad = if oversize {
            Bytes::from(format!("data: {}\n\n", "x".repeat(limits.max_event_bytes)))
        } else {
            Bytes::from_static(b"event: response.completed\ndata: invalid-json\n\n")
        };
        let first = initial.clone();
        let last = terminal.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/broken",
                    get(move || {
                        let frames = [first.clone(), bad.clone(), last.clone()];
                        async move {
                            let (tx, rx) = mpsc::channel(1);
                            tokio::spawn(async move {
                                for frame in frames {
                                    if tx.send(frame).await.is_err() {
                                        return;
                                    }
                                }
                            });
                            ([(CONTENT_TYPE, "text/event-stream")], body(rx))
                        }
                    }),
                ),
            )
            .await
            .unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut response = tokio::time::timeout(
            Duration::from_secs(3),
            client.get(format!("http://{address}/broken")).send(),
        )
        .await
        .unwrap()
        .unwrap();
        let mut decoder = ResponsesSseDecoder::new(200, "text/event-stream", limits, None).unwrap();
        let mut started = 0;
        let mut rejected = false;
        while let Some(chunk) = tokio::time::timeout(Duration::from_secs(3), response.chunk())
            .await
            .unwrap()
            .unwrap()
        {
            let mut rest = chunk.as_ref();
            while !rest.is_empty() {
                match decoder.consume(rest) {
                    Ok((n, events)) => {
                        started += events.len();
                        rest = &rest[n..];
                    }
                    Err(_) => {
                        rejected = true;
                        break;
                    }
                }
            }
            if rejected {
                break;
            }
        }
        assert_eq!(started, 1);
        assert!(rejected);
        assert!(decoder.finish().is_err());
        assert!(decoder.materialize().is_err());
        drop(response);
        server.abort();
        let _ = server.await;
    }
}

#[tokio::test]
async fn truncated_http_body_never_becomes_a_successful_responses_terminal() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/truncated",
                get(|| async {
                    let (tx, rx) = mpsc::channel(1);
                    tokio::spawn(async move {
                        let frame =
                            encode_frame(&wire::events(2)[0], SseLimits::default().max_event_bytes)
                                .unwrap();
                        let _ = tx.send(frame).await;
                    });
                    ([(CONTENT_TYPE, "text/event-stream")], body(rx))
                }),
            ),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut response = tokio::time::timeout(
        Duration::from_secs(3),
        client.get(format!("http://{address}/truncated")).send(),
    )
    .await
    .unwrap()
    .unwrap();
    let mut decoder = ResponsesSseDecoder::new(
        response.status().as_u16(),
        response.headers()[CONTENT_TYPE].to_str().unwrap(),
        SseLimits::default(),
        None,
    )
    .unwrap();
    let mut started = 0;
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(3), response.chunk())
        .await
        .unwrap()
        .unwrap()
    {
        let mut rest = chunk.as_ref();
        while !rest.is_empty() {
            let (n, events) = decoder.consume(rest).unwrap();
            started += events.len();
            rest = &rest[n..];
        }
    }
    assert_eq!(started, 1);
    assert!(decoder.finish().is_err());
    assert!(decoder.materialize().is_err());
    server.abort();
    let _ = server.await;
}
