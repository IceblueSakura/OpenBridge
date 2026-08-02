//! Native 与 Bridged 上游 SSE body 的增量处理。

use std::io;

use bytes::Bytes;
use futures_util::{StreamExt, stream};

use crate::{
    bridge::BridgeStreamRenderer,
    core::ApiProtocol,
    observability::RequestObservation,
    provider::{ProviderAdapter, StreamEventStatus},
    transport::sse::SseDecoder,
};

/// 增量解码上游 SSE，并用单请求 Bridge renderer 生成目标协议 event。
pub(super) fn bridge_sse_body(
    body: axum::body::Body,
    renderer: BridgeStreamRenderer,
    max_sse_event_bytes: usize,
) -> axum::body::Body {
    // 保持 source、decoder 与 renderer 同生命周期，下游 drop 会同步取消上游 body。
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            renderer,
            false,
        ),
        move |(mut source, mut decoder, mut renderer, finished)| async move {
            if finished {
                return None;
            }
            match source.as_mut().next().await {
                Some(Ok(chunk)) => {
                    let events = match decoder.push(&chunk) {
                        Ok(events) => events,
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    };
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true),
                                ));
                            }
                        }
                    }
                    Some((
                        Ok::<_, io::Error>(Bytes::from(output)),
                        (source, decoder, renderer, false),
                    ))
                }
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    (source, decoder, renderer, true),
                )),
                None => {
                    let events = match decoder.finish() {
                        Ok(events) => events,
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    };
                    let mut output = Vec::new();
                    for event in events {
                        match renderer.render(event) {
                            Ok(bytes) => output.extend_from_slice(&bytes),
                            Err(_) => {
                                return Some((
                                    Err(io::Error::other("upstream bridge stream is invalid")),
                                    (source, decoder, renderer, true),
                                ));
                            }
                        }
                    }
                    match renderer.finish() {
                        Ok(bytes) => output.extend_from_slice(&bytes),
                        Err(_) => {
                            return Some((
                                Err(io::Error::other("upstream bridge stream is invalid")),
                                (source, decoder, renderer, true),
                            ));
                        }
                    }
                    if output.is_empty() {
                        None
                    } else {
                        Some((
                            Ok::<_, io::Error>(Bytes::from(output)),
                            (source, decoder, renderer, true),
                        ))
                    }
                }
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// 在不重写原始 bytes 的前提下观察上游 SSE 生命周期。
///
/// decoder 仅用于处理跨网络 chunk 的 UTF-8/SSE framing，并委托 provider adapter 识别协议
/// terminal event。合法 EOF 但未看到 terminal 会保留已收到的 bytes 并记录 warning；无效
/// framing、无效 UTF-8 或上游 body error 则以 stream error 关闭。body 被下游丢弃时，
/// `source` 一并 drop，从而取消 reqwest 的上游字节流。
pub(super) fn validate_sse_body(
    body: axum::body::Body,
    protocol: ApiProtocol,
    adapter: ProviderAdapter,
    max_sse_event_bytes: usize,
    observation: RequestObservation,
) -> axum::body::Body {
    // 创建保持上游 source 生命周期的增量 SSE decoder。
    let stream = stream::unfold(
        (
            Box::pin(body.into_data_stream()),
            SseDecoder::new(max_sse_event_bytes),
            false,
            false,
            observation,
        ),
        move |(mut source, mut decoder, mut terminal_seen, finished, observation)| async move {
            if finished {
                return None;
            }
            // 读取下一个上游 chunk，并只观察 framing/terminal，不改写原始 bytes。
            match source.as_mut().next().await {
                Some(Ok(chunk)) => match decoder.push(&chunk) {
                    Ok(events) => {
                        match observe_sse_events(
                            adapter,
                            protocol,
                            events,
                            &mut terminal_seen,
                            &observation,
                        ) {
                            Ok(()) => Some((
                                Ok::<_, io::Error>(chunk),
                                (source, decoder, terminal_seen, false, observation),
                            )),
                            Err(()) => Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            )),
                        }
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true, observation),
                    )),
                },
                Some(Err(_)) => Some((
                    Err(io::Error::other(
                        "upstream SSE stream terminated unexpectedly",
                    )),
                    (source, decoder, terminal_seen, true, observation),
                )),
                None => match decoder.finish() {
                    Ok(events) => {
                        if observe_sse_events(
                            adapter,
                            protocol,
                            events,
                            &mut terminal_seen,
                            &observation,
                        )
                        .is_err()
                        {
                            return Some((
                                Err(io::Error::other("upstream SSE stream is invalid")),
                                (source, decoder, terminal_seen, true, observation),
                            ));
                        }
                        if !terminal_seen {
                            observation.record_stream_failure("sse_eof_before_terminal");
                            tracing::warn!(
                                ?protocol,
                                "upstream SSE stream ended before a terminal event"
                            );
                        }
                        None
                    }
                    Err(_) => Some((
                        Err(io::Error::other("upstream SSE stream is invalid")),
                        (source, decoder, terminal_seen, true, observation),
                    )),
                },
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

/// 分类一个或多个已完成 framing 的 SSE event，并更新 terminal/failure 观测。
fn observe_sse_events(
    adapter: ProviderAdapter,
    protocol: ApiProtocol,
    events: Vec<crate::transport::sse::SseEvent>,
    terminal_seen: &mut bool,
    observation: &RequestObservation,
) -> Result<(), ()> {
    // 逐个交给 Provider adapter 分类，只记录 terminal/failure，不保存事件正文。
    for event in events {
        let decoded = adapter
            .classify_sse_event(protocol, event)
            .map_err(|_| ())?;
        match decoded.status() {
            StreamEventStatus::Continue => {}
            StreamEventStatus::Completed => *terminal_seen = true,
            StreamEventStatus::Failed => {
                *terminal_seen = true;
                observation.record_stream_failure("provider_terminal_failed");
            }
        }
    }
    Ok(())
}
