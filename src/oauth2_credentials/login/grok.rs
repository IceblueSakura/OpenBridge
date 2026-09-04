//! Explicit Grok RFC 8628 device authorization login and token polling.
//!
//! The authority publishes a standard device authorization endpoint and polls the ordinary token
//! endpoint with the `device_code` grant; no PKCE material or secondary approval endpoints are
//! involved. Every polling semantic, budget rule, and interval policy belongs here so upstream
//! protocol drift stays contained inside the Grok flow; only protocol-agnostic atoms are borrowed
//! from `common`.

use std::{
    fmt,
    time::{Duration, Instant},
};

use secrecy::{ExposeSecret, SecretString};

use crate::{
    provider::ProviderKind,
    providers::grok::oauth::{GrokOAuthRegistration, REGISTRATION},
};

use super::common::{
    ExchangedTokens, LoginClock, LoginSleeper, OAuth2LoginError, OAuth2LoginOutcome, TokioSleeper,
    TransportError, finalize_login, map_device_request_error, map_poll_error,
};

mod transport;

use transport::{GrokDeviceSession, GrokOAuthTransport, GrokTokenPoll, ReqwestGrokOAuthTransport};

/// User-visible values for one explicitly initiated Grok device login.
pub struct GrokDevicePrompt {
    verification_uri: &'static str,
    user_code: SecretString,
    expires_in: Duration,
}

