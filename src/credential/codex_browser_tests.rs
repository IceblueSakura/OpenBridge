//! Independent product browser wire and lifecycle; never contacts OpenAI.
use super::browser_tests::request;
use super::codex_tests::{claims, id_token, keys, sign};
use super::tests::{Authority, Directory, Step, step};
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde_json::{Value, json};
use std::{collections::BTreeMap, net::SocketAddr, sync::Mutex, time::Duration};
use tokio::{
    net::TcpListener,
    time::{Instant, timeout},
};

const CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
fn options() -> LoginOptions {
    LoginOptions {
        method: LoginMethod::Browser,
        ..LoginOptions::default()
    }
}
fn token(nonce: Option<&str>, workspace: &str) -> Value {
    let mut claim = claims(workspace);
    if let Some(nonce) = nonce {
        claim["nonce"] = json!(nonce);
    }
    json!({"access_token":"browser-access", "refresh_token":"browser-refresh",
        "id_token":sign(json!({"alg":"RS256","kid":"synthetic-codex-key"}),claim)})
}
fn steps() -> Vec<Step> {
    vec![
        step(
            "/oauth/token",
            vec![
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT),
                ("code", "browser-code"),
            ],
            json!({}),
        ),
        step("/.well-known/jwks.json", vec![], keys()),
    ]
}
fn inspect(prompt: &BrowserPrompt) -> (SocketAddr, BTreeMap<String, String>) {
    let url = url::Url::parse(&prompt.authorization_url).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://auth.openai.com"
    );
    assert_eq!(url.path(), "/oauth/authorize");
    let fields: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(fields.len(), 11);
    assert_eq!(fields["client_id"], CLIENT);
    assert_eq!(fields["scope"], "openid profile email offline_access");
    assert_eq!(fields["response_type"], "code");
    assert_eq!(fields["code_challenge_method"], "S256");
    assert_eq!(fields["redirect_uri"], prompt.redirect_uri);
    assert_eq!(fields["id_token_add_organizations"], "true");
    assert_eq!(fields["codex_cli_simplified_flow"], "true");
    assert_eq!(fields["originator"], "codex_cli_rs");
    assert_ne!(fields["state"], fields["nonce"]);
    let redirect = url::Url::parse(&prompt.redirect_uri).unwrap();
    assert_eq!(redirect.scheme(), "http");
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert_eq!(redirect.path(), "/auth/callback");
    assert!(redirect.query().is_none() && redirect.fragment().is_none());
    (
        SocketAddr::from(([127, 0, 0, 1], redirect.port().unwrap())),
        fields,
    )
}
async fn browser_login(
    manager: &CredentialManager,
    authority: &Authority,
    nonce_mode: u8,
    workspace: &str,
    supersede: bool,
) -> Result<AccountStatus, CredentialError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let mut login = Box::pin(manager.login("codex", "personal", options(), |prompt| {
        let LoginPrompt::Browser(prompt) = prompt else {
            panic!("browser must not fall back to device");
        };
        sender
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(prompt.clone())
            .unwrap();
    }));
    let prompt = timeout(Duration::from_secs(3), async {
        tokio::select! { result = &mut login => panic!("must wait for callback: {result:?}"), prompt = receiver => prompt.unwrap() }
    }).await.unwrap();
    let (address, fields) = inspect(&prompt);
    let nonce = match nonce_mode {
        0 => Some(fields["nonce"].as_str()),
        1 => Some("wrong-nonce"),
        _ => None,
    };
    authority.steps.lock().unwrap().front_mut().unwrap().body = token(nonce, workspace);
    if supersede {
        manager.logout("codex", "personal", false).await.unwrap();
    }
    let target = format!("/auth/callback?state={}&code=browser-code", fields["state"]);
    let callbacks = async {
        // An unrelated error or the other profile's callback path cannot consume this code.
        let wrong = request(
            address,
            "/auth/callback?state=wrong&error=access_denied&error_description=private",
        )
        .await;
        assert!(wrong.starts_with("HTTP/1.1 400") && !wrong.contains("private"));
        let wrong_path = request(
            address,
            &format!("/callback?state={}&code=browser-code", fields["state"]),
        )
        .await;
        assert!(wrong_path.starts_with("HTTP/1.1 404"));
        request(address, &target).await
    };
    let (result, response) = timeout(Duration::from_secs(5), async {
        tokio::join!(&mut login, callbacks)
    })
    .await
    .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("Check the CLI"));
    assert!(!response.contains("browser-code"));
    {
        let calls = authority.forms.lock().unwrap();
        let exchange = calls
            .iter()
            .rfind(|call| call.path == "/oauth/token")
            .unwrap();
        assert_eq!(exchange.fields.len(), 5);
        assert_eq!(exchange.fields["redirect_uri"], prompt.redirect_uri);
        let verifier = &exchange.fields["code_verifier"];
        let digest = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
        assert_eq!(B64.encode(digest.as_ref()), fields["code_challenge"]);
        assert!(!prompt.authorization_url.contains(verifier));
    }
    drop(TcpListener::bind(address).await.unwrap());
    result
}

