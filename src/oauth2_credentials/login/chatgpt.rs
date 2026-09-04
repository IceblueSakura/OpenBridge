//! Explicit ChatGPT device login and PKCE token exchange.
//!
//! This state machine implements the private Codex device interaction observed in the pinned
//! reference clients: it creates a device session, polls for authorization material, verifies the
//! returned PKCE challenge, and exchanges the code before finalization. Every stage ordering,
//! retry policy, and polling rule belongs here so upstream protocol drift stays contained inside
//! the ChatGPT flow; only protocol-agnostic atoms are borrowed from `common`.

use std::{
    fmt,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::{
    provider::ProviderKind,
    providers::chatgpt::oauth::{ChatGptOAuthRegistration, REGISTRATION},
};

#[cfg(test)]
use super::common::ExchangedTokens;
use super::common::{
    LoginClock, LoginSleeper, OAuth2LoginError, OAuth2LoginOutcome, TokioSleeper, TransportError,
    finalize_login, map_device_request_error, map_exchange_error, map_poll_error,
};

mod transport;

use transport::{
    ChatGptOAuthTransport, DevicePoll, DeviceSession, PkceGrant, ReqwestChatGptOAuthTransport,
};

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

/// Runs the explicit ChatGPT device interaction, validates PKCE and tokens, and persists one bundle.
pub async fn login_chatgpt<F>(
    target: &super::super::storage::OAuth2LoginTarget,
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
            &TokioSleeper,
            &REGISTRATION,
            show_prompt,
        ),
    )
    .await
    .map_err(|_| OAuth2LoginError::TimedOut)?
}

