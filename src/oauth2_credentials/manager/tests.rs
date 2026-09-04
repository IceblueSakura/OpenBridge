use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use tokio::sync::Notify;
use zeroize::Zeroizing;

use super::{
    OAuth2CredentialManager, OAuth2CredentialManagerBuilder, OAuth2CredentialStatus,
    OAuth2RefreshOutcome,
    credential::{REFRESH_SAFETY_WINDOW, refresh_due_at},
};
use crate::oauth2_credentials::document::parse_auth_document;
use crate::oauth2_credentials::transport::refresh::{
    OAuth2RefreshTransport, RefreshTerminalReason, RefreshTokenResponse, RefreshTransportError,
};
use crate::provider::ProviderKind;

#[test]
fn managed_snapshot_retains_complete_tokens_without_exposing_them_through_debug() {
    let fixture = TestDirectory::new();
    let manager = fixture.manager(expiring_document(
        "synthetic-account",
        unix_now() + 3_600,
        "synthetic-refresh",
    ));

    // Confirm the private state owns the complete lifecycle bundle.
    let managed = manager.find_credential(ProviderKind::ChatGpt).unwrap();
    let state = managed.lock_state();
    assert_eq!(
        state.bundle.refresh_token.expose_secret(),
        "synthetic-refresh"
    );
    assert_eq!(chatgpt_account_id(&state.bundle), "synthetic-account");
    drop(state);

    // Keep every secret and locator out of manager and snapshot diagnostics.
    let snapshot = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .unwrap();
    let debug = format!("{manager:?} {snapshot:?}");
    for forbidden in [
        "synthetic-refresh",
        "synthetic-account",
        "sensitive-auth-file",
    ] {
        assert!(!debug.contains(forbidden));
    }
}

#[tokio::test]
async fn concurrent_refreshes_reload_once_and_publish_one_rotated_generation() {
    let fixture = TestDirectory::new();
    let manager = Arc::new(fixture.manager(expiring_document(
        "synthetic-account",
        1,
        "synthetic-old-refresh",
    )));
    let transport = Arc::new(BlockingTransport::new(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
        id_token: None,
        refresh_token: Some(SecretString::from("synthetic-rotated-refresh".to_owned())),
        token_type: Some("Bearer".to_owned()),
    }));
    let now = SystemTime::now();

    // Hold the first network exchange while a second caller queues behind the in-process gate.
    let first_manager = Arc::clone(&manager);
    let first_transport = Arc::clone(&transport);
    let first = tokio::spawn(async move {
        let credential = first_manager
            .find_credential(ProviderKind::ChatGpt)
            .unwrap();
        first_manager
            .refresh_provider_with(credential, &*first_transport, now)
            .await
    });
    transport.started.notified().await;
    let second_manager = Arc::clone(&manager);
    let second_transport = Arc::clone(&transport);
    let second = tokio::spawn(async move {
        let credential = second_manager
            .find_credential(ProviderKind::ChatGpt)
            .unwrap();
        second_manager
            .refresh_provider_with(credential, &*second_transport, now)
            .await
    });
    transport.release.notify_one();
    let (first, second) = (first.await.unwrap(), second.await.unwrap());

    // Publish one rotated generation and let the queued caller reuse the persisted winner.
    assert_eq!(first, OAuth2RefreshOutcome::Refreshed { generation: 2 });
    assert_eq!(second, OAuth2RefreshOutcome::Current { generation: 2 });
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
    let snapshot = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .unwrap();
    assert_eq!(snapshot.metadata().generation(), 2);
    assert_eq!(snapshot.status(), OAuth2CredentialStatus::Active);
}

#[tokio::test]
async fn refresh_preserves_optional_tokens_and_rejects_account_changes() {
    // Preserve both optional tokens when the authority returns only a new access token.
    let fixture = TestDirectory::new();
    let manager = fixture.manager(expiring_document(
        "synthetic-account",
        1,
        "synthetic-preserved-refresh",
    ));
    let transport = ImmediateTransport::success(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
        id_token: None,
        refresh_token: None,
        token_type: None,
    });
    let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
    let outcome = manager
        .refresh_provider_with(credential, &transport, SystemTime::now())
        .await;
    assert_eq!(outcome, OAuth2RefreshOutcome::Refreshed { generation: 2 });
    let document = Zeroizing::new(fs::read(fixture.auth_file()).unwrap());
    let persisted = parse_auth_document(ProviderKind::ChatGpt, &document, true).unwrap();
    assert_eq!(
        persisted.refresh_token.expose_secret(),
        "synthetic-preserved-refresh"
    );
    assert_eq!(chatgpt_account_id(&persisted), "synthetic-account");

    // Reject a successful response whose replacement ID token changes account identity.
    let fixture = TestDirectory::new();
    let original = expiring_document("synthetic-account", 1, "synthetic-refresh");
    let manager = fixture.manager(original.clone());
    let transport = ImmediateTransport::success(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({"exp": unix_now() + 3_600}))),
        id_token: Some(SecretString::from(id_token("synthetic-other-account"))),
        refresh_token: None,
        token_type: Some("Bearer".to_owned()),
    });
    let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
    let outcome = manager
        .refresh_provider_with(credential, &transport, SystemTime::now())
        .await;
    assert_eq!(outcome, OAuth2RefreshOutcome::Ambiguous { generation: 1 });
    assert_eq!(fs::read(fixture.auth_file()).unwrap(), original);
}

