//! Protocol-agnostic login primitives shared by every Provider OAuth flow.
//!
//! Upstream OAuth protocols drift independently per Provider, so each Provider owns its own
//! state-machine module and wire transport. This layer supplies only atoms that are not part of
//! any protocol flow: outcome/error shapes, transport error classification, the token bundle
//! shape, bounded wire parsing, error mapping, and the single transactional finalization step.
//! Polling semantics, retry policy, and flow ordering never live here.

use std::{
    future::Future,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use http::header;
use reqwest::Response;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, de::Error as _};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::provider::ProviderKind;

use super::super::{
    document::{serialize_auth_document, validate_exchanged_tokens},
    error::OAuth2CredentialManagerError,
    storage::{OAuth2AuthFileVersion, OAuth2LoginTarget, OAuth2StorageError},
};

/// Upper bound for one buffered OAuth response document.
pub(super) const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// Safe result of one complete login and transactional credential write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuth2LoginOutcome {
    provider: ProviderKind,
    pool_id: String,
}

impl OAuth2LoginOutcome {
    /// Returns the Provider whose managed bundle was replaced.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// Returns the compile-time credential pool populated by the login.
    pub fn pool_id(&self) -> &str {
        &self.pool_id
    }
}

/// Constructs one outcome after a Provider's finalization step succeeded.
pub(super) fn login_outcome(target: &OAuth2LoginTarget) -> OAuth2LoginOutcome {
    OAuth2LoginOutcome {
        provider: target.provider(),
        pool_id: target.pool_id().to_owned(),
    }
}

/// Value-free failure from one explicit Provider login.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OAuth2LoginError {
    /// The destination does not belong to the Provider that owns this login.
    #[error("OAuth2 login Provider is unsupported")]
    UnsupportedProvider,
    /// The fixed no-redirect HTTPS client could not be created.
    #[error("OAuth2 login client could not be initialized")]
    Client,
    /// Device-session creation failed before a valid response was received.
    #[error("OAuth2 device authorization request failed")]
    DeviceRequest,
    /// Device-session creation returned a terminal HTTP status.
    #[error("OAuth2 device authorization request returned HTTP {status}")]
    DeviceRequestStatus {
        /// Non-success HTTP status without any response body.
        status: u16,
    },
    /// Device polling failed before a terminal response was received.
    #[error("OAuth2 device authorization polling failed")]
    Poll,
    /// Device polling returned a terminal HTTP status.
    #[error("OAuth2 device authorization polling returned HTTP {status}")]
    PollStatus {
        /// Non-pending HTTP status without any response body.
        status: u16,
    },
    /// The authority kept the operation rate-limited after its bounded retry budget.
    #[error("OAuth2 authorization authority is rate-limiting this operation")]
    RateLimited {
        /// Safe delta hint supplied by the authority, if present.
        retry_after: Option<Duration>,
    },
    /// The administrator or authority denied the device session.
    #[error("OAuth2 device authorization was denied")]
    AccessDenied,
    /// The interactive session exceeded its fixed deadline.
    #[error("OAuth2 device authorization timed out")]
    TimedOut,
    /// The returned verifier does not match the returned S256 challenge.
    #[error("OAuth2 device authorization returned inconsistent PKCE material")]
    PkceMismatch,
    /// The authorization-code exchange failed before a valid response was received.
    #[error("OAuth2 authorization-code exchange failed")]
    Exchange,
    /// The authorization-code exchange returned a terminal HTTP status.
    #[error("OAuth2 authorization-code exchange returned HTTP {status}")]
    ExchangeStatus {
        /// Non-success HTTP status without any response body.
        status: u16,
    },
    /// The exchange returned an incomplete, expired, or inconsistently bound token bundle.
    #[error("OAuth2 authorization-code exchange returned an invalid token bundle")]
    InvalidTokenBundle(#[source] OAuth2CredentialManagerError),
    /// The bounded blocking file transaction task could not complete.
    #[error("OAuth2 managed auth transaction task failed")]
    TransactionTask,
    /// The managed auth document could not be transactionally read or replaced.
    #[error("OAuth2 managed auth transaction failed")]
    Storage(#[source] OAuth2StorageError),
}

/// Sleep abstraction that keeps state-machine tests free from wall-clock delays.
pub(super) trait LoginSleeper {
    /// Waits for one bounded state-machine interval.
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;
}

/// Production sleeper bound to the tokio runtime clock.
pub(super) struct TokioSleeper;

impl LoginSleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Monotonic clock abstraction that keeps absolute budget deadlines deterministic in tests.
///
/// State machines enforce authority-advertised budgets against a deadline, so real HTTP latency
/// and non-sleep stages count against the budget in production.
pub(super) trait LoginClock {
    /// Returns the current monotonic instant.
    fn now(&self) -> Instant;
}

impl LoginClock for TokioSleeper {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Value-free transport failure classification shared by Provider wire transports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransportError {
    /// The request failed before any response status was observed.
    Network,
    /// The authority returned a terminal non-success status.
    HttpStatus(u16),
    /// The authority explicitly throttled the request, with an optional delta hint.
    RateLimited(Option<Duration>),
    /// The response body did not carry the expected document shape.
    InvalidResponse,
    /// The response body exceeded the bounded OAuth document limit.
    BodyTooLarge,
}

/// Complete token bundle issued by an authority after a successful grant.
#[derive(Deserialize)]
pub(super) struct ExchangedTokens {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) id_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) access_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) refresh_token: SecretString,
}

