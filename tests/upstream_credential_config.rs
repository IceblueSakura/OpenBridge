//! Verifies private upstream credential TOML parsing, pool binding, and environment-variable isolation.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use openbridge::{
    config::parse_bootstrap_config,
    credential::{CredentialSource, CredentialStoreBuilder},
    oauth2_credentials::OAuth2CredentialManagerError,
    provider::{CredentialKind, ProviderKind},
    providers::{build_compiled_registry, compiled_config},
    registry::{CredentialPoolConfig, build_registry},
    upstream_credentials::{UpstreamCredentialConfigError, UpstreamCredentialConfiguration},
};
use serde_json::{Value, json};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

const UPSTREAM_CREDENTIALS: &str = r#"
schema_version = 1

[[credential_pools]]
id = "openai-primary"
api_keys = ["synthetic-openai-key-a", "synthetic-openai-key-b"]
"#;

#[test]
fn upstream_toml_loads_a_required_pool_without_environment_locators() {
    // Build a runtime registry that requires only the OpenAI pool.
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // Load the selected pool from private TOML and verify ordered members and source category.
    let configuration = UpstreamCredentialConfiguration::from_toml(UPSTREAM_CREDENTIALS).unwrap();
    let credentials = configuration
        .into_builder_for(&registry, ["openai-primary"])
        .unwrap()
        .build();
    let members = credentials
        .upstream_pool(
            openbridge::provider::ProviderKind::OpenAi,
            "openai-primary",
            openbridge::provider::CredentialKind::ApiKey,
        )
        .unwrap();
    assert_eq!(members[0].member_id(), "openai-primary#1");
    assert_eq!(members[1].member_id(), "openai-primary#2");
    assert_eq!(
        members[0].metadata().source(),
        CredentialSource::UpstreamConfiguration
    );
}

#[test]
fn api_key_probe_builder_does_not_read_an_unselected_oauth2_auth_file() {
    let registry = registry();
    let configuration = UpstreamCredentialConfiguration::from_toml(
        r#"schema_version = 1

[[credential_pools]]
id = "openai-primary"
api_keys = ["synthetic-openai-key"]

[[credential_pools]]
id = "chatgpt-codex"
auth_json_file = "missing-unselected-sensitive-auth.json"
"#,
    )
    .unwrap();

    // Preserve the API-key probe boundary by loading only its explicitly selected credential source.
    let credentials = configuration
        .into_builder_for(&registry, ["openai-primary"])
        .unwrap()
        .build();
    assert!(
        credentials
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .is_ok()
    );
}

#[test]
fn oauth2_login_target_resolves_without_opening_or_requiring_the_auth_file() {
    // Configure a missing OpenBridge-owned auth file under a process-unique directory.
    let fixture = TestAuthFile::new("unused");
    let missing_auth_file = fixture.directory.join("missing-login-auth.json");
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
        toml_path(&missing_auth_file)
    ))
    .unwrap();

    // Resolve a purpose-bound login target without reading or exposing the missing locator.
    let target = configuration
        .oauth2_login_target_for(&registry(), ProviderKind::ChatGpt)
        .unwrap();
    assert_eq!(target.provider(), ProviderKind::ChatGpt);
    assert_eq!(target.pool_id(), "chatgpt-codex");
    assert!(!missing_auth_file.exists());
    assert!(!format!("{target:?}").contains("missing-login-auth"));
}

