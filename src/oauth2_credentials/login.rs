//! Explicit ChatGPT device login and PKCE token exchange.
//!
//! This state machine uses only the compile-time ChatGPT OAuth registration. Device identifiers,
//! authorization codes, PKCE material, and tokens remain purpose-bound secrets; diagnostics carry
//! only value-free stages or HTTP status classes. Tests replace both transport and sleeping.

use std::{fmt, future::Future, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use http::{StatusCode, header};
use reqwest::{Client, Response, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, de::Error as _};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    provider::ProviderKind,
    providers::chatgpt::oauth::{ChatGptOAuthRegistration, REGISTRATION},
};

use super::{
    OAuth2CredentialManagerError, OAuth2LoginTarget,
    document::{serialize_auth_document, validate_exchanged_tokens},
    storage::{OAuth2AuthFileVersion, OAuth2StorageError},
};

const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;
const DEVICE_REQUEST_ATTEMPTS: usize = 4;

/// User-visible values for one explicitly initiated ChatGPT device login.
pub struct ChatGptDevicePrompt {
    verification_uri: &'static str,
    user_code: SecretString,
    expires_in: Duration,
}

impl ChatGptDevicePrompt {
    /// Returns the fixed browser destination registered by the ChatGPT Provider.
    pub fn verification_uri(&self) -> &str {
        self.verification_uri
    }

    /// Returns the short-lived code solely for display to the initiating administrator.
    pub fn user_code(&self) -> &str {
        self.user_code.expose_secret()
    }

    /// Returns the maximum remaining lifetime advertised for this local session.
    pub fn expires_in(&self) -> Duration {
        self.expires_in
    }
}

