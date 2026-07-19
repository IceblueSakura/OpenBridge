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

#[derive(Default)]
pub struct SafeHeaders(HeaderMap);

impl SafeHeaders {
    pub fn get(&self, name: HeaderName) -> Option<&HeaderValue> {
        self.0.get(name)
    }

    pub(super) fn insert(
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

    pub(super) fn into_inner(self) -> HeaderMap {
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

#[derive(Default)]
pub struct SensitiveHeaders(HashMap<HeaderName, Zeroizing<String>>);

impl SensitiveHeaders {
    pub fn contains(&self, name: HeaderName) -> bool {
        self.0.contains_key(&name)
    }

    #[cfg(test)]
    pub(super) fn expose(&self, name: HeaderName) -> Option<&str> {
        self.0.get(&name).map(|value| value.as_str())
    }

    pub(super) fn insert(&mut self, name: HeaderName, value: Zeroizing<String>) {
        self.0.insert(name, value);
    }

    pub(super) fn append_to(self, headers: &mut HeaderMap) -> Result<(), ProviderFailure> {
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

pub trait HeaderAdapter {
    fn build_headers(&self) -> Result<SafeHeaders, ProviderFailure>;
}

pub trait AuthAdapter {
    fn build_auth_headers(
        &self,
        credential: &CredentialLease,
    ) -> Result<SensitiveHeaders, ProviderFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventDisposition {
    Continue,
    Completed,
    Failed,
}

#[derive(Debug)]
pub struct DecodedEvent {
    event: SseEvent,
    disposition: EventDisposition,
}

impl DecodedEvent {
    pub(super) fn new(event: SseEvent, disposition: EventDisposition) -> Self {
        Self { event, disposition }
    }

    pub fn event(&self) -> &SseEvent {
        &self.event
    }

    pub fn disposition(&self) -> EventDisposition {
        self.disposition
    }
}

pub trait ResponseAdapter {
    fn decode_event(
        &self,
        protocol: Protocol,
        event: SseEvent,
    ) -> Result<DecodedEvent, ProviderFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderErrorClass {
    InvalidRequest,
    Authentication,
    RateLimited,
    UpstreamUnavailable,
    UpstreamFailure,
}

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
    pub(super) fn new(class: ProviderErrorClass, retry_hint: RetryHint) -> Self {
        Self { class, retry_hint }
    }

    pub fn class(&self) -> ProviderErrorClass {
        self.class
    }

    pub fn retry_hint(&self) -> RetryHint {
        self.retry_hint
    }
}

pub trait ErrorAdapter {
    fn classify_status(&self, status: StatusCode) -> ClassifiedProviderError;
}

pub trait CapabilityAdapter {
    fn validate_capabilities(&self, requested: CapabilitySet) -> Result<(), ProviderFailure>;
}