#[test]
fn upstream_toml_rejects_invalid_pool_documents_without_exposing_secrets() {
    // Cover empty pools, blank keys, duplicate keys, and duplicate pool IDs.
    let cases = [
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = []\n",
            UpstreamCredentialConfigError::EmptyPool {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"  \"]\n",
            UpstreamCredentialConfigError::BlankApiKey {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-a\", \"secret-a\"]\n",
            UpstreamCredentialConfigError::DuplicateApiKey {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-a\"]\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-b\"]\n",
            UpstreamCredentialConfigError::DuplicatePoolId {
                id: "openai-primary".to_owned(),
            },
        ),
    ];
    for (document, expected) in cases {
        let error = UpstreamCredentialConfiguration::from_toml(document).unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("secret-a"));
        assert!(!format!("{error:?} {error}").contains("secret-b"));
    }
}

#[test]
fn upstream_toml_rejects_unknown_and_missing_required_pools_before_insertion() {
    // Build the complete compile-time registry as the only valid pool list.
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // Reject a pool in TOML that is not registered in code.
    let unknown = UpstreamCredentialConfiguration::from_toml(
        "schema_version = 1\n[[credential_pools]]\nid = \"unknown\"\napi_keys = [\"secret\"]\n",
    )
    .unwrap();
    assert_eq!(
        unknown
            .into_builder_for(&registry, ["openai-primary"])
            .unwrap_err(),
        UpstreamCredentialConfigError::UnknownPool {
            id: "unknown".to_owned()
        }
    );

    // Reject a compile-time pool required by the caller but absent from TOML.
    let missing = UpstreamCredentialConfiguration::from_toml(UPSTREAM_CREDENTIALS).unwrap();
    assert_eq!(
        missing
            .into_builder_for(&registry, ["openai-primary", "mimo-primary"])
            .unwrap_err(),
        UpstreamCredentialConfigError::MissingPool {
            id: "mimo-primary".to_owned()
        }
    );
}

#[test]
fn upstream_toml_rejects_api_keys_for_the_registered_chatgpt_oauth_binding() {
    // Build the registry containing the disabled OAuth pool and present an API-key-shaped entry.
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();
    let configuration = UpstreamCredentialConfiguration::from_toml(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\napi_keys = [\"synthetic-value\"]\n",
    )
    .unwrap();

    // Reject the registered pool before any OAuth-shaped credential can enter the builder.
    let error = configuration
        .into_builder_for(&registry, ["chatgpt-codex"])
        .unwrap_err();
    assert_eq!(
        error,
        UpstreamCredentialConfigError::CredentialSourceKindMismatch {
            id: "chatgpt-codex".to_owned()
        }
    );
    assert!(!format!("{error:?} {error}").contains("synthetic-value"));
}

#[test]
fn upstream_toml_loads_one_chatgpt_auth_file_into_a_guarded_oauth2_manager() {
    // Write one complete synthetic ChatGPT OAuth auth document.
    let expires_at = unix_now().saturating_add(3_600);
    let access_token = jwt(json!({"exp": expires_at}));
    let id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-account",
            "chatgpt_account_is_fedramp": false
        }
    }));
    let fixture = TestAuthFile::new(
        &json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": id_token,
                "access_token": access_token,
                "refresh_token": "synthetic-refresh-token",
                "account_id": "synthetic-account"
            },
            "last_refresh": "2026-08-05T00:00:00Z"
        })
        .to_string(),
    );
    let before = fs::read(fixture.path()).unwrap();
    let registry = registry();
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
        toml_path(fixture.path())
    ))
    .unwrap();

    // Bind the OAuth source into its dedicated manager without populating the API-key store.
    let mut builder = CredentialStoreBuilder::new();
    let manager = configuration
        .load_into_for(&mut builder, &registry, ["chatgpt-codex"])
        .unwrap();
    let credential = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .expect("ChatGPT OAuth2 credential should be configured");
    assert_eq!(manager.configured_provider_count(), 1);
    assert_eq!(credential.pool_id(), "chatgpt-codex");
    assert_eq!(credential.member_id(), "chatgpt-codex#1");
    assert_eq!(
        credential.metadata().source(),
        CredentialSource::OAuth2AuthJsonFile
    );
    assert_eq!(
        credential.metadata().expires_at(),
        Some(UNIX_EPOCH + Duration::from_secs(expires_at))
    );
    drop(credential);

    // Prove the source is read-only and later file changes cannot mutate the startup snapshot.
    assert_eq!(fs::read(fixture.path()).unwrap(), before);
    fs::write(fixture.path(), "{changed-after-startup").unwrap();
    assert_eq!(manager.configured_provider_count(), 1);
    let debug = format!("{manager:?}");
    for forbidden in [
        "synthetic-account",
        "synthetic-refresh-token",
        access_token.as_str(),
        id_token.as_str(),
        fixture.path().to_string_lossy().as_ref(),
    ] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn expired_startup_bundle_is_loaded_for_immediate_refresh() {
    // Persist a complete expired bundle whose refresh token can recover the credential.
    let access_token = jwt(json!({"exp": 1}));
    let id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-expired-account"
        }
    }));
    let fixture = TestAuthFile::new(
        &json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": id_token,
                "access_token": access_token,
                "refresh_token": "synthetic-expired-refresh",
                "account_id": "synthetic-expired-account"
            },
            "last_refresh": "2026-08-05T00:00:00Z"
        })
        .to_string(),
    );
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
        toml_path(fixture.path())
    ))
    .unwrap();

    // Keep the complete credential so the runtime refresh worker can run immediately.
    let manager = configuration
        .load_into_for(
            &mut CredentialStoreBuilder::new(),
            &registry(),
            ["chatgpt-codex"],
        )
        .unwrap();
    let credential = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .expect("expired credential should remain refreshable");
    assert_eq!(
        credential.metadata().expires_at(),
        Some(UNIX_EPOCH + Duration::from_secs(1))
    );
}

