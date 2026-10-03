//! Independent Codex wire/identity expectations, not Grok grant aliases.
//! RSA key/JWK fixtures were generated locally with OpenSSL solely for synthetic
//! tests. They contain no real credential, copied asset or upstream private key.
use super::test_support::{Authority, Directory, Step, step};
use super::*;
use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
pub(super) fn keys() -> Value {
    json!({"keys":[serde_json::from_str::<Value>(include_str!("../../tests/fixtures/auth/codex-test-jwk.json")).unwrap()]})
}
pub(super) fn claims(workspace: &str) -> Value {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    json!({"iss":"https://auth.openai.com", "aud":CLIENT, "sub":"person-a", "exp":timestamp+3600,
        "https://api.openai.com/auth":{"chatgpt_account_id":workspace}})
}
pub(super) fn sign(header: Value, claims: Value) -> String {
    let pair = ring::signature::RsaKeyPair::from_pkcs8(include_bytes!(
        "../../tests/fixtures/auth/codex-test-rsa.pk8"
    ))
    .unwrap();
    let message = format!(
        "{}.{}",
        B64.encode(serde_json::to_vec(&header).unwrap()),
        B64.encode(serde_json::to_vec(&claims).unwrap())
    );
    let mut signature = vec![0; pair.public().modulus_len()];
    pair.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        message.as_bytes(),
        &mut signature,
    )
    .unwrap();
    format!("{message}.{}", B64.encode(signature))
}
pub(super) fn id_token(workspace: &str) -> String {
    sign(
        json!({"alg":"RS256","kid":"synthetic-codex-key"}),
        claims(workspace),
    )
}
fn device() -> Step {
    step(
        "/api/accounts/deviceauth/usercode",
        vec![("client_id", CLIENT)],
        json!({"device_auth_id":"synthetic-device-id","user_code":"WXYZ-9999","interval":"1"}),
    )
}
fn code() -> Step {
    step(
        "/api/accounts/deviceauth/token",
        vec![
            ("device_auth_id", "synthetic-device-id"),
            ("user_code", "WXYZ-9999"),
        ],
        json!({"authorization_code":"synthetic-code","code_verifier":VERIFIER,"code_challenge":CHALLENGE}),
    )
}
fn login_steps(workspace: &str) -> Vec<Step> {
    vec![
        device(),
        code(),
        step(
            "/oauth/token",
            vec![
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT),
                ("code", "synthetic-code"),
                ("code_verifier", VERIFIER),
                (
                    "redirect_uri",
                    "https://auth.openai.com/deviceauth/callback",
                ),
            ],
            json!({"access_token":"synthetic-codex-access","refresh_token":"synthetic-codex-refresh","id_token":id_token(workspace)}),
        ),
        step("/.well-known/jwks.json", vec![], keys()),
    ]
}
async fn login(pool: &CredentialManager) -> AccountStatus {
    pool.login_codex("personal", |prompt| {
        assert_eq!(
            prompt.verification_uri,
            "https://auth.openai.com/codex/device"
        );
        assert_eq!(prompt.user_code, "WXYZ-9999");
        assert!(!format!("{prompt:?}").contains("synthetic-device-id"));
    })
    .await
    .unwrap()
}

#[test]
fn browser_is_an_explicit_supported_product_login_method() {
    let driver = codex::CodexAuthority::new(None).unwrap();
    let options = LoginOptions {
        method: LoginMethod::Browser,
        ..LoginOptions::default()
    };
    assert_eq!(driver.login_client(&options).unwrap(), CLIENT);
}