#[tokio::test]
async fn grok_refresh_preserves_optional_tokens_and_subscription_tier() {
    // Preserve id/refresh tokens and the persisted tier when the authority omits all three.
    let fixture = TestDirectory::new();
    let original = grok_document("synthetic-subject", 1);
    let manager = fixture.manager_for(ProviderKind::Grok, "grok-cli", original.clone());
    let transport = ImmediateTransport::success(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({
            "exp": unix_now() + 3_600,
            "sub": "synthetic-subject"
        }))),
        id_token: None,
        refresh_token: None,
        token_type: None,
    });
    let credential = manager.find_credential(ProviderKind::Grok).unwrap();
    let outcome = manager
        .refresh_provider_with(credential, &transport, SystemTime::now())
        .await;
    assert_eq!(outcome, OAuth2RefreshOutcome::Refreshed { generation: 2 });
    let document = Zeroizing::new(fs::read(fixture.auth_file()).unwrap());
    let persisted = parse_auth_document(ProviderKind::Grok, &document, true).unwrap();
    assert_eq!(
        persisted.refresh_token.expose_secret(),
        "synthetic-preserved-refresh"
    );
    let super::super::document::OAuth2AccountContext::Grok {
        subject,
        subscription_tier,
    } = &persisted.context
    else {
        panic!("expected a Grok account context");
    };
    assert_eq!(subject.expose_secret(), "synthetic-subject");
    // The refreshed access token omits the tier claim, so the stored tier is retained.
    assert_eq!(subscription_tier, "supergrok_heavy");
}

#[tokio::test]
async fn grok_refresh_rejects_rotated_access_tokens_for_another_subject() {
    // A rotated access token that carries a different subject must never replace the bundle.
    let fixture = TestDirectory::new();
    let original = grok_document("synthetic-subject", 1);
    let manager = fixture.manager_for(ProviderKind::Grok, "grok-cli", original.clone());
    let transport = ImmediateTransport::success(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({
            "exp": unix_now() + 3_600,
            "sub": "synthetic-other-subject"
        }))),
        id_token: None,
        refresh_token: None,
        token_type: Some("Bearer".to_owned()),
    });
    let credential = manager.find_credential(ProviderKind::Grok).unwrap();
    let outcome = manager
        .refresh_provider_with(credential, &transport, SystemTime::now())
        .await;
    assert_eq!(outcome, OAuth2RefreshOutcome::Ambiguous { generation: 1 });
    assert_eq!(fs::read(fixture.auth_file()).unwrap(), original);
}

#[tokio::test]
async fn unauthorized_current_generation_forces_one_refresh_and_stale_callers_reuse_it() {
    let fixture = TestDirectory::new();
    let manager = fixture.manager(expiring_document(
        "synthetic-account",
        unix_now() + 3_600,
        "synthetic-refresh",
    ));
    let transport = ImmediateTransport::success(RefreshTokenResponse {
        access_token: SecretString::from(jwt(json!({"exp": unix_now() + 7_200}))),
        id_token: None,
        refresh_token: None,
        token_type: Some("Bearer".to_owned()),
    });
    let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();

    // Force rotation even though the rejected generation is outside the expiry safety window.
    let first = manager
        .recover_after_unauthorized_with(credential, &transport, SystemTime::now(), 1)
        .await;
    assert_eq!(first, OAuth2RefreshOutcome::Refreshed { generation: 2 });
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);

    // Merge a stale concurrent 401 into the already published generation without a second grant.
    let second = manager
        .recover_after_unauthorized_with(credential, &transport, SystemTime::now(), 1)
        .await;
    assert_eq!(second, OAuth2RefreshOutcome::Current { generation: 2 });
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn terminal_ambiguous_and_transient_refresh_failures_are_fail_closed() {
    let cases = [
        (
            RefreshTransportError::Transient { retry_after: None },
            OAuth2CredentialStatus::RefreshBackoff,
        ),
        (
            RefreshTransportError::ReauthRequired(RefreshTerminalReason::InvalidGrant),
            OAuth2CredentialStatus::ReauthRequired,
        ),
        (
            RefreshTransportError::Ambiguous,
            OAuth2CredentialStatus::Ambiguous,
        ),
    ];

    // Map each value-free transport category to the corresponding scheduler state.
    for (error, expected_status) in cases {
        let fixture = TestDirectory::new();
        let manager = fixture.manager(expiring_document(
            "synthetic-account",
            1,
            "synthetic-refresh",
        ));
        let credential = manager.find_credential(ProviderKind::ChatGpt).unwrap();
        let outcome = manager
            .refresh_provider_with(
                credential,
                &ImmediateTransport::failure(error),
                SystemTime::now(),
            )
            .await;
        assert_eq!(
            manager
                .credential_for_provider(ProviderKind::ChatGpt)
                .unwrap()
                .status(),
            expected_status
        );
        assert!(!matches!(outcome, OAuth2RefreshOutcome::Refreshed { .. }));
    }
}

