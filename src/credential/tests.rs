use super::*;
use axum::{
    Router,
    body::Bytes,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

pub(super) fn offline(path: &std::path::Path) -> Result<CredentialManager, CredentialError> {
    CredentialManager::new(path, builtin_drivers(None)?)
}
impl CredentialManager {
    pub(super) fn synthetic(root: &std::path::Path, origin: &str) -> Result<Self, CredentialError> {
        Self::new(
            root,
            vec![
                Arc::new(grok::GrokAuthority::synthetic(origin)?),
                Arc::new(codex::CodexAuthority::synthetic(origin)?),
            ],
        )
    }
    pub(super) async fn login_grok(
        &self,
        alias: &str,
        client: &str,
        notify: impl FnOnce(&DevicePrompt) + Send,
    ) -> Result<AccountStatus, CredentialError> {
        let notify = Mutex::new(Some(notify));
        self.login(
            "grok",
            alias,
            LoginOptions {
                client_id: Some(client.into()),
                ..LoginOptions::default()
            },
            |prompt| {
                let LoginPrompt::Device(prompt) = prompt else {
                    panic!("wrong method");
                };
                notify.lock().unwrap().take().unwrap()(prompt);
            },
        )
        .await
    }
    pub(super) async fn login_grok_browser(
        &self,
        alias: &str,
        client: &str,
        port: u16,
        notify: impl FnOnce(&BrowserPrompt) + Send,
    ) -> Result<AccountStatus, CredentialError> {
        let notify = Mutex::new(Some(notify));
        self.login(
            "grok",
            alias,
            LoginOptions {
                client_id: Some(client.into()),
                method: LoginMethod::Browser,
                callback_port: port,
            },
            |prompt| {
                let LoginPrompt::Browser(prompt) = prompt else {
                    panic!("wrong method");
                };
                notify.lock().unwrap().take().unwrap()(prompt);
            },
        )
        .await
    }
    pub(super) async fn login_codex(
        &self,
        alias: &str,
        notify: impl FnOnce(&DevicePrompt) + Send,
    ) -> Result<AccountStatus, CredentialError> {
        let notify = Mutex::new(Some(notify));
        self.login("codex", alias, LoginOptions::default(), |prompt| {
            let LoginPrompt::Device(prompt) = prompt else {
                panic!("wrong method");
            };
            notify.lock().unwrap().take().unwrap()(prompt);
        })
        .await
    }
}
const TEST_SCOPES: &str = "openid profile email offline_access grok-cli:access api:access";

pub(super) struct Directory(pub PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Self(std::env::temp_dir().join(format!("openbridge-auth-test-{name}")))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
pub(super) struct Step {
    pub path: &'static str,
    pub expected: Vec<(&'static str, &'static str)>,
    pub status: StatusCode,
    pub body: Value,
    pub gate: Option<Arc<Notify>>,
}
pub(super) fn step(
    path: &'static str,
    expected: Vec<(&'static str, &'static str)>,
    body: Value,
) -> Step {
    Step {
        path,
        expected,
        status: StatusCode::OK,
        body,
        gate: None,
    }
}
pub(super) struct FormCall {
    pub path: String,
    pub fields: std::collections::BTreeMap<String, String>,
}
pub(super) struct Authority {
    pub origin: String,
    pub steps: Arc<Mutex<VecDeque<Step>>>,
    pub entered: Arc<Notify>,
    pub arrivals: Arc<Mutex<Vec<tokio::time::Instant>>>,
    pub forms: Arc<Mutex<Vec<FormCall>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Authority {
    pub async fn new(steps: Vec<Step>) -> Self {
        let steps = Arc::new(Mutex::new(VecDeque::from(steps)));
        let entered = Arc::new(Notify::new());
        let arrivals = Arc::new(Mutex::new(Vec::new()));
        let forms = Arc::new(Mutex::new(Vec::new()));
        let handler = {
            let steps = steps.clone();
            let entered = entered.clone();
            let arrivals = arrivals.clone();
            let forms = forms.clone();
            move |request: axum::extract::Request| {
                let steps = steps.clone();
                let entered = entered.clone();
                let arrivals = arrivals.clone();
                let forms = forms.clone();
                async move {
                    arrivals.lock().unwrap().push(tokio::time::Instant::now());
                    let path = request.uri().path().to_owned();
                    let headers = request.headers().clone();
                    let body = axum::body::to_bytes(request.into_body(), 65536)
                        .await
                        .unwrap();
                    let next = steps
                        .lock()
                        .unwrap()
                        .pop_front()
                        .expect("unexpected authority request");
                    assert_eq!(path, next.path);
                    if path == "/oauth2/userinfo" {
                        assert_eq!(headers["authorization"], next.expected[0].1);
                    } else if path == "/.well-known/jwks.json" {
                        assert!(!headers.contains_key("authorization"));
                        assert!(!headers.contains_key("originator"));
                        assert!(body.is_empty());
                    } else if path.starts_with("/api/accounts/deviceauth")
                        || path == "/oauth/revoke"
                        || (path == "/oauth/token" && headers["content-type"] == "application/json")
                    {
                        assert_eq!(headers["content-type"], "application/json");
                        let actual: Value = serde_json::from_slice(&body).unwrap();
                        for (key, expected) in next.expected {
                            assert_eq!(actual[key].as_str(), Some(expected));
                        }
                        if path.starts_with("/oauth/") {
                            assert_eq!(headers["originator"], "codex_cli_rs");
                            let agent = headers["user-agent"].to_str().unwrap();
                            assert!(agent.starts_with("codex_cli_rs/0.160.0 "));
                            assert!(agent.contains("OpenBridge/"));
                        } else {
                            assert!(!headers.contains_key("originator"));
                            assert!(
                                headers["user-agent"]
                                    .to_str()
                                    .unwrap()
                                    .starts_with("OpenBridge/")
                            );
                        }
                    } else {
                        assert_eq!(headers["content-type"], "application/x-www-form-urlencoded");
                        assert!(!headers.contains_key("originator"));
                        let actual: std::collections::BTreeMap<_, _> =
                            url::form_urlencoded::parse(&body).into_owned().collect();
                        forms.lock().unwrap().push(FormCall {
                            path: path.clone(),
                            fields: actual.clone(),
                        });
                        for (key, expected) in next.expected {
                            assert_eq!(actual.get(key).map(String::as_str), Some(expected));
                        }
                    }
                    entered.notify_one();
                    if let Some(gate) = next.gate {
                        gate.notified().await;
                    }
                    (next.status, axum::Json(next.body))
                }
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/oauth2/device/code", post(handler.clone()))
            .route("/oauth2/token", post(handler.clone()))
            .route("/oauth2/userinfo", get(handler.clone()))
            .route("/oauth2/revoke", post(handler.clone()))
            .route("/api/accounts/deviceauth/usercode", post(handler.clone()))
            .route("/api/accounts/deviceauth/token", post(handler.clone()))
            .route("/oauth/token", post(handler.clone()))
            .route("/oauth/revoke", post(handler.clone()))
            .route("/.well-known/jwks.json", get(handler));
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            steps,
            entered,
            arrivals,
            forms,
            task,
        }
    }
    pub fn done(&self) {
        assert!(self.steps.lock().unwrap().is_empty());
    }
    pub fn pool(&self, dir: &Directory) -> CredentialManager {
        CredentialManager::synthetic(&dir.0, &self.origin).unwrap()
    }
}
impl Drop for Authority {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn device() -> Step {
    step(
        "/oauth2/device/code",
        vec![
            ("client_id", "synthetic-client"),
            ("referrer", "openbridge"),
            ("scope", TEST_SCOPES),
        ],
        json!({"device_code":"secret-device", "user_code":"ABCD-1234",
        "verification_uri":"https://accounts.x.ai/device", "expires_in":60, "interval":1}),
    )
}
fn token(access: &str, refresh: Option<&str>) -> Value {
    let mut body = json!({"access_token":access, "expires_in":3600, "token_type":"Bearer", "scope":TEST_SCOPES});
    if let Some(refresh) = refresh {
        body["refresh_token"] = json!(refresh);
    }
    body
}
pub(super) fn login_steps(subject: &str) -> Vec<Step> {
    vec![
        device(),
        step(
            "/oauth2/token",
            vec![
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", "synthetic-client"),
                ("device_code", "secret-device"),
            ],
            token("synthetic-access", Some("synthetic-refresh")),
        ),
        step(
            "/oauth2/userinfo",
            vec![("authorization", "Bearer synthetic-access")],
            json!({"sub":subject}),
        ),
    ]
}
pub(super) async fn login(pool: &CredentialManager, account: &str) -> AccountStatus {
    pool.login_grok(account, "synthetic-client", |prompt| {
        assert_eq!(prompt.user_code, "ABCD-1234");
        assert_eq!(prompt.verification_uri, "https://accounts.x.ai/device");
        assert!(!format!("{prompt:?}").contains("secret-device"));
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn pool_persists_isolated_accounts_and_local_logout() {
    let mut steps = login_steps("person-a");
    steps.extend(login_steps("person-b"));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    assert!(pool.list(None).unwrap().is_empty());
    assert_eq!(login(&pool, "alice").await.state, AccountState::Active);
    login(&pool, "bob").await;
    let reopened = authority.pool(&dir);
    assert_eq!(reopened.list(None).unwrap().len(), 2);
    assert_eq!(
        reopened.logout("grok", "alice", false).await.unwrap(),
        LogoutOutcome::LocalOnly
    );
    let entries = reopened.list(None).unwrap();
    assert_eq!(entries[0].state, AccountState::SignedOut);
    assert_eq!(entries[1].state, AccountState::Active);
    let output = serde_json::to_string(&entries).unwrap();
    for secret in [
        "synthetic-access",
        "synthetic-refresh",
        "person-a",
        "person-b",
    ] {
        assert!(!output.contains(secret));
    }
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(dir.0.join("grok/alice.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    authority.done();
}

#[tokio::test]
async fn rotation_and_omitted_replacement_use_account_bound_refresh() {
    let mut steps = login_steps("person-a");
    steps.extend([
        step(
            "/oauth2/token",
            vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", "synthetic-refresh"),
                ("client_id", "synthetic-client"),
            ],
            token("new-access", Some("rotated-refresh")),
        ),
        step(
            "/oauth2/userinfo",
            vec![("authorization", "Bearer new-access")],
            json!({"sub":"person-a"}),
        ),
        step(
            "/oauth2/token",
            vec![("refresh_token", "rotated-refresh")],
            token("final-access", None),
        ),
        step(
            "/oauth2/userinfo",
            vec![("authorization", "Bearer final-access")],
            json!({"sub":"person-a"}),
        ),
        step(
            "/oauth2/revoke",
            vec![
                ("token", "rotated-refresh"),
                ("token_type_hint", "refresh_token"),
                ("client_id", "synthetic-client"),
            ],
            json!({}),
        ),
    ]);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let first = pool.refresh("grok", "alice").await.unwrap();
    let second = pool.refresh("grok", "alice").await.unwrap();
    assert!(second.generation > first.generation);
    assert_eq!(
        pool.logout("grok", "alice", true).await.unwrap(),
        LogoutOutcome::Revoked
    );
    assert_eq!(pool.list(None).unwrap()[0].state, AccountState::SignedOut);
    authority.done();
}

#[tokio::test]
async fn failed_or_cancelled_login_preserves_existing_session_and_logout_supersedes_it() {
    let mut steps = login_steps("person-a");
    steps.push(device());
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let mut future = Box::pin(pool.login_grok("alice", "synthetic-client", |_| {}));
    tokio::select! { _ = &mut future => panic!("must await device approval"),
    _ = authority.entered.notified() => {} }
    // Previous requests can leave one notification; drive until the device step was consumed.
    while !authority.steps.lock().unwrap().is_empty() {
        tokio::select! { _ = &mut future => panic!("must await approval"),
        _ = authority.entered.notified() => {} }
    }
    assert_eq!(pool.list(None).unwrap()[0].state, AccountState::Active);
    pool.logout("grok", "alice", false).await.unwrap();
    drop(future);
    assert_eq!(pool.list(None).unwrap()[0].state, AccountState::SignedOut);
    authority.done();
}

#[tokio::test]
async fn identity_change_and_scope_loss_never_publish_credentials() {
    let mut steps = login_steps("person-a");
    steps.extend(login_steps("person-b"));
    let mut denied = token("lost-scope", Some("new-refresh"));
    denied["scope"] = json!("openid offline_access");
    steps.push(step(
        "/oauth2/token",
        vec![("refresh_token", "synthetic-refresh")],
        denied,
    ));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    let before = login(&pool, "alice").await;
    assert_eq!(
        pool.login_grok("alice", "synthetic-client", |_| {})
            .await
            .unwrap_err(),
        CredentialError::IdentityMismatch
    );
    assert_eq!(pool.list(None).unwrap()[0].expires_at, before.expires_at);
    assert_eq!(
        pool.refresh("grok", "alice").await.unwrap_err(),
        CredentialError::Permission
    );
    assert_eq!(
        pool.list(None).unwrap()[0].state,
        AccountState::NeedsReauthorization
    );
    assert_eq!(
        pool.refresh("grok", "alice").await.unwrap_err(),
        CredentialError::LoginRequired
    );
    authority.done();
}

#[tokio::test]
async fn cancelled_rotation_and_lock_contention_fail_closed_across_pool_handles() {
    let gate = Arc::new(Notify::new());
    let mut steps = login_steps("person-a");
    let mut rotation = step(
        "/oauth2/token",
        vec![("refresh_token", "synthetic-refresh")],
        token("new-access", Some("new-refresh")),
    );
    rotation.gate = Some(gate.clone());
    steps.push(rotation);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let other = authority.pool(&dir);
    let mut future = Box::pin(pool.refresh("grok", "alice"));
    while !authority.steps.lock().unwrap().is_empty() {
        tokio::select! { _ = &mut future => panic!("rotation blocked by server"),
        _ = authority.entered.notified() => {} }
    }
    assert_eq!(
        other.refresh("grok", "alice").await.unwrap_err(),
        CredentialError::Busy
    );
    drop(future);
    gate.notify_one();
    assert_eq!(
        other.list(None).unwrap()[0].state,
        AccountState::NeedsReauthorization
    );
    assert_eq!(
        other.refresh("grok", "alice").await.unwrap_err(),
        CredentialError::LoginRequired
    );
    authority.done();
}

#[tokio::test]
async fn failed_rotation_publication_leaves_durable_uncertainty() {
    let mut steps = login_steps("person-a");
    steps.extend([
        step(
            "/oauth2/token",
            vec![],
            token("new-access", Some("new-refresh")),
        ),
        step(
            "/oauth2/userinfo",
            vec![("authorization", "Bearer new-access")],
            json!({"sub":"person-a"}),
        ),
    ]);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let mut refresh = Box::pin(pool.refresh("grok", "alice"));
    while authority.steps.lock().unwrap().len() > 1 {
        tokio::select! { _ = &mut refresh => panic!("must await identity"), _ = authority.entered.notified() => {} }
    }
    pool.store.fail_at(store::PublishStep::Rename);
    assert_eq!(refresh.await.unwrap_err(), CredentialError::Storage);
    assert_eq!(
        authority.pool(&dir).list(None).unwrap()[0].state,
        AccountState::NeedsReauthorization
    );
    authority.done();
}

#[tokio::test]
async fn revocation_failure_still_clears_only_local_session() {
    let mut steps = login_steps("person-a");
    let mut revoke = step(
        "/oauth2/revoke",
        vec![("token", "synthetic-refresh")],
        json!({"error_description":"secret-upstream-body"}),
    );
    revoke.status = StatusCode::BAD_GATEWAY;
    steps.push(revoke);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    assert_eq!(
        pool.logout("grok", "alice", true).await.unwrap(),
        LogoutOutcome::RevocationUnconfirmed
    );
    assert_eq!(pool.list(None).unwrap()[0].state, AccountState::SignedOut);
    authority.done();
}

#[test]
fn store_rejects_symlinks_insecure_permissions_and_invalid_aliases() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = Directory::new();
    let pool = super::tests::offline(&dir.0).unwrap();
    assert_eq!(
        pool.store.account_lock("grok", "../escape").unwrap_err(),
        CredentialError::InvalidInput
    );
    std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        super::tests::offline(&dir.0),
        Err(CredentialError::Storage)
    ));
    std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    let outside = Directory::new();
    std::fs::create_dir(&outside.0).unwrap();
    std::fs::write(outside.0.join("private"), "do not read").unwrap();
    drop(pool.store.account_lock("grok", "alice").unwrap());
    symlink(outside.0.join("private"), dir.0.join("grok/alice.json")).unwrap();
    assert_eq!(pool.list(None).unwrap_err(), CredentialError::Storage);
    assert_eq!(
        std::fs::read_to_string(outside.0.join("private")).unwrap(),
        "do not read"
    );
}

#[tokio::test]
async fn cancelled_old_ticket_cannot_clear_a_new_login_attempt() {
    let authority = Authority::new(vec![device(), device()]).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    let (sent, received) = tokio::sync::oneshot::channel();
    let mut first = Box::pin(pool.login_grok("alice", "synthetic-client", |_| {
        sent.send(()).unwrap();
    }));
    tokio::select! { _ = &mut first => panic!("must await approval"), result = received => result.unwrap() }
    let (sent, received) = tokio::sync::oneshot::channel();
    let mut second = Box::pin(pool.login_grok("alice", "synthetic-client", |_| {
        sent.send(()).unwrap();
    }));
    tokio::select! { _ = &mut second => panic!("must await approval"), result = received => result.unwrap() }
    drop(first);
    let status = pool.list(None).unwrap().remove(0);
    assert!(status.login_pending);
    assert_eq!(status.generation, 0);
    drop(second);
    assert!(!pool.list(None).unwrap()[0].login_pending);
    authority.done();
}

#[test]
fn verification_and_poll_intervals_are_not_generic_oauth_fallbacks() {
    for uri in [
        "https://evil.test/device",
        "https://auth.x.ai@evil.test/",
        "http://accounts.x.ai/",
        "https://accounts.x.ai:444/",
        "https://accounts.x.ai/device?device_code=secret",
        "https://accounts.x.ai/#secret",
    ] {
        assert_eq!(
            grok::verification_uri(uri).unwrap_err(),
            CredentialError::Protocol
        );
    }
    assert!(grok::verification_uri("https://auth.x.ai/device").is_ok());
    assert_eq!(
        grok::next_interval(std::time::Duration::from_secs(5))
            .unwrap()
            .as_secs(),
        10
    );
}

#[tokio::test]
async fn authority_rejects_redirects_oversized_bodies_and_leaked_errors() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route(
            "/oauth2/device/code",
            post(|| async {
                (
                    StatusCode::FOUND,
                    [("location", "https://evil.test/token")],
                    "secret-server-body",
                )
            }),
        )
        .route(
            "/oauth2/token",
            post(|| async { Bytes::from(vec![b'x'; 65537]) }),
        );
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let auth = grok::GrokAuthority::synthetic(&origin).unwrap();
    assert_eq!(
        auth.device("client").await.unwrap_err(),
        CredentialError::Protocol
    );
    assert_eq!(
        auth.refresh("client", "refresh", TEST_SCOPES)
            .await
            .unwrap_err(),
        CredentialError::Protocol
    );
    task.abort();
}

#[tokio::test]
async fn polling_waits_before_first_attempt_and_honors_pending_and_slow_down() {
    let mut steps = vec![device()];
    for error in ["authorization_pending", "slow_down"] {
        let mut response = step(
            "/oauth2/token",
            vec![("device_code", "secret-device")],
            json!({"error":error}),
        );
        response.status = StatusCode::BAD_REQUEST;
        steps.push(response);
    }
    steps.extend(login_steps("person-a").into_iter().skip(1));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let arrivals = authority.arrivals.lock().unwrap();
    assert!(arrivals[1].duration_since(arrivals[0]).as_secs() >= 1);
    assert!(arrivals[2].duration_since(arrivals[1]).as_secs() >= 1);
    assert!(arrivals[3].duration_since(arrivals[2]).as_secs() >= 6);
    authority.done();
}

#[tokio::test]
async fn polling_denial_expiry_and_invalid_grant_are_terminal() {
    for (error, expected) in [
        ("access_denied", CredentialError::Denied),
        ("expired_token", CredentialError::Expired),
        ("invalid_grant", CredentialError::InvalidGrant),
    ] {
        let mut response = step(
            "/oauth2/token",
            vec![],
            json!({"error":error,
            "error_description":"sensitive-upstream-detail"}),
        );
        response.status = StatusCode::BAD_REQUEST;
        let authority = Authority::new(vec![device(), response]).await;
        let dir = Directory::new();
        let pool = authority.pool(&dir);
        let failure = pool
            .login_grok("alice", "synthetic-client", |_| {})
            .await
            .unwrap_err();
        assert_eq!(failure, expected);
        assert!(!failure.to_string().contains("sensitive-upstream-detail"));
        assert_eq!(pool.list(None).unwrap()[0].state, AccountState::SignedOut);
        assert!(!pool.list(None).unwrap()[0].login_pending);
        authority.done();
    }
}

#[tokio::test]
async fn expired_device_never_polls_after_its_deadline() {
    let mut response = device();
    response.body["expires_in"] = json!(1);
    let authority = Authority::new(vec![response]).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    assert_eq!(
        pool.login_grok("alice", "synthetic-client", |_| {})
            .await
            .unwrap_err(),
        CredentialError::Expired
    );
    authority.done();
}

#[tokio::test]
async fn complete_old_login_cannot_override_a_logout() {
    let mut steps = login_steps("person-a");
    steps.extend(login_steps("person-a"));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool, "alice").await;
    let mut future = Box::pin(pool.login_grok("alice", "synthetic-client", |_| {}));
    while authority.steps.lock().unwrap().len() > 2 {
        tokio::select! { _ = &mut future => panic!("approval not received"),
        _ = authority.entered.notified() => {} }
    }
    pool.logout("grok", "alice", false).await.unwrap();
    assert_eq!(future.await.unwrap_err(), CredentialError::Superseded);
    assert_eq!(pool.list(None).unwrap()[0].state, AccountState::SignedOut);
    authority.done();
}

#[tokio::test]
async fn null_optional_fields_are_not_treated_as_absent() {
    for field in ["refresh_token", "scope"] {
        let mut body = token("synthetic-access", Some("replacement"));
        body[field] = Value::Null;
        let authority = Authority::new(vec![step("/oauth2/token", vec![], body)]).await;
        let auth = grok::GrokAuthority::synthetic(&authority.origin).unwrap();
        assert_eq!(
            auth.refresh("client", "previous", TEST_SCOPES)
                .await
                .unwrap_err(),
            CredentialError::Protocol
        );
        authority.done();
    }
}