#[test]
fn signed_id_tokens_enforce_claims_algorithm_keys_and_workspace() {
    let valid = id_token("workspace-a");
    let identity = codex::verify_identity(&valid, &keys(), CLIENT, None).unwrap();
    assert_eq!(identity.subject, "person-a");
    assert_eq!(identity.scope.as_deref(), Some("workspace-a"));
    for field in ["iss", "aud", "sub", "exp", "https://api.openai.com/auth"] {
        let mut value = claims("workspace-a");
        match field {
            "iss" => value[field] = json!("https://untrusted.test"),
            "aud" => value[field] = json!("another-client"),
            "sub" => value[field] = json!(""),
            "exp" => value[field] = json!(1),
            _ => value[field] = json!({}),
        }
        let token = sign(json!({"alg":"RS256","kid":"synthetic-codex-key"}), value);
        assert_eq!(
            codex::verify_identity(&token, &keys(), CLIENT, None).unwrap_err(),
            CredentialError::Protocol
        );
    }
    for header in [
        json!({"alg":"none","kid":"synthetic-codex-key"}),
        json!({"alg":"HS256","kid":"synthetic-codex-key"}),
        json!({"alg":"RS256","kid":"unknown-key"}),
        json!({"alg":"RS256","kid":"synthetic-codex-key","crit":["unknown"]}),
        json!({"alg":"RS256","kid":"synthetic-codex-key","b64":false}),
    ] {
        let token = sign(header, claims("workspace-a"));
        assert_eq!(
            codex::verify_identity(&token, &keys(), CLIENT, None).unwrap_err(),
            CredentialError::Protocol
        );
    }
    let mut forged = valid.as_bytes().to_vec();
    let index = valid.rfind('.').unwrap() + 8;
    forged[index] = if forged[index] == b'A' { b'B' } else { b'A' };
    assert!(
        codex::verify_identity(std::str::from_utf8(&forged).unwrap(), &keys(), CLIENT, None)
            .is_err()
    );
    let mut duplicate = keys();
    let key = duplicate["keys"][0].clone();
    duplicate["keys"].as_array_mut().unwrap().push(key);
    assert!(codex::verify_identity(&valid, &duplicate, CLIENT, None).is_err());
    let mut audience = claims("workspace-a");
    audience["aud"] = json!([CLIENT, "another-client"]);
    let token = sign(
        json!({"alg":"RS256","kid":"synthetic-codex-key"}),
        audience.clone(),
    );
    assert!(codex::verify_identity(&token, &keys(), CLIENT, None).is_err());
    audience["azp"] = json!(CLIENT);
    let token = sign(
        json!({"alg":"RS256","kid":"synthetic-codex-key","jku":"https://untrusted.test/key"}),
        audience,
    );
    // Header key URLs are never fetched or used as authority.
    assert!(codex::verify_identity(&token, &keys(), CLIENT, None).is_ok());
}

#[test]
fn jwt_library_defaults_must_not_relax_profile_policy() {
    let header = json!({"alg":"RS256","kid":"synthetic-codex-key"});
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for (field, value) in [
        ("exp", timestamp - 1),
        ("nbf", timestamp + 3600),
        ("iat", timestamp + 3600),
    ] {
        let mut value_claims = claims("workspace-a");
        value_claims[field] = json!(value);
        assert!(
            codex::verify_identity(&sign(header.clone(), value_claims), &keys(), CLIENT, None)
                .is_err(),
            "{field}"
        );
    }
    let token = id_token("workspace-a");
    for (field, value) in [
        ("use", json!("enc")),
        ("key_ops", json!(["sign"])),
        ("alg", json!("ES256")),
    ] {
        let mut keys = keys();
        keys["keys"][0][field] = value;
        assert!(
            codex::verify_identity(&token, &keys, CLIENT, None).is_err(),
            "{field}"
        );
    }
    let mut header = header;
    header["jwk"] = json!({"kty":"oct","k":"c3ludGhldGlj"});
    header["x5u"] = json!("https://untrusted.invalid/key");
    let token = sign(header, claims("workspace-a"));
    assert!(codex::verify_identity(&token, &keys(), CLIENT, None).is_ok());
}
#[tokio::test]
async fn shared_pool_keeps_same_alias_subject_and_logout_isolated() {
    let mut steps = super::tests::login_steps("person-a");
    steps.extend(login_steps("workspace-a"));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    super::tests::login(&pool, "personal").await;
    let status = login(&pool).await;
    assert_eq!(status.profile, "codex");
    assert_eq!(status.expires_at, None); // Access TTL is unknown, not the ID-token exp.
    assert_eq!(pool.list(None).unwrap().len(), 2);
    assert_eq!(pool.list(Some("grok")).unwrap().len(), 1);
    pool.logout("grok", "personal", false).await.unwrap();
    assert_eq!(
        pool.list(Some("grok")).unwrap()[0].state,
        AccountState::SignedOut
    );
    assert_eq!(
        pool.list(Some("codex")).unwrap()[0].state,
        AccountState::Active
    );
    let public = serde_json::to_string(&pool.list(None).unwrap()).unwrap();
    for private in [
        "person-a",
        "workspace-a",
        "synthetic-codex-access",
        "synthetic-codex-refresh",
    ] {
        assert!(!public.contains(private));
    }
    authority.done();
}

#[tokio::test]
async fn private_pending_statuses_do_not_become_standard_device_grant() {
    let mut steps = vec![device()];
    for status in [StatusCode::FORBIDDEN, StatusCode::NOT_FOUND] {
        let mut response = code();
        response.status = status;
        response.body = json!({"detail":"private-pending"});
        steps.push(response);
    }
    steps.extend(login_steps("workspace-a").into_iter().skip(1));
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool).await;
    authority.done();
    let mut unavailable = device();
    unavailable.status = StatusCode::NOT_FOUND;
    let authority = Authority::new(vec![unavailable]).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    assert_eq!(
        pool.login_codex("personal", |_| {}).await.unwrap_err(),
        CredentialError::DeviceUnavailable
    );
    authority.done();
}

