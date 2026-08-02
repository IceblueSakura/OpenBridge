//! provider adapter 使用的安全 header、SSE 与错误分类契约。
//!
//! `SafeHeaders` 和 `SensitiveHeaders` 被故意分开：前者不能承载认证/host/cookie，后者
//! 只在 egress 前转换成标记为 sensitive 的 HTTP header，并在释放时清零字符串内容。

use std::{collections::HashMap, fmt};

use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{AUTHORIZATION, COOKIE, HOST, PROXY_AUTHORIZATION},
};
use zeroize::Zeroizing;

use crate::transport::sse::SseEvent;

use super::AdapterError;

/// 允许受信 Provider hook 增删改的非敏感请求头集合。
///
/// 认证、cookie、host 和 proxy authorization 等头不能通过此类型写入。
#[derive(Default)]
pub struct SafeHeaders(HeaderMap);

impl SafeHeaders {
    /// 读取一个已允许的请求头。
    pub fn get(&self, name: HeaderName) -> Option<&HeaderValue> {
        self.0.get(name)
    }

    /// 写入或替换一个普通请求头。
    ///
    /// 认证、cookie、Host 与 proxy authentication header 会被拒绝。
    pub fn insert(&mut self, name: HeaderName, value: HeaderValue) -> Result<(), AdapterError> {
        // 拒绝认证、cookie、host 和 proxy authorization，维持普通 header 的安全边界。
        if name == AUTHORIZATION || name == PROXY_AUTHORIZATION || name == COOKIE || name == HOST {
            return Err(AdapterError::SensitiveHeaderInSafeSet);
        }
        self.0.insert(name, value);
        Ok(())
    }

    /// 删除一个请求头并返回原值。
    pub fn remove(&mut self, name: HeaderName) -> Option<HeaderValue> {
        self.0.remove(name)
    }

    pub(crate) fn into_inner(self) -> HeaderMap {
        self.0
    }
}

impl fmt::Debug for SafeHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SafeHeaders")
            .field("header_names", &self.0.keys().collect::<Vec<_>>())
            .field("values", &"[OMITTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderName, HeaderValue};

    use super::*;

    #[test]
    fn safe_headers_support_regular_header_rewrite_and_drop() {
        let source = HeaderName::from_static("x-provider-source");
        let target = HeaderName::from_static("x-provider-target");
        let mut headers = SafeHeaders::default();

        headers
            .insert(source.clone(), HeaderValue::from_static("source-value"))
            .unwrap();
        headers
            .insert(
                target.clone(),
                HeaderValue::from_static("transformed-value"),
            )
            .unwrap();
        headers.remove(source.clone());

        assert!(headers.get(source).is_none());
        assert_eq!(headers.get(target).unwrap(), "transformed-value");
    }
}

/// 仅在发送到上游前附加、并在调试输出中隐藏值的敏感请求头集合。
#[derive(Default)]
pub struct SensitiveHeaders(HashMap<HeaderName, Zeroizing<String>>);

impl SensitiveHeaders {
    /// 判断集合中是否包含指定头名。
    pub fn contains(&self, name: HeaderName) -> bool {
        self.0.contains_key(&name)
    }

    #[cfg(test)]
    pub(super) fn expose(&self, name: HeaderName) -> Option<&str> {
        self.0.get(&name).map(|value| value.as_str())
    }

    pub(crate) fn insert(&mut self, name: HeaderName, value: Zeroizing<String>) {
        self.0.insert(name, value);
    }

    pub(crate) fn append_to(self, headers: &mut HeaderMap) -> Result<(), AdapterError> {
        // 将敏感字符串一次性转换为 HTTP header，并标记为 sensitive 后释放来源容器。
        for (name, value) in self.0 {
            let mut value = HeaderValue::from_str(value.as_str())
                .map_err(|_| AdapterError::InvalidAuthenticationHeader)?;
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(())
    }
}

impl fmt::Debug for SensitiveHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveHeaders")
            .field("header_names", &self.0.keys().collect::<Vec<_>>())
            .field("values", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 上游 SSE event 在 ingress 中的生命周期状态。
pub enum StreamEventStatus {
    /// event 不是终止事件，继续读取上游。
    Continue,
    /// event 表示正常完成。
    Completed,
    /// event 表示 provider 侧失败。
    Failed,
}

/// 一个已完成 framing、但尚未由 ingress 重写的 SSE event。
#[derive(Debug)]
pub struct ClassifiedSseEvent {
    event: SseEvent,
    status: StreamEventStatus,
}

impl ClassifiedSseEvent {
    pub(crate) fn new(event: SseEvent, status: StreamEventStatus) -> Self {
        Self { event, status }
    }

    /// 返回原始 SSE event。
    pub fn event(&self) -> &SseEvent {
        &self.event
    }

    /// 返回 adapter 对 event 的生命周期判定。
    pub fn status(&self) -> StreamEventStatus {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 上游 HTTP failure 的粗粒度类别。
pub enum UpstreamErrorKind {
    /// 请求内容或参数不合法。
    InvalidRequest,
    /// 上游拒绝认证。
    Authentication,
    /// 上游限流。
    RateLimited,
    /// 上游暂时不可用。
    UpstreamUnavailable,
    /// 其他上游失败。
    UpstreamFailure,
}

/// adapter 对 HTTP status 给出的重试边界，而非“总是重试”的指令。
///
/// ingress 还会叠加 streaming、attempt 上限、candidate 顺序和是否已经下游输出等条件；
/// `BeforeFirstEvent` 的含义是绝不能用于拼接已开始的 token stream。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryHint {
    /// 不允许基于该 status 自动重试。
    Never,
    /// 仅允许在尚未向下游输出第一个 event 前重试。
    BeforeFirstEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// adapter 对上游 status 给出的错误类别和重试边界。
pub struct StatusClassification {
    kind: UpstreamErrorKind,
    retry_hint: RetryHint,
}

impl StatusClassification {
    pub(crate) fn new(kind: UpstreamErrorKind, retry_hint: RetryHint) -> Self {
        Self { kind, retry_hint }
    }

    /// 返回上游错误类别。
    pub fn kind(&self) -> UpstreamErrorKind {
        self.kind
    }

    /// 返回 adapter 建议的最早重试边界。
    pub fn retry_hint(&self) -> RetryHint {
        self.retry_hint
    }
}