/// Executes the transport-independent login stages used by production and deterministic tests.
async fn login_with<T, S, C, F>(
    target: &super::super::storage::OAuth2LoginTarget,
    source_version: super::super::storage::OAuth2AuthFileVersion,
    transport: &T,
    sleeper: &S,
    clock: &C,
    registration: &ChatGptOAuthRegistration,
    show_prompt: F,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
    C: LoginClock,
    F: FnOnce(&ChatGptDevicePrompt),
{
    // Start the fixed session budget before session creation so every stage's wall-clock
    // latency, including HTTP round-trips and prompt handling, counts against it.
    let started = clock.now();
    let deadline = started + registration.device_session_timeout;

    // Request a device interaction with bounded handling for authority throttling.
    let session = request_device_session(transport, sleeper, clock, deadline, registration).await?;
    let poll_interval = bounded_poll_interval(session.interval, registration);
    show_prompt(&ChatGptDevicePrompt {
        verification_uri: registration.verification_uri,
        user_code: session.user_code.clone(),
        expires_in: registration.device_session_timeout,
    });

    // Poll only the fixed private endpoint until it returns PKCE authorization material.
    let grant = poll_for_pkce_grant(
        transport,
        sleeper,
        clock,
        deadline,
        registration,
        &session,
        poll_interval,
    )
    .await?;
    validate_pkce(&grant)?;

    // Exchange the authorization code and finalize the account-bound bundle transactionally.
    let tokens = transport
        .exchange_authorization_code(&grant)
        .await
        .map_err(map_exchange_error)?;
    finalize_login(target, source_version, ProviderKind::ChatGpt, tokens).await
}

/// Requests the private device session with bounded 429 retry handling.
///
/// Retry delays are checked against the absolute session deadline so throttled session creation
/// cannot outlive the budget that started before the first request.
async fn request_device_session<T, S, C>(
    transport: &T,
    sleeper: &S,
    clock: &C,
    deadline: Instant,
    registration: &ChatGptOAuthRegistration,
) -> Result<DeviceSession, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
    C: LoginClock,
{
    // Retry only explicit authority throttling and cap every server-provided delay.
    for attempt in 1..=DEVICE_REQUEST_ATTEMPTS {
        match transport.request_device_session().await {
            Ok(session) => return Ok(session),
            Err(TransportError::RateLimited(retry_after)) if attempt < DEVICE_REQUEST_ATTEMPTS => {
                let delay = bounded_retry_after(retry_after, registration);
                // Reject a retry delay that no longer fits the remaining session budget.
                if deadline.saturating_duration_since(clock.now()) < delay {
                    return Err(OAuth2LoginError::TimedOut);
                }
                sleeper.sleep(delay).await;
            }
            Err(error) => return Err(map_device_request_error(error)),
        }
    }
    Err(OAuth2LoginError::DeviceRequest)
}

/// Polls for the private authorization response within the configured session budget.
///
/// The budget is an absolute deadline derived from the fixed device-session timeout, so sleep
/// intervals and transport latency are both charged against it.
async fn poll_for_pkce_grant<T, S, C>(
    transport: &T,
    sleeper: &S,
    clock: &C,
    deadline: Instant,
    registration: &ChatGptOAuthRegistration,
    session: &DeviceSession,
    poll_interval: Duration,
) -> Result<PkceGrant, OAuth2LoginError>
where
    T: ChatGptOAuthTransport,
    S: LoginSleeper,
    C: LoginClock,
{
    // Reject the next wait once it no longer fits the remaining session budget.
    loop {
        if deadline.saturating_duration_since(clock.now()) < poll_interval {
            return Err(OAuth2LoginError::TimedOut);
        }
        sleeper.sleep(poll_interval).await;

        // Continue only for documented pending or bounded throttle responses.
        match transport.poll_device_session(session).await {
            Ok(DevicePoll::Authorized(grant)) => return Ok(grant),
            Ok(DevicePoll::Pending) => {}
            Err(TransportError::RateLimited(retry_after)) => {
                let delay = bounded_retry_after(retry_after, registration);
                if deadline.saturating_duration_since(clock.now()) < delay {
                    return Err(OAuth2LoginError::TimedOut);
                }
                sleeper.sleep(delay).await;
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
    use crate::oauth2_credentials::{
        error::OAuth2CredentialManagerError, storage::OAuth2LoginTarget,
    };

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
        let bundle = super::super::super::document::parse_auth_document(
            ProviderKind::ChatGpt,
            &document,
            true,
        )
        .unwrap();
        let super::super::super::document::OAuth2AccountContext::ChatGpt {
            account_id,
            is_fedramp_account: _,
        } = &bundle.context
        else {
            panic!("expected a ChatGPT account context");
        };
        assert_eq!(account_id.expose_secret(), "synthetic-account");
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

    #[tokio::test]
    async fn chatgpt_session_creation_retries_within_budget_and_stops_at_deadline() {
        // A throttled session request retries once the capped delay fits the remaining budget.
        let transport = FakeTransport {
            sessions: Mutex::new(VecDeque::from([
                Err(TransportError::RateLimited(None)),
                Ok(synthetic_session()),
            ])),
            polls: Mutex::new(VecDeque::new()),
            tokens: Ok(valid_tokens()),
            poll_count: Mutex::new(0),
        };
        let sleeper = FakeSleeper::default();
        let deadline = sleeper.now() + REGISTRATION.device_session_timeout;
        let session =
            match request_device_session(&transport, &sleeper, &sleeper, deadline, &REGISTRATION)
                .await
            {
                Ok(session) => session,
                Err(error) => panic!("unexpected session request failure: {error}"),
            };
        assert_eq!(session.user_code.expose_secret(), "USER-CODE");
        assert_eq!(sleeper.delays(), vec![REGISTRATION.default_poll_interval]);

        // Once the capped delay no longer fits the remaining budget, throttling fails closed.
        let transport = FakeTransport::throttled_sessions();
        let sleeper = FakeSleeper::default();
        let deadline = sleeper.now() + Duration::from_secs(1);
        let error =
            match request_device_session(&transport, &sleeper, &sleeper, deadline, &REGISTRATION)
                .await
            {
                Ok(_) => panic!("expected a deadline failure"),
                Err(error) => error,
            };
        assert_eq!(error, OAuth2LoginError::TimedOut);
        assert!(sleeper.delays().is_empty());
    }

    #[derive(Clone, Copy)]
    enum FailureCase {
        Pkce,
        Token,
    }

    struct FakeTransport {
        sessions: Mutex<VecDeque<Result<DeviceSession, TransportError>>>,
        polls: Mutex<VecDeque<Result<DevicePoll, TransportError>>>,
        tokens: Result<ExchangedTokens, TransportError>,
        poll_count: Mutex<usize>,
    }

    impl FakeTransport {
        fn successful(verifier: &str) -> Self {
            let grant = grant(verifier, false);
            Self {
                sessions: Mutex::new(VecDeque::from([Ok(synthetic_session())])),
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
                sessions: Mutex::new(VecDeque::from([Ok(synthetic_session())])),
                polls: Mutex::new(VecDeque::from([Ok(DevicePoll::Authorized(grant))])),
                tokens,
                poll_count: Mutex::new(0),
            }
        }

        /// Fails every device-session request with explicit authority throttling.
        fn throttled_sessions() -> Self {
            Self {
                sessions: Mutex::new(VecDeque::new()),
                polls: Mutex::new(VecDeque::new()),
                tokens: Ok(valid_tokens()),
                poll_count: Mutex::new(0),
            }
        }

        fn poll_count(&self) -> usize {
            *self.poll_count.lock().unwrap()
        }
    }

    fn synthetic_session() -> DeviceSession {
        DeviceSession {
            device_auth_id: SecretString::from("synthetic-device".to_owned()),
            user_code: SecretString::from("USER-CODE".to_owned()),
            interval: Some(Duration::from_secs(5)),
        }
    }

    impl ChatGptOAuthTransport for FakeTransport {
        async fn request_device_session(&self) -> Result<DeviceSession, TransportError> {
            self.sessions
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(TransportError::RateLimited(None)))
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

    struct FakeSleeper {
        delays: Mutex<Vec<Duration>>,
        elapsed: Mutex<Duration>,
        base: Instant,
    }

    impl Default for FakeSleeper {
        fn default() -> Self {
            Self {
                delays: Mutex::new(Vec::new()),
                elapsed: Mutex::new(Duration::ZERO),
                base: Instant::now(),
            }
        }
    }

    impl FakeSleeper {
        fn delays(&self) -> Vec<Duration> {
            self.delays.lock().unwrap().clone()
        }
    }

    impl LoginSleeper for FakeSleeper {
        async fn sleep(&self, duration: Duration) {
            self.delays.lock().unwrap().push(duration);
            // Advance the virtual clock so deadline accounting stays deterministic.
            let mut elapsed = self.elapsed.lock().unwrap();
            *elapsed += duration;
        }
    }

    impl LoginClock for FakeSleeper {
        fn now(&self) -> Instant {
            // Return one stable virtual instant anchored at construction time.
            self.base + *self.elapsed.lock().unwrap()
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
                super::super::super::storage::next_test_id()
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