impl fmt::Debug for ChatGptDevicePrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatGptDevicePrompt")
            .field("verification_uri", &self.verification_uri)
            .field("user_code", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// Safe result of one complete device login and transactional credential write.
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

/// Runs the explicit ChatGPT device interaction, validates PKCE and tokens, and persists one bundle.
pub async fn login_chatgpt<F>(
    target: &OAuth2LoginTarget,
    show_prompt: F,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError>
where
    F: FnOnce(&ChatGptDevicePrompt),
{
    // Reject cross-Provider use and capture the destination version before network interaction.
    if target.provider() != ProviderKind::ChatGpt {
        return Err(OAuth2LoginError::UnsupportedProvider);
    }
    let source_version = target
        .capture_version()
        .map_err(OAuth2LoginError::Storage)?;
    let transport = ReqwestChatGptOAuthTransport::new(&REGISTRATION)?;

    // Bound the complete interactive state machine independently from per-request timeouts.
    tokio::time::timeout(
        REGISTRATION.device_session_timeout,
        login_with(
            target,
            source_version,
            &transport,
            &TokioSleeper,
            &REGISTRATION,
            show_prompt,
        ),
    )
    .await
    .map_err(|_| OAuth2LoginError::TimedOut)?
}

/// Executes the transport-independent login stages used by production and deterministic tests.
async fn login_with<T, S, F>(
    target: &OAuth2LoginTarget,
    source_version: OAuth2AuthFileVersion,
    transport: &T,
    sleeper: &S,
    registration: &ChatGptOAuthRegistration,
    show_prompt: F,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
    F: FnOnce(&ChatGptDevicePrompt),
{
    // Request a device interaction with bounded handling for authority throttling.
    let session = request_device_session(transport, sleeper, registration).await?;
    let poll_interval = bounded_poll_interval(session.interval, registration);
    show_prompt(&ChatGptDevicePrompt {
        verification_uri: registration.verification_uri,
        user_code: session.user_code.clone(),
        expires_in: registration.device_session_timeout,
    });

    // Poll only the fixed private endpoint until it returns PKCE authorization material.
    let grant =
        poll_for_pkce_grant(transport, sleeper, registration, &session, poll_interval).await?;
    validate_pkce(&grant)?;

    // Exchange and validate the complete account-bound token bundle before touching the file.
    let tokens = transport
        .exchange_authorization_code(&grant)
        .await
        .map_err(map_exchange_error)?;
    let bundle = validate_exchanged_tokens(
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
    Ok(OAuth2LoginOutcome {
        provider: target.provider(),
        pool_id: target.pool_id().to_owned(),
    })
}

/// Requests the private device session with bounded 429 retry handling.
async fn request_device_session<T, S>(
    transport: &T,
    sleeper: &S,
    registration: &ChatGptOAuthRegistration,
) -> Result<DeviceSession, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
{
    // Retry only explicit authority throttling and cap every server-provided delay.
    for attempt in 1..=DEVICE_REQUEST_ATTEMPTS {
        match transport.request_device_session().await {
            Ok(session) => return Ok(session),
            Err(TransportError::RateLimited(retry_after)) if attempt < DEVICE_REQUEST_ATTEMPTS => {
                sleeper
                    .sleep(bounded_retry_after(retry_after, registration))
                    .await;
            }
            Err(error) => return Err(map_device_request_error(error)),
        }
    }
    Err(OAuth2LoginError::DeviceRequest)
}

/// Polls for the private authorization response within the configured session budget.
async fn poll_for_pkce_grant<T, S>(
    transport: &T,
    sleeper: &S,
    registration: &ChatGptOAuthRegistration,
    session: &DeviceSession,
    poll_interval: Duration,
) -> Result<PkceGrant, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
{
    let mut remaining = registration.device_session_timeout;

    // Spend the deterministic budget on each wait before issuing the next poll.
    loop {
        if remaining < poll_interval {
            return Err(OAuth2LoginError::TimedOut);
        }
        sleeper.sleep(poll_interval).await;
        remaining = remaining.saturating_sub(poll_interval);

        // Continue only for documented pending or bounded throttle responses.
        match transport.poll_device_session(session).await {
            Ok(DevicePoll::Authorized(grant)) => return Ok(grant),
            Ok(DevicePoll::Pending) => {}
            Err(TransportError::RateLimited(retry_after)) => {
                let delay = bounded_retry_after(retry_after, registration);
                if remaining < delay {
                    return Err(OAuth2LoginError::TimedOut);
                }
                sleeper.sleep(delay).await;
                remaining = remaining.saturating_sub(delay);
            }
            Err(error) => return Err(map_poll_error(error)),
        }
    }
}

/// Verifies the authority-returned S256 challenge in constant time before token exchange.
fn validate_pkce(grant: &PkceGrant) -> Result<(), OAuth2LoginError> {
    // Compute the base64url SHA-256 challenge from the short-lived verifier.
    let digest = Sha256::digest(grant.code_verifier.expose_secret().as_bytes());
    let computed = Zeroizing::new(URL_SAFE_NO_PAD.encode(digest));
    let expected = grant.code_challenge.expose_secret().as_bytes();

    // Reject unequal lengths before applying constant-time byte comparison.
    if computed.len() != expected.len() || computed.as_bytes().ct_eq(expected).unwrap_u8() != 1 {
        return Err(OAuth2LoginError::PkceMismatch);
    }
    Ok(())
}

/// Applies the trusted lower and upper interval bounds to the authority hint.
fn bounded_poll_interval(
    interval: Option<Duration>,
    registration: &ChatGptOAuthRegistration,
) -> Duration {
    interval
        .unwrap_or(registration.default_poll_interval)
        .max(registration.minimum_poll_interval)
        .min(registration.maximum_poll_interval)
}

/// Caps Retry-After and supplies the default polling delay when it is absent.
fn bounded_retry_after(
    retry_after: Option<Duration>,
    registration: &ChatGptOAuthRegistration,
) -> Duration {
    retry_after
        .unwrap_or(registration.default_poll_interval)
        .max(Duration::from_secs(1))
        .min(registration.maximum_poll_interval)
}

/// Maps a value-free transport result at device creation into the public stage error.
fn map_device_request_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::DeviceRequestStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::DeviceRequest,
    }
}

/// Maps a value-free polling transport result into the public stage error.
fn map_poll_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::PollStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::Poll,
    }
}

/// Maps a value-free token exchange result into the public stage error.
fn map_exchange_error(error: TransportError) -> OAuth2LoginError {
    match error {
        TransportError::RateLimited(retry_after) => OAuth2LoginError::RateLimited { retry_after },
        TransportError::HttpStatus(status) => OAuth2LoginError::ExchangeStatus { status },
        TransportError::Network
        | TransportError::InvalidResponse
        | TransportError::BodyTooLarge => OAuth2LoginError::Exchange,
    }
}