#[test]
fn scheduler_due_time_uses_expiry_window_and_deterministic_early_jitter() {
    let expiry = UNIX_EPOCH + Duration::from_secs(10_000);

    // Derive a stable due time strictly before the safety-window boundary and expiry.
    let first = refresh_due_at("chatgpt-codex", expiry);
    let second = refresh_due_at("chatgpt-codex", expiry);
    assert_eq!(first, second);
    assert!(first <= expiry - REFRESH_SAFETY_WINDOW);
    assert!(first >= expiry - REFRESH_SAFETY_WINDOW - Duration::from_secs(30));
    assert!(first < expiry);
}

struct BlockingTransport {
    response: Mutex<Option<RefreshTokenResponse>>,
    calls: AtomicUsize,
    started: Notify,
    release: Notify,
}

impl BlockingTransport {
    fn new(response: RefreshTokenResponse) -> Self {
        Self {
            response: Mutex::new(Some(response)),
            calls: AtomicUsize::new(0),
            started: Notify::new(),
            release: Notify::new(),
        }
    }
}

impl OAuth2RefreshTransport for BlockingTransport {
    async fn refresh(
        &self,
        _refresh_token: &SecretString,
    ) -> Result<RefreshTokenResponse, RefreshTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        self.release.notified().await;
        Ok(self.response.lock().unwrap().take().unwrap())
    }
}

struct ImmediateTransport {
    result: Mutex<Option<Result<RefreshTokenResponse, RefreshTransportError>>>,
    calls: AtomicUsize,
}

impl ImmediateTransport {
    fn success(response: RefreshTokenResponse) -> Self {
        Self {
            result: Mutex::new(Some(Ok(response))),
            calls: AtomicUsize::new(0),
        }
    }

    fn failure(error: RefreshTransportError) -> Self {
        Self {
            result: Mutex::new(Some(Err(error))),
            calls: AtomicUsize::new(0),
        }
    }
}

impl OAuth2RefreshTransport for ImmediateTransport {
    async fn refresh(
        &self,
        _refresh_token: &SecretString,
    ) -> Result<RefreshTokenResponse, RefreshTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.lock().unwrap().take().unwrap()
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openbridge-oauth-manager-test-{}-{}",
            std::process::id(),
            crate::oauth2_credentials::storage::next_test_id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn auth_file(&self) -> PathBuf {
        self.path.join("sensitive-auth-file.json")
    }

    fn manager(&self, document: Vec<u8>) -> OAuth2CredentialManager {
        self.manager_for(ProviderKind::ChatGpt, "chatgpt-codex", document)
    }

    /// Builds one manager bound to the selected Provider and pool.
    fn manager_for(
        &self,
        provider: ProviderKind,
        pool_id: &str,
        document: Vec<u8>,
    ) -> OAuth2CredentialManager {
        fs::write(self.auth_file(), document).unwrap();
        let mut builder = OAuth2CredentialManagerBuilder::new();
        builder
            .load_auth_json_file(provider, pool_id, self.auth_file())
            .unwrap();
        builder.build()
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

/// Extracts the ChatGPT account binding from a validated bundle's context variant.
fn chatgpt_account_id(bundle: &super::super::document::ValidatedOAuth2Bundle) -> String {
    let super::super::document::OAuth2AccountContext::ChatGpt { account_id, .. } = &bundle.context
    else {
        panic!("expected a ChatGPT account context");
    };
    account_id.expose_secret().to_owned()
}

fn expiring_document(account: &str, expiry: u64, refresh_token: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": id_token(account),
            "access_token": jwt(json!({"exp": expiry})),
            "refresh_token": refresh_token,
            "account_id": account
        },
        "last_refresh": "2026-08-05T00:00:00Z"
    }))
    .unwrap()
}

fn id_token(account: &str) -> String {
    jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account,
            "chatgpt_account_is_fedramp": false
        }
    }))
}

/// Builds one expiring Grok auth document with a subject-bound token pair and stored tier.
fn grok_document(subject: &str, expiry: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "auth_mode": "grok",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": jwt(json!({"sub": subject})),
            "access_token": jwt(json!({
                "exp": expiry,
                "sub": subject,
                "tier": 5
            })),
            "refresh_token": "synthetic-preserved-refresh"
        },
        "last_refresh": "2026-08-05T00:00:00Z",
        "subscription_tier": "supergrok_heavy"
    }))
    .unwrap()
}

fn jwt(payload: serde_json::Value) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(b"{}"),
        URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes()),
        URL_SAFE_NO_PAD.encode(b"signature")
    )
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
