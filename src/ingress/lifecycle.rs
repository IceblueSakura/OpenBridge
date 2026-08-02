//! 下游 response body 的取消、终态与 usage 观测生命周期。

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use axum::response::Response;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http_body::{Body as HttpBody, Frame, SizeHint};

use crate::observability::{RequestObservation, UsageCapture};

/// 在 response body 建立前捕获 middleware future 被取消的请求。
pub(super) struct RequestLifecycleGuard {
    observation: Option<RequestObservation>,
}

impl RequestLifecycleGuard {
    /// 创建仍由 request future 负责的生命周期 guard。
    pub(super) fn new(observation: RequestObservation) -> Self {
        Self {
            observation: Some(observation),
        }
    }

    /// response body wrapper 建立后移交取消和终态责任。
    pub(super) fn handoff_to_body(&mut self) {
        self.observation.take();
    }
}

impl Drop for RequestLifecycleGuard {
    fn drop(&mut self) {
        // pending send、backoff 或 handler 阶段被取消时尚无 body wrapper，必须在这里收口。
        if let Some(observation) = self.observation.take() {
            observation.cancel();
        }
    }
}

/// 用不改写字节的外层 stream 在真实 EOF、错误或 drop 时结束请求观测。
pub(super) fn observe_response_body(
    response: &mut Response,
    observation: RequestObservation,
    max_json_body_bytes: usize,
    max_sse_event_bytes: usize,
) {
    // 只为成功 JSON/SSE response 创建有界 usage 解析器。
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let usage = if response.status().is_success() {
        UsageCapture::for_response(content_type, max_json_body_bytes, max_sse_event_bytes)
    } else {
        UsageCapture::None
    };
    let body = std::mem::replace(response.body_mut(), axum::body::Body::empty());
    *response.body_mut() =
        axum::body::Body::new(RequestBodyObserver::new(body, observation, usage));
}

/// 保留原始 HTTP frame，并在 body 的实际消费边界提交请求终态。
struct RequestBodyObserver {
    body: axum::body::Body,
    observation: RequestObservation,
    usage: UsageCapture,
    finished: bool,
}

impl RequestBodyObserver {
    /// 创建尚未产生首字节或终态的透明 body wrapper。
    fn new(body: axum::body::Body, observation: RequestObservation, usage: UsageCapture) -> Self {
        Self {
            body,
            observation,
            usage,
            finished: false,
        }
    }

    fn complete(&mut self) {
        // 正常 EOF 先提交最后一个 usage event，再提交请求终态。
        if self.finished {
            return;
        }
        self.usage.finish(&self.observation);
        self.observation.finish();
        self.finished = true;
    }

    fn fail(&mut self, kind: &'static str) {
        // body error 已是最终可见边界，不能等待下一次 poll 才记录。
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

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let observer = self.get_mut();
        // 保留所有 data/trailer frame，只在 data frame 上观察首字节和 usage。
        match Pin::new(&mut observer.body).poll_frame(context) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    if !chunk.is_empty() {
                        observer.observation.record_first_body_byte();
                    }
                    observer.usage.observe_chunk(&observer.observation, chunk);
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

    fn is_end_stream(&self) -> bool {
        // 外层只有在提交 EOF 或错误后才能报告结束，否则 Hyper 可能跳过最终 poll 并把完整 body 误记为取消。
        self.finished
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

impl Drop for RequestBodyObserver {
    fn drop(&mut self) {
        // 未到 EOF 且未产生 body error 表示下游不再消费 response。
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
    use crate::observability::{GatewayMetrics, RequestObservation, UsageCapture};

    #[test]
    fn complete_single_frame_body_waits_for_observer_eof_before_reporting_end_stream() {
        // 构造会在首帧后立即报告底层 end-stream 的完整内存 body。
        let metrics = GatewayMetrics::default();
        let observation = RequestObservation::new(metrics.clone(), tracing::Span::none());
        observation.record_response_ready(http::StatusCode::OK);
        let mut observer = pin!(RequestBodyObserver::new(
            Body::from("complete"),
            observation,
            UsageCapture::None,
        ));
        let mut context = Context::from_waker(noop_waker_ref());

        // 消费唯一 data frame 后，外层仍需等待一次 EOF poll 才能提交 completed。
        assert!(matches!(
            observer.as_mut().poll_frame(&mut context),
            std::task::Poll::Ready(Some(Ok(_)))
        ));
        assert!(!observer.is_end_stream());
        assert!(matches!(
            observer.as_mut().poll_frame(&mut context),
            std::task::Poll::Ready(None)
        ));
        drop(observer);

        // 正常 EOF 只能累计 completed，不能在 Drop 中误记 cancelled。
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.requests_completed, 1);
        assert_eq!(snapshot.requests_cancelled, 0);
    }
}