#[tokio::test]
async fn browser_login_refresh_and_revoke_use_one_bound_file_session() {
    let mut wire = steps();
    wire.extend([
        step("/oauth/token",vec![("grant_type","refresh_token"),("client_id",CLIENT),("refresh_token","browser-refresh")],
            json!({"access_token":"renewed-browser-access","refresh_token":"rotated-browser-refresh","id_token":id_token("workspace-a")})),
        step("/.well-known/jwks.json",vec![],keys()),
        step("/oauth/revoke",vec![("client_id",CLIENT),("token","rotated-browser-refresh"),("token_type_hint","refresh_token")],json!({})),
    ]);
    let authority = Authority::new(wire).await;
    let dir = Directory::new();
    let manager = authority.pool(&dir);
    let login = browser_login(&manager, &authority, 0, "workspace-a", false)
        .await
        .unwrap();
    assert_eq!(login.state, AccountState::Active);
    assert_eq!(login.access, AccessState::Unknown);
    let refresh = manager.refresh("codex", "personal").await.unwrap();
    assert!(refresh.generation > login.generation);
    assert_eq!(
        manager.logout("codex", "personal", true).await.unwrap(),
        LogoutOutcome::Revoked
    );
    let record: Value =
        serde_json::from_slice(&std::fs::read(dir.0.join("codex/personal.json")).unwrap()).unwrap();
    assert!(record["credential"].is_null());
    assert_eq!(record["identity"]["scope"], "workspace-a");
    authority.done();
}

#[tokio::test]
async fn missing_nonce_wrong_nonce_and_changed_workspace_preserve_the_previous_session() {
    for (nonce_mode, workspace, error) in [
        (1, "workspace-a", CredentialError::Protocol),
        (2, "workspace-a", CredentialError::Protocol),
        (0, "workspace-b", CredentialError::IdentityMismatch),
    ] {
        let mut wire = steps();
        wire.extend(steps());
        let authority = Authority::new(wire).await;
        let dir = Directory::new();
        let manager = authority.pool(&dir);
        let first = browser_login(&manager, &authority, 0, "workspace-a", false)
            .await
            .unwrap();
        let original = std::fs::read(dir.0.join("codex/personal.json")).unwrap();
        assert_eq!(
            browser_login(&manager, &authority, nonce_mode, workspace, false)
                .await
                .unwrap_err(),
            error
        );
        let previous: Value = serde_json::from_slice(&original).unwrap();
        let current: Value =
            serde_json::from_slice(&std::fs::read(dir.0.join("codex/personal.json")).unwrap())
                .unwrap();
        assert_eq!(current["credential"], previous["credential"]);
        assert_eq!(current["identity"], previous["identity"]);
        let status = manager.list(Some("codex")).unwrap().remove(0);
        assert_eq!(status.state, AccountState::Active);
        assert_eq!(status.generation, first.generation);
        assert!(!status.login_pending);
        authority.done();
    }
}
#[tokio::test]
async fn a_browser_result_cannot_resurrect_a_locally_signed_out_account() {
    let authority = Authority::new(steps()).await;
    let dir = Directory::new();
    let manager = authority.pool(&dir);
    assert_eq!(
        browser_login(&manager, &authority, 0, "workspace-a", true)
            .await
            .unwrap_err(),
        CredentialError::Superseded
    );
    assert_eq!(
        manager.list(None).unwrap()[0].state,
        AccountState::SignedOut
    );
    authority.done();
}
#[tokio::test]
async fn bound_browser_error_is_terminal_and_never_exchanges_a_code() {
    let authority = Authority::new(vec![]).await;
    let dir = Directory::new();
    let manager = authority.pool(&dir);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let mut login = Box::pin(manager.login("codex", "personal", options(), |prompt| {
        let LoginPrompt::Browser(prompt) = prompt else {
            panic!("wrong method")
        };
        sender
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(prompt.clone())
            .unwrap();
    }));
    let prompt = tokio::select! { _ = &mut login => panic!("must await callback"), result = receiver => result.unwrap() };
    let (address, fields) = inspect(&prompt);
    let target = format!(
        "/auth/callback?state={}&error=access_denied&error_description=private-denial",
        fields["state"]
    );
    let (result, response) = timeout(Duration::from_secs(3), async {
        tokio::join!(&mut login, request(address, &target))
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err(), CredentialError::Denied);
    assert!(!response.contains("private-denial"));
    assert!(!manager.list(None).unwrap()[0].login_pending);
    drop(TcpListener::bind(address).await.unwrap());
    authority.done();
}
#[tokio::test]
async fn browser_cancellation_and_deadline_release_the_callback() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let authority = Authority::new(vec![]).await;
    let dir = Directory::new();
    let manager = authority.pool(&dir);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let mut login = Box::pin(manager.login("codex", "personal", options(), |prompt| {
        let LoginPrompt::Browser(prompt) = prompt else {
            panic!("wrong method")
        };
        sender
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(prompt.clone())
            .unwrap();
    }));
    let prompt = tokio::select! { _ = &mut login => panic!("must await callback"), result = receiver => result.unwrap() };
    let (address, _) = inspect(&prompt);
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(b"GET /auth/").await.unwrap();
    assert!(
        timeout(Duration::from_millis(20), &mut login)
            .await
            .is_err()
    );
    drop(login);
    let mut byte = [0; 1];
    let read = timeout(Duration::from_secs(1), stream.read(&mut byte))
        .await
        .unwrap();
    assert!(read.is_err() || read.unwrap() == 0);
    assert!(!manager.list(None).unwrap()[0].login_pending);
    drop(TcpListener::bind(address).await.unwrap());

    let driver = codex::CodexAuthority::synthetic(&authority.origin).unwrap();
    let mut grant = driver.browser(None).await.unwrap();
    let (address, _) = inspect(&grant.prompt);
    grant.deadline = Instant::now();
    assert_eq!(
        driver.complete_browser(grant).await.unwrap_err(),
        CredentialError::Expired
    );
    drop(TcpListener::bind(address).await.unwrap());
    authority.done();
}