/// Transport abstraction that keeps deterministic tests independent from real HTTPS endpoints.
trait ChatGptOAuthTransport {
    /// Requests a new private device interaction.
    fn request_device_session(
        &self,
    ) -> impl Future<Output = Result<DeviceSession, TransportError>> + Send;

    /// Polls an existing private device interaction once.
    fn poll_device_session(
        &self,
        session: &DeviceSession,
    ) -> impl Future<Output = Result<DevicePoll, TransportError>> + Send;

    /// Exchanges a validated authorization code and PKCE verifier for tokens.
    fn exchange_authorization_code(
        &self,
        grant: &PkceGrant,
    ) -> impl Future<Output = Result<ExchangedTokens, TransportError>> + Send;
}

/// Sleep abstraction used to avoid wall-clock delays in state-machine tests.
trait LoginSleeper {
    /// Waits for one bounded state-machine interval.
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;
}

struct TokioSleeper;

impl LoginSleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// HTTPS transport restricted to the compile-time ChatGPT OAuth registration.
struct ReqwestChatGptOAuthTransport {
    client: Client,
    registration: &'static ChatGptOAuthRegistration,
}

impl ReqwestChatGptOAuthTransport {
    /// Builds a no-redirect client with the fixed per-request timeout.
    fn new(registration: &'static ChatGptOAuthRegistration) -> Result<Self, OAuth2LoginError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(registration.request_timeout)
            .build()
            .map_err(|_| OAuth2LoginError::Client)?;
        Ok(Self {
            client,
            registration,
        })
    }
}

impl ChatGptOAuthTransport for ReqwestChatGptOAuthTransport {
    async fn request_device_session(&self) -> Result<DeviceSession, TransportError> {
        // Encode only the compile-time public client identity for device-session creation.
        let body = serde_json::to_vec(&serde_json::json!({
            "client_id": self.registration.client_id,
        }))
        .map_err(|_| TransportError::InvalidResponse)?;
        let response = self
            .client
            .post(self.registration.device_authorization_endpoint)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Parse a bounded success body and normalize HTTP failures without reading error bodies.
        parse_json_response(response).await
    }

    async fn poll_device_session(
        &self,
        session: &DeviceSession,
    ) -> Result<DevicePoll, TransportError> {
        // Encode the short-lived session identifiers only for the fixed private poll endpoint.
        let body = Zeroizing::new(
            serde_json::to_vec(&serde_json::json!({
                "device_auth_id": session.device_auth_id.expose_secret(),
                "user_code": session.user_code.expose_secret(),
            }))
            .map_err(|_| TransportError::InvalidResponse)?,
        );
        let response = self
            .client
            .post(self.registration.device_poll_endpoint)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.to_vec())
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Treat only the observed private pending statuses as a non-terminal poll result.
        match response.status() {
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => Ok(DevicePoll::Pending),
            StatusCode::OK => parse_json_body(response).await.map(DevicePoll::Authorized),
            StatusCode::TOO_MANY_REQUESTS => {
                Err(TransportError::RateLimited(parse_retry_after(&response)))
            }
            status => Err(TransportError::HttpStatus(status.as_u16())),
        }
    }

    async fn exchange_authorization_code(
        &self,
        grant: &PkceGrant,
    ) -> Result<ExchangedTokens, TransportError> {
        // Form-encode the authorization code and verifier for the fixed OAuth token endpoint.
        let body = Zeroizing::new({
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer
                .append_pair("grant_type", "authorization_code")
                .append_pair("code", grant.authorization_code.expose_secret())
                .append_pair("redirect_uri", self.registration.redirect_uri)
                .append_pair("client_id", self.registration.client_id)
                .append_pair("code_verifier", grant.code_verifier.expose_secret());
            serializer.finish()
        });
        let response = self
            .client
            .post(self.registration.token_endpoint)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body.to_string())
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Parse only a bounded successful exchange document.
        parse_json_response(response).await
    }
}

/// Parses a bounded JSON body only for HTTP success and classifies other statuses safely.
async fn parse_json_response<T>(response: Response) -> Result<T, TransportError>
where
    T: for<'de> Deserialize<'de>,
{
    match response.status() {
        StatusCode::OK => parse_json_body(response).await,
        StatusCode::TOO_MANY_REQUESTS => {
            Err(TransportError::RateLimited(parse_retry_after(&response)))
        }
        status => Err(TransportError::HttpStatus(status.as_u16())),
    }
}