impl GrokDevicePrompt {
    /// Returns the fixed browser destination where the administrator approves the session.
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

impl fmt::Debug for GrokDevicePrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokDevicePrompt")
            .field("verification_uri", &self.verification_uri)
            .field("user_code", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// Runs the explicit Grok device interaction, polls for tokens, and persists one bundle.
pub async fn login_grok<F>(
    target: &super::super::storage::OAuth2LoginTarget,
    show_prompt: F,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError>
where
    F: FnOnce(&GrokDevicePrompt),
{
    // Reject cross-Provider use and capture the destination version before network interaction.
    if target.provider() != ProviderKind::Grok {
        return Err(OAuth2LoginError::UnsupportedProvider);
    }
    let source_version = target
        .capture_version()
        .map_err(OAuth2LoginError::Storage)?;
    let transport = ReqwestGrokOAuthTransport::new(&REGISTRATION)?;

    // Bound the complete interactive state machine independently from per-request timeouts.
    tokio::time::timeout(
        REGISTRATION.device_session_timeout,
        login_grok_with(
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
async fn login_grok_with<T, S, C, F>(
    target: &super::super::storage::OAuth2LoginTarget,
    source_version: super::super::storage::OAuth2AuthFileVersion,
    transport: &T,
    sleeper: &S,
    clock: &C,
    registration: &GrokOAuthRegistration,
    show_prompt: F,
) -> Result<OAuth2LoginOutcome, OAuth2LoginError>
where
    T: GrokOAuthTransport,
    S: LoginSleeper,
    C: LoginClock,
    F: FnOnce(&GrokDevicePrompt),
{
    // Start the authority budget before session creation so every stage's wall-clock latency,
    // including HTTP round-trips and prompt handling, counts against the advertised expiry.
    let started = clock.now();
    let session = transport
        .request_device_session()
        .await
        .map_err(map_device_request_error)?;
    let budget = session.expires_in.min(registration.device_session_timeout);
    let deadline = started + budget;
    let mut poll_interval = bounded_poll_interval(session.interval, registration);
    show_prompt(&GrokDevicePrompt {
        verification_uri: registration.verification_uri,
        user_code: session.user_code.clone(),
        expires_in: budget,
    });

    // Poll the token endpoint under RFC 8628 semantics until a terminal result arrives.
    let tokens = poll_for_tokens(
        transport,
        sleeper,
        clock,
        deadline,
        &session,
        &mut poll_interval,
        registration,
    )
    .await?;

    // Validate the complete subscription-bound token bundle and publish it transactionally.
    finalize_login(target, source_version, ProviderKind::Grok, tokens).await
}

/// Polls the token endpoint once per bounded interval and applies RFC 8628 polling semantics.
///
/// The budget is an absolute deadline derived from the authority-advertised `expires_in`, so
/// sleep intervals and transport latency are both charged against it.
async fn poll_for_tokens<T, S, C>(
    transport: &T,
    sleeper: &S,
    clock: &C,
    deadline: Instant,
    session: &GrokDeviceSession,
    poll_interval: &mut Duration,
    registration: &GrokOAuthRegistration,
) -> Result<ExchangedTokens, OAuth2LoginError>
where
    T: GrokOAuthTransport,
    S: LoginSleeper,
    C: LoginClock,
{
    loop {
        // Reject the next wait once it no longer fits the remaining authority budget.
        if deadline.saturating_duration_since(clock.now()) < *poll_interval {
            return Err(OAuth2LoginError::TimedOut);
        }
        sleeper.sleep(*poll_interval).await;

        match transport.poll_for_tokens(session).await {
            Ok(GrokTokenPoll::Tokens(tokens)) => return Ok(tokens),
            Ok(GrokTokenPoll::Pending) => {}
            Ok(GrokTokenPoll::SlowDown) => {
                // RFC 8628: slow_down increases the polling interval, never terminates.
                *poll_interval = poll_interval
                    .saturating_add(registration.slow_down_increment)
                    .min(registration.maximum_poll_interval);
            }
            Ok(GrokTokenPoll::AccessDenied) => return Err(OAuth2LoginError::AccessDenied),
            Ok(GrokTokenPoll::ExpiredToken) => return Err(OAuth2LoginError::TimedOut),
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

/// Applies the trusted lower and upper interval bounds to the authority hint.
fn bounded_poll_interval(
    interval: Option<Duration>,
    registration: &GrokOAuthRegistration,
) -> Duration {
    interval
        .unwrap_or(registration.default_poll_interval)
        .max(registration.minimum_poll_interval)
        .min(registration.maximum_poll_interval)
}

/// Caps Retry-After and supplies the default polling delay when it is absent.
fn bounded_retry_after(
    retry_after: Option<Duration>,
    registration: &GrokOAuthRegistration,
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

    use secrecy::SecretString;
    use zeroize::Zeroizing;

    use super::*;
    use crate::oauth2_credentials::{
        error::OAuth2CredentialManagerError, storage::OAuth2LoginTarget,
    };

    #[tokio::test]
    async fn grok_device_login_polls_then_persists_one_validated_bundle_atomically() {
        let fixture = TestDirectory::new();
        let auth_file = fixture.path.join("auth.json");
        let target = OAuth2LoginTarget::new(ProviderKind::Grok, "grok-cli", auth_file.clone());
        let transport = FakeTransport::successful();
        let sleeper = FakeTime::default();
        let source = target.capture_version().unwrap();
        let prompt_code = Arc::new(Mutex::new(None));
        let observed_code = Arc::clone(&prompt_code);

        // Drive pending and slow_down polls through token issuance without real network or sleep.
        let outcome = login_grok_with(
            &target,
            source,
            &transport,
            &sleeper,
            &sleeper,
            &REGISTRATION,
            move |prompt| {
                *observed_code.lock().unwrap() = Some(prompt.user_code().to_owned());
                assert_eq!(prompt.verification_uri(), REGISTRATION.verification_uri);
                assert_eq!(prompt.expires_in(), Duration::from_secs(1_800));
            },
        )
        .await
        .unwrap();

        // Reload the persisted envelope through the production validator.
        assert_eq!(outcome.provider(), ProviderKind::Grok);
        assert_eq!(outcome.pool_id(), "grok-cli");
        assert_eq!(prompt_code.lock().unwrap().as_deref(), Some("USER-CODE"));
        let document = Zeroizing::new(fs::read(auth_file).unwrap());
        let bundle =
            super::super::super::document::parse_auth_document(ProviderKind::Grok, &document, true)
                .unwrap();
        let super::super::super::document::OAuth2AccountContext::Grok { subject, .. } =
            &bundle.context
        else {
            panic!("expected a Grok account context");
        };
        assert_eq!(subject.expose_secret(), "synthetic-subject");
        // Two waiting polls plus one slow_down interval increase before the final wait.
        assert_eq!(transport.poll_count(), 3);
        let delays = sleeper.delays();
        assert_eq!(
            delays,
            vec![
                REGISTRATION.default_poll_interval,
                REGISTRATION.default_poll_interval,
                REGISTRATION.default_poll_interval + REGISTRATION.slow_down_increment,
            ]
        );
    }

    #[tokio::test]
    async fn grok_device_login_rejects_denied_expired_and_invalid_outcomes_without_writing() {
        for failure in [
            FailureCase::Denied,
            FailureCase::Expired,
            FailureCase::InvalidTokens,
        ] {
            let fixture = TestDirectory::new();
            let path = fixture.path.join("auth.json");
            fs::write(&path, b"synthetic-previous-document").unwrap();
            let target = OAuth2LoginTarget::new(ProviderKind::Grok, "grok-cli", path.clone());
            let source = target.capture_version().unwrap();
            let transport = FakeTransport::failing(failure);

            // Reject every terminal failure before the transactional file boundary.
            let time = FakeTime::default();
            let error = login_grok_with(
                &target,
                source,
                &transport,
                &time,
                &time,
                &REGISTRATION,
                |_| {},
            )
            .await
            .unwrap_err();
            match failure {
                FailureCase::Denied => assert_eq!(error, OAuth2LoginError::AccessDenied),
                FailureCase::Expired => assert_eq!(error, OAuth2LoginError::TimedOut),
                FailureCase::InvalidTokens => assert!(matches!(
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
    async fn grok_device_login_times_out_when_budget_exhausted_while_pending() {
        let fixture = TestDirectory::new();
        let path = fixture.path.join("auth.json");
        let target = OAuth2LoginTarget::new(ProviderKind::Grok, "grok-cli", path.clone());
        let source = target.capture_version().unwrap();
        // A session whose authority budget covers exactly one poll interval.
        let transport = FakeTransport::pending_forever(Duration::from_secs(5));
        let time = FakeTime::default();

        let error = login_grok_with(
            &target,
            source,
            &transport,
            &time,
            &time,
            &REGISTRATION,
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error, OAuth2LoginError::TimedOut);
        // One completed wait then a rejected wait that no longer fits the remaining budget.
        assert_eq!(transport.poll_count(), 1);
        assert_eq!(time.delays().len(), 1);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn grok_device_login_rejects_token_pairs_with_disagreeing_subjects() {
        let fixture = TestDirectory::new();
        let path = fixture.path.join("auth.json");
        fs::write(&path, b"synthetic-previous-document").unwrap();
        let target = OAuth2LoginTarget::new(ProviderKind::Grok, "grok-cli", path.clone());
        let source = target.capture_version().unwrap();
        let transport = FakeTransport::subject_mismatch();
        let time = FakeTime::default();

        // Reject a token pair whose ID-token and access-token subjects disagree.
        let error = login_grok_with(
            &target,
            source,
            &transport,
            &time,
            &time,
            &REGISTRATION,
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            OAuth2LoginError::InvalidTokenBundle(
                OAuth2CredentialManagerError::AccountBindingMismatch
            )
        ));
        assert_eq!(fs::read(path).unwrap(), b"synthetic-previous-document");
    }

    #[derive(Clone, Copy)]
    enum FailureCase {
        Denied,
        Expired,
        InvalidTokens,
    }

    struct FakeTransport {
        polls: Mutex<VecDeque<Result<GrokTokenPoll, TransportError>>>,
        session_expires_in: Duration,
        poll_count: Mutex<usize>,
    }

    impl FakeTransport {
        fn successful() -> Self {
            Self {
                polls: Mutex::new(VecDeque::from([
                    Ok(GrokTokenPoll::Pending),
                    Ok(GrokTokenPoll::SlowDown),
                    Ok(GrokTokenPoll::Tokens(valid_tokens())),
                ])),
                session_expires_in: Duration::from_secs(1_800),
                poll_count: Mutex::new(0),
            }
        }

        fn failing(failure: FailureCase) -> Self {
            let poll = match failure {
                FailureCase::Denied => GrokTokenPoll::AccessDenied,
                FailureCase::Expired => GrokTokenPoll::ExpiredToken,
                FailureCase::InvalidTokens => GrokTokenPoll::Tokens(ExchangedTokens {
                    id_token: SecretString::from("synthetic-id".to_owned()),
                    access_token: SecretString::from("not-a-jwt".to_owned()),
                    refresh_token: SecretString::from("synthetic-refresh".to_owned()),
                }),
            };
            Self {
                polls: Mutex::new(VecDeque::from([Ok(poll)])),
                session_expires_in: Duration::from_secs(1_800),
                poll_count: Mutex::new(0),
            }
        }

        /// Issues one authorized poll whose token pair carries two disagreeing subjects.
        fn subject_mismatch() -> Self {
            let expiry = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3_600;
            Self {
                polls: Mutex::new(VecDeque::from([Ok(GrokTokenPoll::Tokens(
                    ExchangedTokens {
                        id_token: SecretString::from(jwt(r#"{"sub":"synthetic-subject"}"#)),
                        access_token: SecretString::from(jwt(&format!(
                            r#"{{"exp":{expiry},"sub":"synthetic-other-subject"}}"#
                        ))),
                        refresh_token: SecretString::from("synthetic-refresh".to_owned()),
                    },
                ))])),
                session_expires_in: Duration::from_secs(1_800),
                poll_count: Mutex::new(0),
            }
        }

        fn pending_forever(session_expires_in: Duration) -> Self {
            Self {
                polls: Mutex::new(VecDeque::new()),
                session_expires_in,
                poll_count: Mutex::new(0),
            }
        }

        fn poll_count(&self) -> usize {
            *self.poll_count.lock().unwrap()
        }
    }

    impl GrokOAuthTransport for FakeTransport {
        async fn request_device_session(&self) -> Result<GrokDeviceSession, TransportError> {
            Ok(GrokDeviceSession {
                device_code: SecretString::from("synthetic-device-code".to_owned()),
                user_code: SecretString::from("USER-CODE".to_owned()),
                expires_in: self.session_expires_in,
                interval: None,
            })
        }

        async fn poll_for_tokens(
            &self,
            _session: &GrokDeviceSession,
        ) -> Result<GrokTokenPoll, TransportError> {
            *self.poll_count.lock().unwrap() += 1;
            self.polls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(GrokTokenPoll::Pending))
        }
    }

    struct FakeTime {
        delays: Mutex<Vec<Duration>>,
        elapsed: Mutex<Duration>,
        base: Instant,
    }

    impl Default for FakeTime {
        fn default() -> Self {
            Self {
                delays: Mutex::new(Vec::new()),
                elapsed: Mutex::new(Duration::ZERO),
                base: Instant::now(),
            }
        }
    }

    impl FakeTime {
        fn delays(&self) -> Vec<Duration> {
            self.delays.lock().unwrap().clone()
        }
    }

    impl LoginSleeper for FakeTime {
        async fn sleep(&self, duration: Duration) {
            self.delays.lock().unwrap().push(duration);
            // Advance the virtual clock so deadline accounting stays deterministic.
            let mut elapsed = self.elapsed.lock().unwrap();
            *elapsed += duration;
        }
    }

    impl LoginClock for FakeTime {
        fn now(&self) -> Instant {
            // Return one stable virtual instant anchored at construction time.
            self.base + *self.elapsed.lock().unwrap()
        }
    }

    fn valid_tokens() -> ExchangedTokens {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3_600;
        ExchangedTokens {
            id_token: SecretString::from(jwt(r#"{"sub":"synthetic-subject"}"#)),
            access_token: SecretString::from(jwt(&format!(
                r#"{{"exp":{expiry},"sub":"synthetic-subject","tier":5}}"#
            ))),
            refresh_token: SecretString::from("synthetic-refresh".to_owned()),
        }
    }

    fn jwt(payload: &str) -> String {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
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
                "openbridge-oauth-grok-login-test-{}-{}",
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