/// Buffers one successful response under the strict OAuth document size limit.
pub(super) async fn parse_json_body<T>(response: Response) -> Result<T, TransportError>
where
    T: for<'de> Deserialize<'de>,
{
    // Collect chunks while rejecting a body that reaches beyond the configured bound.
    let mut body = Zeroizing::new(Vec::new());
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| TransportError::Network)?;
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or(TransportError::BodyTooLarge)?;
        if next_len > MAX_RESPONSE_BODY_BYTES {
            return Err(TransportError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk);
    }

    // Deserialize into owned secret wrappers and zero the source buffer on return.
    serde_json::from_slice(&body).map_err(|_| TransportError::InvalidResponse)
}

/// Parses a bounded delta-seconds Retry-After hint without retaining header text.
pub(super) fn parse_retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Moves one non-empty JSON string directly into a zeroizing secret wrapper.
pub(super) fn deserialize_non_empty_secret<'de, D>(
    deserializer: D,
) -> Result<SecretString, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Err(D::Error::custom("secret value must not be empty"));
    }
    Ok(SecretString::from(value))
}

/// Accepts a numeric or string interval and converts positive seconds into a duration.
pub(super) fn deserialize_optional_interval<'de, D>(
    deserializer: D,
) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let seconds = match value {
        None | Some(serde_json::Value::Null) => return Ok(None),
        Some(serde_json::Value::Number(number)) => number.as_u64(),
        Some(serde_json::Value::String(value)) => value.parse::<u64>().ok(),
        Some(_) => None,
    }
    .ok_or_else(|| D::Error::custom("interval must be positive seconds"))?;
    if seconds == 0 {
        return Err(D::Error::custom("interval must be positive seconds"));
    }
    Ok(Some(Duration::from_secs(seconds)))
}

/// Maps a value-free transport result at device creation into the public stage error.
pub(super) fn map_device_request_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::DeviceRequestStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::DeviceRequest,
    }
}

/// Maps a value-free polling transport result into the public stage error.
pub(super) fn map_poll_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::PollStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::Poll,
    }
}

/// Maps a value-free token exchange result into the public stage error.
pub(super) fn map_exchange_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::ExchangeStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::Exchange,
    }
}

/// Validates one exchanged token bundle, serializes it, and publishes it transactionally.
///
/// This is the shared finalization composition step: every Provider state machine owns all
/// protocol-specific stages before calling it, and the managed auth file is only touched after
/// complete validation succeeds. A Provider whose protocol later needs extra steps before or
/// after persistence composes its own finalization instead of calling this function.
pub(super) async fn finalize_login(
    target: &OAuth2LoginTarget,
    source_version: OAuth2AuthFileVersion,
    provider: ProviderKind,
    tokens: ExchangedTokens,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError> {
    // Validate the complete Provider-bound token bundle before touching the file.
    let bundle = validate_exchanged_tokens(
        provider,
        tokens.id_token.expose_secret(),
        tokens.access_token.expose_secret(),
        tokens.refresh_token.expose_secret(),
    )
    .map_err(OAuth2LoginError::InvalidTokenBundle)?;
    let document =
        serialize_auth_document(&bundle).map_err(OAuth2LoginError::InvalidTokenBundle)?;

    // Publish one complete document only if no competing writer changed the source version.
    let write_target = target.clone();
    let write = tokio::task::spawn_blocking(move || {
        write_target.compare_and_replace(&source_version, &document)
    })
    .await
    .map_err(|_| OAuth2LoginError::TransactionTask)?;
    write.map_err(OAuth2LoginError::Storage)?;
    Ok(login_outcome(target))
}