#[test]
fn production_callback_policy_distinguishes_default_from_explicit_zero() {
    let drivers = builtin_drivers(None).unwrap();
    let codex = drivers.iter().find(|d| d.profile() == "codex").unwrap();
    let grok = drivers.iter().find(|d| d.profile() == "grok").unwrap();
    for port in [None, Some(1455), Some(1457)] {
        assert_eq!(
            codex
                .login_client(&LoginOptions {
                    callback_port: port,
                    ..options()
                })
                .unwrap(),
            CLIENT
        );
    }
    for port in [0, 1456, 65535] {
        assert_eq!(
            codex
                .login_client(&LoginOptions {
                    callback_port: Some(port),
                    ..options()
                })
                .unwrap_err(),
            CredentialError::InvalidInput
        );
    }
    for method in [LoginMethod::Device, LoginMethod::Browser] {
        assert!(
            codex
                .login_client(&LoginOptions {
                    method,
                    client_id: Some("other".into()),
                    ..LoginOptions::default()
                })
                .is_err()
        );
    }
    assert!(
        codex
            .login_client(&LoginOptions {
                callback_port: Some(0),
                ..LoginOptions::default()
            })
            .is_err()
    );
    assert!(
        grok.login_client(&LoginOptions {
            client_id: Some("approved-client".into()),
            callback_port: Some(0),
            ..options()
        })
        .is_ok()
    );
}

#[tokio::test]
async fn deadline_includes_the_last_identity_request_without_replaying_the_code() {
    let gate = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut wire = steps();
    wire[1].gate = Some(gate.clone());
    let authority = Authority::new(wire).await;
    let driver = codex::CodexAuthority::synthetic(&authority.origin).unwrap();
    let mut grant = driver.browser(None).await.unwrap();
    let (address, fields) = inspect(&grant.prompt);
    authority.steps.lock().unwrap().front_mut().unwrap().body =
        token(Some(&fields["nonce"]), "workspace-a");
    grant.deadline = Instant::now() + Duration::from_secs(1);
    let target = format!("/auth/callback?state={}&code=browser-code", fields["state"]);
    let (result, _) = timeout(Duration::from_secs(3), async {
        tokio::join!(driver.complete_browser(grant), request(address, &target))
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err(), CredentialError::Expired);
    gate.notify_one();
    drop(TcpListener::bind(address).await.unwrap());
    authority.done();
}
