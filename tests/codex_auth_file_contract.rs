//! Verifies the probe-only Codex auth file loader with synthetic credentials and no network access.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use openbridge::{
    codex_auth::{CodexAuthFileError, load_codex_auth_file_for_target},
    config::parse_bootstrap_config,
    credential::CredentialSource,
    provider::{CredentialKind, ProviderKind},
    providers::build_compiled_registry,
};
use serde_json::{Value, json};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

#[test]
fn codex_auth_file_loads_one_redacted_account_bound_oauth_credential_without_writing() {
    // Build synthetic JWTs with a future access-token expiry and a FedRAMP account claim.
    let expires_at = unix_now().saturating_add(3_600);
    let access_token = jwt(json!({"exp": expires_at}));
    let id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "account-sensitive",
            "chatgpt_account_is_fedramp": true
        }
    }));
    let document = json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": id_token,
            "access_token": access_token,
            "refresh_token": "refresh-sensitive",
            "account_id": "account-sensitive"
        },
        "last_refresh": "2026-08-05T00:00:00Z"
    })
    .to_string();
    let fixture = TestAuthFile::new(&document);
    let before = fs::read(fixture.path()).unwrap();
    let registry = registry();

    // Load the file once into the target-bound OAuth store and verify only non-sensitive metadata.
    let credentials =
        load_codex_auth_file_for_target(fixture.path(), &registry, "chatgpt-gpt-5-6-sol").unwrap();
    let credential = credentials
        .upstream_pool(
            ProviderKind::ChatGpt,
            "chatgpt-codex",
            CredentialKind::OAuth2BearerAccessToken,
        )
        .unwrap()
        .remove(0);
    assert_eq!(
        credential.metadata().source(),
        CredentialSource::CodexAuthFile
    );
    assert_eq!(
        credential.metadata().expires_at(),
        Some(UNIX_EPOCH + Duration::from_secs(expires_at))
    );

    // Confirm the source file is unchanged and every Debug surface omits credential values and paths.
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    let debug = format!("{credentials:?} {credential:?}");
    for forbidden in [
        "account-sensitive",
        "refresh-sensitive",
        access_token.as_str(),
        id_token.as_str(),
        fixture.path().to_string_lossy().as_ref(),
    ] {
        assert!(!debug.contains(forbidden));
    }

    // Preserve Codex's fallback to the ID-token workspace when the top-level account is absent.
    let fallback = TestAuthFile::new(&auth_document(&id_token, &access_token, None));
    assert!(
        load_codex_auth_file_for_target(fallback.path(), &registry, "chatgpt-gpt-5-6-sol",).is_ok()
    );
}

#[test]
fn codex_auth_file_rejects_wrong_mode_missing_binding_invalid_jwt_and_expired_access() {
    let future = unix_now().saturating_add(3_600);
    let expired = unix_now().saturating_sub(1);
    let valid_id = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "account-a",
            "chatgpt_account_is_fedramp": false
        }
    }));
    let unbound_id = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_is_fedramp": false
        }
    }));
    let registry = registry();
    let cases = [
        (
            json!({
                "auth_mode": "apikey",
                "OPENAI_API_KEY": "api-key-sensitive",
                "tokens": null
            })
            .to_string(),
            CodexAuthFileError::UnsupportedAuthMode,
        ),
        (
            auth_document(&unbound_id, &jwt(json!({"exp": future})), None),
            CodexAuthFileError::MissingAccountBinding,
        ),
        (
            auth_document(&valid_id, "not-a-jwt", Some("account-a")),
            CodexAuthFileError::InvalidAccessToken,
        ),
        (
            auth_document(&valid_id, &jwt(json!({"exp": expired})), Some("account-a")),
            CodexAuthFileError::ExpiredAccessToken,
        ),
        (
            auth_document("not-a-jwt", &jwt(json!({"exp": future})), Some("account-a")),
            CodexAuthFileError::InvalidIdToken,
        ),
        (
            auth_document(
                &valid_id,
                &jwt(json!({"missing_exp": true})),
                Some("account-a"),
            ),
            CodexAuthFileError::MissingAccessTokenExpiry,
        ),
        (
            auth_document(
                &jwt(json!({
                    "https://api.openai.com/auth": {"chatgpt_account_id": "account-b"}
                })),
                &jwt(json!({"exp": future})),
                Some("account-a"),
            ),
            CodexAuthFileError::AccountBindingMismatch,
        ),
    ];

    // Verify every invalid shape fails before egress with a stable value-free error.
    for (document, expected) in cases {
        let fixture = TestAuthFile::new(&document);
        let error =
            load_codex_auth_file_for_target(fixture.path(), &registry, "chatgpt-gpt-5-6-sol")
                .unwrap_err();
        assert_eq!(error, expected);
        let message = format!("{error:?} {error}");
        assert!(!message.contains("account-a"));
        assert!(!message.contains("account-b"));
        assert!(!message.contains("api-key-sensitive"));
        assert!(!message.contains(fixture.path().to_string_lossy().as_ref()));
    }
}

#[test]
fn codex_auth_file_hides_read_and_json_failure_paths() {
    let registry = registry();
    let missing = std::env::temp_dir().join("openbridge-codex-auth-missing-sensitive.json");

    // Reject a missing file without copying its path into Display or Debug output.
    let read_error =
        load_codex_auth_file_for_target(&missing, &registry, "chatgpt-gpt-5-6-sol").unwrap_err();
    assert_eq!(read_error, CodexAuthFileError::Read);
    assert!(!format!("{read_error:?} {read_error}").contains("missing-sensitive"));

    // Reject malformed JSON without reflecting source content or its temporary path.
    let fixture = TestAuthFile::new("{malformed-sensitive");
    let parse_error =
        load_codex_auth_file_for_target(fixture.path(), &registry, "chatgpt-gpt-5-6-sol")
            .unwrap_err();
    assert_eq!(parse_error, CodexAuthFileError::InvalidDocument);
    let message = format!("{parse_error:?} {parse_error}");
    assert!(!message.contains("malformed-sensitive"));
    assert!(!message.contains(fixture.path().to_string_lossy().as_ref()));
}

fn registry() -> openbridge::registry::RuntimeRegistry {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    build_compiled_registry(bootstrap).unwrap()
}

fn auth_document(id_token: &str, access_token: &str, account_id: Option<&str>) -> String {
    json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": id_token,
            "access_token": access_token,
            "refresh_token": "ignored-refresh-sensitive",
            "account_id": account_id
        }
    })
    .to_string()
}

fn jwt(payload: Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    format!("{header}.{payload}.synthetic-signature")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct TestAuthFile {
    directory: PathBuf,
    path: PathBuf,
}

impl TestAuthFile {
    fn new(contents: &str) -> Self {
        // Create one process-unique directory and write only the synthetic fixture supplied by the test.
        let suffix = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "openbridge-codex-auth-test-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("auth.json");
        fs::write(&path, contents).unwrap();
        Self { directory, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestAuthFile {
    fn drop(&mut self) {
        // Remove only the exact synthetic file and its process-unique empty directory.
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}