#[tokio::test]
async fn code_challenge_mismatch_does_not_exchange_or_publish() {
    let mut invalid = code();
    invalid.body["code_challenge"] = json!("invalid");
    let authority = Authority::new(vec![device(), invalid]).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    assert_eq!(
        pool.login_codex("personal", |_| {}).await.unwrap_err(),
        CredentialError::Protocol
    );
    assert_eq!(
        pool.list(Some("codex")).unwrap()[0].state,
        AccountState::SignedOut
    );
    authority.done();
}

#[tokio::test]
async fn refresh_uses_json_metadata_rotation_and_preserves_unreported_fields() {
    let mut steps = login_steps("workspace-a");
    steps.extend([
        step(
            "/oauth/token",
            vec![
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT),
                ("refresh_token", "synthetic-codex-refresh"),
            ],
            json!({"access_token":"opaque-renewed","refresh_token":"rotated-codex-refresh"}),
        ),
        step(
            "/oauth/token",
            vec![("refresh_token", "rotated-codex-refresh")],
            json!({"access_token":"final-opaque"}),
        ),
        step(
            "/oauth/revoke",
            vec![
                ("client_id", CLIENT),
                ("token", "rotated-codex-refresh"),
                ("token_type_hint", "refresh_token"),
            ],
            json!({}),
        ),
    ]);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    let before = login(&pool).await;
    let first = pool.refresh("codex", "personal").await.unwrap();
    assert!(first.generation > before.generation);
    assert_eq!(first.expires_at, None);
    pool.refresh("codex", "personal").await.unwrap();
    assert_eq!(
        pool.logout("codex", "personal", true).await.unwrap(),
        LogoutOutcome::Revoked
    );
    authority.done();
}

#[tokio::test]
async fn changed_signed_workspace_cannot_rebind_login_or_refresh() {
    let mut steps = login_steps("workspace-a");
    steps.extend(login_steps("workspace-b"));
    steps.extend([
        step(
            "/oauth/token",
            vec![],
            json!({"access_token":"new-access","id_token":id_token("workspace-b")}),
        ),
        step("/.well-known/jwks.json", vec![], keys()),
    ]);
    let authority = Authority::new(steps).await;
    let dir = Directory::new();
    let pool = authority.pool(&dir);
    login(&pool).await;
    assert_eq!(
        pool.login_codex("personal", |_| {}).await.unwrap_err(),
        CredentialError::IdentityMismatch
    );
    assert_eq!(
        pool.list(Some("codex")).unwrap()[0].state,
        AccountState::Active
    );
    assert_eq!(
        pool.refresh("codex", "personal").await.unwrap_err(),
        CredentialError::IdentityMismatch
    );
    assert_eq!(
        pool.list(Some("codex")).unwrap()[0].state,
        AccountState::NeedsReauthorization
    );
    assert_eq!(
        pool.logout("codex", "personal", true).await.unwrap(),
        LogoutOutcome::RevocationUnconfirmed
    );
    authority.done();
}

#[tokio::test]
async fn rejected_refresh_does_not_repeat_consumed_grants() {
    for (code, expected) in [
        ("refresh_token_expired", CredentialError::GrantExpired),
        ("refresh_token_reused", CredentialError::GrantReused),
        ("refresh_token_invalidated", CredentialError::GrantRevoked),
    ] {
        let mut steps = login_steps("workspace-a");
        let mut response = step(
            "/oauth/token",
            vec![],
            json!({"error":{"code":code,"message":"sensitive-detail"}}),
        );
        response.status = StatusCode::UNAUTHORIZED;
        steps.push(response);
        let authority = Authority::new(steps).await;
        let dir = Directory::new();
        let pool = authority.pool(&dir);
        login(&pool).await;
        let error = pool.refresh("codex", "personal").await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("sensitive-detail"));
        assert_eq!(
            pool.refresh("codex", "personal").await.unwrap_err(),
            CredentialError::LoginRequired
        );
        authority.done();
    }
}

#[test]
fn legacy_snapshot_is_rejected_without_reading_or_rewriting_it() {
    use std::{fs::DirBuilder, os::unix::fs::DirBuilderExt};
    let dir = Directory::new();
    DirBuilder::new().mode(0o700).create(&dir.0).unwrap();
    let original = b"synthetic obsolete snapshot; intentionally not parsed";
    let path = dir.0.join("accounts.json");
    std::fs::write(&path, original).unwrap();
    assert!(matches!(
        super::test_support::offline(&dir.0),
        Err(CredentialError::LegacyStore)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn codex_principal_scope_is_validated_by_its_driver_on_reload() {
    let dir = Directory::new();
    let pool = super::test_support::offline(&dir.0).unwrap();
    let mut account = model::Account::new("codex", "personal", CLIENT);
    account.identity = Some(VerifiedIdentity {
        subject: "person-a".into(),
        scope: None,
    });
    pool.store.transaction().unwrap().publish(&account).unwrap();
    assert_eq!(pool.list(None).unwrap_err(), CredentialError::Protocol);
}