#[test]
fn oversized_oauth2_auth_file_is_rejected_before_document_parsing() {
    // Persist one source just beyond the managed OAuth document limit.
    let fixture = TestAuthFile::new(&"x".repeat(64 * 1024 + 1));
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
        toml_path(fixture.path())
    ))
    .unwrap();

    // Reject the file at the storage boundary without attempting JSON/token parsing.
    let error = configuration
        .load_into_for(
            &mut CredentialStoreBuilder::new(),
            &registry(),
            ["chatgpt-codex"],
        )
        .unwrap_err();
    assert_eq!(
        error,
        UpstreamCredentialConfigError::OAuth2Credential(OAuth2CredentialManagerError::Read)
    );
}

#[test]
fn upstream_toml_rejects_ambiguous_or_mismatched_credential_sources() {
    let registry = registry();
    let cases = [
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"synthetic-key\"]\nauth_json_file = \"auth.json\"\n",
            UpstreamCredentialConfigError::ConflictingCredentialSources {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\n",
            UpstreamCredentialConfigError::MissingCredentialSource {
                id: "openai-primary".to_owned(),
            },
        ),
    ];

    // Reject tables that select both source variants or neither variant during document parsing.
    for (document, expected) in cases {
        assert_eq!(
            UpstreamCredentialConfiguration::from_toml(document).unwrap_err(),
            expected
        );
    }

    // Reject an auth-file locator bound to an API-key registry entry before reading the file.
    let configuration = UpstreamCredentialConfiguration::from_toml(
        "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\nauth_json_file = \"sensitive-path.json\"\n",
    )
    .unwrap();
    let error = configuration
        .load_into_for(
            &mut CredentialStoreBuilder::new(),
            &registry,
            ["openai-primary"],
        )
        .unwrap_err();
    assert_eq!(
        error,
        UpstreamCredentialConfigError::CredentialSourceKindMismatch {
            id: "openai-primary".to_owned(),
        }
    );
    assert!(!format!("{error:?} {error}").contains("sensitive-path"));
}

