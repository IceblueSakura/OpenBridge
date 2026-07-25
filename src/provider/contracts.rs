//! provider adapter 使用的安全 header、SSE 与错误分类契约。
//!
//! `SafeHeaders` 和 `SensitiveHeaders` 被故意分开：前者不能承载认证/host/cookie，后者
//! 只在 egress 前转换成标记为 sensitive 的 HTTP header，并在释放时清零字符串内容。

use std::{collections::HashMap, fmt};

use http::{
    HeaderMap, HeaderName, HeaderValue, StatusCode,
    header::{AUTHORIZATION, COOKIE, HOST, PROXY_AUTHORIZATION},
};
use zeroize::Zeroizing;

use crate::{
    core::{CapabilitySet, Protocol},
    transport::sse::SseEvent,
};

use super::{CredentialLease, ProviderFailure};

/// 允许 provider adapter 添加的非敏感请求头集合。
///
/// 认证、cookie、host 和 proxy authorization 等头不能通过此类型写入。
#[derive(Default)]
pub struct SafeHeaders(HeaderMap);

impl SafeHeaders {
    /// 读取一个已允许的请求头。
    pub fn get(&self, name: HeaderName) -> Option<&HeaderValue> {
        self.0.get(name)
    }

    pub(crate) fn insert(
        &mut self,
        name: HeaderName,
        value: HeaderValue,
    ) -> Result<(), ProviderFailure> {
        if name == AUTHORIZATION || name == PROXY_AUTHORIZATION || name == COOKIE || name == HOST {
            return Err(ProviderFailure::SensitiveHeaderInSafeSet);
        }
        self.0.insert(name, value);
        Ok(())
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

    pub(crate) fn append_to(self, headers: &mut HeaderMap) -> Result<(), ProviderFailure> {
        for (name, value) in self.0 {
            let mut value = HeaderValue::from_str(value.as_str())
                .map_err(|_| ProviderFailure::InvalidAuthenticationHeader)?;
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

/// 构造非敏感上游请求头的 provider adapter 契约。
pub trait HeaderAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure>;
}

/// 从 credential lease 构造敏感认证头的 provider adapter 契约。
pub trait AuthAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventDisposition {
    /// event 不是终止事件，继续读取上游。
    Continue,
    /// event 表示正常完成。
    Completed,
    /// event 表示 provider 侧失败。
    Failed,
}

/// 一个已完成 framing、但尚未由 ingress 重写的 SSE event。
#[derive(Debug)]
pub struct DecodedEvent {
    event: SseEvent,
    disposition: EventDisposition,
}

impl DecodedEvent {
    pub(crate) fn new(event: SseEvent, disposition: EventDisposition) -> Self {
        Self { event, disposition }
    }

    /// 返回原始 SSE event。
    pub fn event(&self) -> &SseEvent {
        &self.event
    }

    /// 返回 adapter 对 event 的生命周期判定。
    pub fn disposition(&self) -> EventDisposition {
        self.disposition
    }
}

/// 解码 provider-specific SSE 生命周期的 adapter 契约。
pub trait ResponseAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderErrorClass {
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
    Never,
    BeforeFirstEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassifiedProviderError {
    class: ProviderErrorClass,
    retry_hint: RetryHint,
}

impl ClassifiedProviderError {
    pub(crate) fn new(class: ProviderErrorClass, retry_hint: RetryHint) -> Self {
        Self { class, retry_hint }
    }

    pub fn class(&self) -> ProviderErrorClass {
        self.class
    }

    pub fn retry_hint(&self) -> RetryHint {
        self.retry_hint
    }
}

/// 将上游 HTTP status 映射为稳定的 provider 错误类别。
pub trait ErrorAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError;
}

/// 检查请求所需能力是否在 provider adapter 的实现上界内。
pub trait CapabilityAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure>;
}