/// Buffers one successful response under the strict OAuth document size limit.
async fn parse_json_body<T>(response: Response) -> Result<T, TransportError>
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
fn parse_retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[derive(Deserialize)]
struct DeviceSession {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    device_auth_id: SecretString,
    #[serde(alias = "usercode", deserialize_with = "deserialize_non_empty_secret")]
    user_code: SecretString,
    #[serde(default, deserialize_with = "deserialize_optional_interval")]
    interval: Option<Duration>,
}

enum DevicePoll {
    Pending,
    Authorized(PkceGrant),
}

#[derive(Deserialize)]
struct PkceGrant {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    authorization_code: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    code_challenge: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    code_verifier: SecretString,
}

#[derive(Deserialize)]
struct ExchangedTokens {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    id_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    access_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    refresh_token: SecretString,
}

/// Moves one non-empty JSON string directly into a zeroizing secret wrapper.
fn deserialize_non_empty_secret<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
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
fn deserialize_optional_interval<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    Network,
    HttpStatus(u16),
    RateLimited(Option<Duration>),
    InvalidResponse,
    BodyTooLarge,
}

/// Value-free failure from explicit ChatGPT device login.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OAuth2LoginError {
    /// The destination does not belong to the ChatGPT Provider.
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

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[tokio::test]
    async fn chatgpt_device_login_polls_then_persists_one_validated_bundle_atomically() {
        let fixture = TestDirectory::new();
        let auth_file = fixture.path.join("auth.json");
        let target =
            OAuth2LoginTarget::new(ProviderKind::ChatGpt, "chatgpt-codex", auth_file.clone());
        let verifier = "synthetic-verifier";
        let transport = FakeTransport::successful(verifier);
        let sleeper = FakeSleeper::default();
        let source = target.capture_version().unwrap();
        let prompt_code = Arc::new(Mutex::new(None));
        let observed_code = Arc::clone(&prompt_code);

        // Drive one pending poll through PKCE exchange without real network or sleeping.
        let outcome = login_with(
            &target,
            source,
            &transport,
            &sleeper,
            &REGISTRATION,
            move |prompt| {
                *observed_code.lock().unwrap() = Some(prompt.user_code().to_owned());
                assert_eq!(prompt.verification_uri(), REGISTRATION.verification_uri);
            },
        )
        .await
        .unwrap();

        // Reload the persisted envelope through the production validator.
        assert_eq!(outcome.provider(), ProviderKind::ChatGpt);
        assert_eq!(outcome.pool_id(), "chatgpt-codex");
        assert_eq!(prompt_code.lock().unwrap().as_deref(), Some("USER-CODE"));
        let document = Zeroizing::new(fs::read(auth_file).unwrap());
        let bundle = super::super::document::parse_auth_document(&document, true).unwrap();
        assert_eq!(bundle.account_id.expose_secret(), "synthetic-account");
        assert_eq!(transport.poll_count(), 2);
        assert_eq!(sleeper.delays().len(), 2);
    }

    #[tokio::test]
    async fn pkce_mismatch_or_invalid_token_preserves_the_previous_bundle() {
        // Run independent invalid-PKCE and invalid-token cases against pre-existing documents.
        for failure in [FailureCase::Pkce, FailureCase::Token] {
            let fixture = TestDirectory::new();
            let path = fixture.path.join("auth.json");
            fs::write(&path, b"synthetic-previous-document").unwrap();
            let target =
                OAuth2LoginTarget::new(ProviderKind::ChatGpt, "chatgpt-codex", path.clone());
            let source = target.capture_version().unwrap();
            let transport = FakeTransport::failing(failure);

            // Reject all invalid exchange material before the transactional file boundary.
            let error = login_with(
                &target,
                source,
                &transport,
                &FakeSleeper::default(),
                &REGISTRATION,
                |_| {},
            )
            .await
            .unwrap_err();
            match failure {
                FailureCase::Pkce => assert_eq!(error, OAuth2LoginError::PkceMismatch),
                FailureCase::Token => assert!(matches!(
                    error,
                    OAuth2LoginError::InvalidTokenBundle(
                        OAuth2CredentialManagerError::InvalidAccessToken
                    )
                )),
            }
            assert_eq!(fs::read(path).unwrap(), b"synthetic-previous-document");
        }
    }

    #[derive(Clone, Copy)]
    enum FailureCase {
        Pkce,
        Token,
    }

    struct FakeTransport {
        polls: Mutex<VecDeque<Result<DevicePoll, TransportError>>>,
        tokens: Result<ExchangedTokens, TransportError>,
        poll_count: Mutex<usize>,
    }

    impl FakeTransport {
        fn successful(verifier: &str) -> Self {
            let grant = grant(verifier, false);
            Self {
                polls: Mutex::new(VecDeque::from([
                    Ok(DevicePoll::Pending),
                    Ok(DevicePoll::Authorized(grant)),
                ])),
                tokens: Ok(valid_tokens()),
                poll_count: Mutex::new(0),
            }
        }

        fn failing(failure: FailureCase) -> Self {
            let grant = grant("synthetic-verifier", matches!(failure, FailureCase::Pkce));
            let tokens = match failure {
                FailureCase::Pkce => Ok(valid_tokens()),
                FailureCase::Token => Ok(ExchangedTokens {
                    id_token: SecretString::from("synthetic-id".to_owned()),
                    access_token: SecretString::from("not-a-jwt".to_owned()),
                    refresh_token: SecretString::from("synthetic-refresh".to_owned()),
                }),
            };
            Self {
                polls: Mutex::new(VecDeque::from([Ok(DevicePoll::Authorized(grant))])),
                tokens,
                poll_count: Mutex::new(0),
            }
        }

        fn poll_count(&self) -> usize {
            *self.poll_count.lock().unwrap()
        }
    }

    impl ChatGptOAuthTransport for FakeTransport {
        async fn request_device_session(&self) -> Result<DeviceSession, TransportError> {
            Ok(DeviceSession {
                device_auth_id: SecretString::from("synthetic-device".to_owned()),
                user_code: SecretString::from("USER-CODE".to_owned()),
                interval: Some(Duration::from_secs(5)),
            })
        }

        async fn poll_device_session(
            &self,
            _session: &DeviceSession,
        ) -> Result<DevicePoll, TransportError> {
            *self.poll_count.lock().unwrap() += 1;
            self.polls
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake poll response must be configured")
        }

        async fn exchange_authorization_code(
            &self,
            _grant: &PkceGrant,
        ) -> Result<ExchangedTokens, TransportError> {
            self.tokens.as_ref().map(clone_tokens).map_err(Clone::clone)
        }
    }

    #[derive(Default)]
    struct FakeSleeper {
        delays: Mutex<Vec<Duration>>,
    }

    impl FakeSleeper {
        fn delays(&self) -> Vec<Duration> {
            self.delays.lock().unwrap().clone()
        }
    }

    impl LoginSleeper for FakeSleeper {
        async fn sleep(&self, duration: Duration) {
            self.delays.lock().unwrap().push(duration);
        }
    }

    fn grant(verifier: &str, mismatch: bool) -> PkceGrant {
        let challenge = if mismatch {
            "synthetic-mismatch".to_owned()
        } else {
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
        };
        PkceGrant {
            authorization_code: SecretString::from("synthetic-code".to_owned()),
            code_challenge: SecretString::from(challenge),
            code_verifier: SecretString::from(verifier.to_owned()),
        }
    }

    fn valid_tokens() -> ExchangedTokens {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3_600;
        ExchangedTokens {
            id_token: SecretString::from(jwt(
                r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"synthetic-account"}}"#,
            )),
            access_token: SecretString::from(jwt(&format!(r#"{{"exp":{expiry}}}"#))),
            refresh_token: SecretString::from("synthetic-refresh".to_owned()),
        }
    }

    fn clone_tokens(tokens: &ExchangedTokens) -> ExchangedTokens {
        ExchangedTokens {
            id_token: tokens.id_token.clone(),
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
        }
    }

    fn jwt(payload: &str) -> String {
        format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(b"{}"),
            URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            URL_SAFE_NO_PAD.encode(b"signature")
        )
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openbridge-oauth-login-test-{}-{}",
                std::process::id(),
                super::super::storage::next_test_id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Remove only files created inside this process-unique test directory.
            if let Ok(entries) = fs::read_dir(&self.path) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.path);
        }
    }
}