#[test]
fn upstream_toml_rejects_invalid_chatgpt_auth_bundles_without_exposing_values() {
    let registry = registry();
    let access_token = jwt(json!({"exp": unix_now().saturating_add(3_600)}));
    let id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-valid-account",
            "chatgpt_account_is_fedramp": false
        }
    }));
    let mismatched_id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-other-account",
            "chatgpt_account_is_fedramp": false
        }
    }));
    let cases = [
        (
            "{invalid-json".to_owned(),
            OAuth2CredentialManagerError::InvalidDocument,
        ),
        (
            json!({
                "auth_mode": "apikey",
                "tokens": {
                    "id_token": id_token.clone(),
                    "access_token": access_token.clone(),
                    "refresh_token": "synthetic-valid-refresh",
                    "account_id": "synthetic-valid-account"
                }
            })
            .to_string(),
            OAuth2CredentialManagerError::UnsupportedAuthMode,
        ),
        (
            json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": id_token.clone(),
                    "access_token": access_token.clone(),
                    "refresh_token": " ",
                    "account_id": "synthetic-valid-account"
                }
            })
            .to_string(),
            OAuth2CredentialManagerError::InvalidRefreshToken,
        ),
        (
            json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": mismatched_id_token.clone(),
                    "access_token": access_token.clone(),
                    "refresh_token": "synthetic-valid-refresh",
                    "account_id": "synthetic-valid-account"
                }
            })
            .to_string(),
            OAuth2CredentialManagerError::AccountBindingMismatch,
        ),
        (
            json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": id_token.clone(),
                    "access_token": access_token.clone(),
                    "refresh_token": "synthetic-valid-refresh",
                    "account_id": "synthetic-valid-account"
                },
                "last_refresh": " "
            })
            .to_string(),
            OAuth2CredentialManagerError::InvalidLastRefresh,
        ),
    ];

    // Reject each malformed or incomplete bundle before it can enter the immutable manager.
    for (document, expected) in cases {
        let fixture = TestAuthFile::new(&document);
        let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
            "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
            toml_path(fixture.path())
        ))
        .unwrap();
        let error = configuration
            .load_into_for(
                &mut CredentialStoreBuilder::new(),
                &registry,
                ["chatgpt-codex"],
            )
            .unwrap_err();
        assert_eq!(
            error,
            UpstreamCredentialConfigError::OAuth2Credential(expected)
        );
        let message = format!("{error:?} {error}");
        for forbidden in [
            "synthetic-valid-refresh",
            "synthetic-valid-account",
            "synthetic-other-account",
            access_token.as_str(),
            id_token.as_str(),
            mismatched_id_token.as_str(),
            fixture.path().to_string_lossy().as_ref(),
        ] {
            assert!(!message.contains(forbidden));
        }
    }

    // Normalize a missing locator target to a value-free manager read error.
    let fixture = TestAuthFile::new("{}");
    let missing = fixture.directory.join("missing-sensitive-auth.json");
    let configuration = UpstreamCredentialConfiguration::from_toml(&format!(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = '{}'\n",
        toml_path(&missing)
    ))
    .unwrap();
    let error = configuration
        .load_into_for(
            &mut CredentialStoreBuilder::new(),
            &registry,
            ["chatgpt-codex"],
        )
        .unwrap_err();
    assert_eq!(
        error,
        UpstreamCredentialConfigError::OAuth2Credential(OAuth2CredentialManagerError::Read)
    );
    assert!(!format!("{error:?} {error}").contains("missing-sensitive-auth"));
}

#[test]
fn upstream_toml_allows_only_one_auth_file_per_oauth2_provider() {
    // Add a second synthetic ChatGPT OAuth2 binding to exercise the Provider-level uniqueness rule.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    let mut definition = compiled_config();
    definition.credential_pools.push(CredentialPoolConfig {
        id: "chatgpt-secondary".to_owned(),
        provider: ProviderKind::ChatGpt,
        kind: CredentialKind::OAuth2BearerAccessToken,
    });
    let registry = build_registry(bootstrap, definition).unwrap();
    let configuration = UpstreamCredentialConfiguration::from_toml(
        "schema_version = 1\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = \"first-sensitive.json\"\n[[credential_pools]]\nid = \"chatgpt-secondary\"\nauth_json_file = \"second-sensitive.json\"\n",
    )
    .unwrap();

    // Reject duplicate Provider ownership before either locator is read or exposed.
    let error = configuration
        .load_into_for(
            &mut CredentialStoreBuilder::new(),
            &registry,
            ["chatgpt-codex", "chatgpt-secondary"],
        )
        .unwrap_err();
    assert_eq!(
        error,
        UpstreamCredentialConfigError::DuplicateOAuth2Provider {
            provider: ProviderKind::ChatGpt,
        }
    );
    let message = format!("{error:?} {error}");
    assert!(!message.contains("first-sensitive"));
    assert!(!message.contains("second-sensitive"));
}

fn registry() -> openbridge::registry::RuntimeRegistry {
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml")).unwrap();
    build_compiled_registry(bootstrap).unwrap()
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

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
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
            "openbridge-managed-oauth2-test-{}-{suffix}",
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
