//! Verifies startup file boundaries and the real process composition root without contacting a Provider.

use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use openbridge::{
    config::{BootstrapConfigFileError, BootstrapConfigPath},
    identity::{UserConfigFileError, UserConfigPath},
    provider::CredentialKind,
    providers,
    upstream_credentials::{UpstreamCredentialConfigFileError, UpstreamCredentialConfigPath},
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        // Allocate a process-unique directory so parallel tests never share startup files.
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("openbridge-{label}-{}-{id}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

#[test]
fn startup_paths_distinguish_missing_files_from_invalid_documents() {
    let workspace = TempWorkspace::new("file-errors");

    // Verify the default locator without mutating the process environment.
    assert_eq!(
        BootstrapConfigPath::default().path(),
        Path::new("config/bootstrap.toml")
    );

    // Preserve the selected path when each startup file cannot be read.
    let missing_bootstrap = workspace.path("missing-bootstrap.toml");
    match BootstrapConfigPath::new(&missing_bootstrap)
        .load()
        .unwrap_err()
    {
        BootstrapConfigFileError::Read { path, .. } => assert_eq!(path, missing_bootstrap),
        error => panic!("unexpected bootstrap error: {error:?}"),
    }
    let missing_users = workspace.path("missing-users.toml");
    match UserConfigPath::new(&missing_users).load().unwrap_err() {
        UserConfigFileError::Read { path, .. } => assert_eq!(path, missing_users),
        error => panic!("unexpected user error: {error:?}"),
    }
    let missing_upstream = workspace.path("missing-upstream.toml");
    match UpstreamCredentialConfigPath::new(&missing_upstream)
        .load()
        .unwrap_err()
    {
        UpstreamCredentialConfigFileError::Read { path, .. } => {
            assert_eq!(path, missing_upstream)
        }
        error => panic!("unexpected upstream error: {error:?}"),
    }

    // Distinguish readable files whose contents fail their respective schema validators.
    let invalid_bootstrap = workspace.write("invalid-bootstrap.toml", "[");
    assert!(matches!(
        BootstrapConfigPath::new(invalid_bootstrap).load(),
        Err(BootstrapConfigFileError::Invalid(_))
    ));
    let invalid_users = workspace.write("invalid-users.toml", "[");
    assert!(matches!(
        UserConfigPath::new(invalid_users).load(),
        Err(UserConfigFileError::Invalid(_))
    ));
    let invalid_upstream = workspace.write("invalid-upstream.toml", "[");
    assert!(matches!(
        UpstreamCredentialConfigPath::new(invalid_upstream).load(),
        Err(UpstreamCredentialConfigFileError::Invalid(_))
    ));
}

#[test]
fn process_reports_configuration_availability_before_bound_listener_failure() {
    let workspace = TempWorkspace::new("process-startup");

    // Reserve a loopback address so the child reaches the final bind step and fails deterministically.
    let occupied_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let listen = occupied_listener.local_addr().unwrap();

    // Write valid synthetic downstream, non-OpenAI API-key, and OpenBridge-owned OAuth2 startup inputs.
    let users = workspace.write(
        "users.toml",
        r#"schema_version = 1

[[users]]
id = "startup-user"
name = "Startup User"
api_key = "startup-downstream-key-0000000000000001"
enabled = true
"#,
    );
    let mut upstream = String::from("schema_version = 1\n");
    for (index, pool) in providers::compiled_config()
        .credential_pools
        .iter()
        .filter(|pool| pool.kind == CredentialKind::ApiKey)
        .filter(|pool| pool.id != "openai-primary")
        .enumerate()
    {
        upstream.push_str(&format!(
            "\n[[credential_pools]]\nid = \"{}\"\napi_keys = [\"synthetic-startup-key-{index}\"]\n",
            pool.id
        ));
    }
    let access_token = jwt(json!({"exp": unix_now().saturating_add(3_600)}));
    let id_token = jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-startup-account",
            "chatgpt_account_is_fedramp": false
        }
    }));
    workspace.write(
        "chatgpt-auth.json",
        &json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": id_token.clone(),
                "access_token": access_token.clone(),
                "refresh_token": "synthetic-startup-refresh",
                "account_id": "synthetic-startup-account"
            },
            "last_refresh": "2026-08-05T00:00:00Z"
        })
        .to_string(),
    );
    upstream.push_str(
        "\n[[credential_pools]]\nid = \"chatgpt-codex\"\nauth_json_file = \"chatgpt-auth.json\"\n",
    );
    let upstream = workspace.write("upstream-credentials.toml", &upstream);
    let bootstrap = workspace.write(
        "bootstrap.toml",
        &format!(
            r#"schema_version = 3
listen = "{listen}"
users_file = "{}"
upstream_credentials_file = "{}"
max_request_body = "1MiB"
max_json_response_body = "16MiB"
max_replay_body = "256KiB"
max_sse_event = "256KiB"
default_instructions = "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
upstream_connect_timeout = "5s"
upstream_pool_idle_timeout = "90s"
upstream_pool_max_idle_per_host = 16
"#,
            toml_path(&users),
            toml_path(&upstream)
        ),
    );

    // Launch the real binary and verify all composition stages completed before the expected bind error.
    let output = Command::new(env!("CARGO_BIN_EXE_openbridge"))
        .env("OPENBRIDGE_CONFIG", &bootstrap)
        .env("RUST_LOG", "info")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{stdout}\n{stderr}");
    assert!(!output.status.success(), "child unexpectedly succeeded");
    assert!(
        stderr.contains("failed to bind OpenBridge"),
        "unexpected startup error: {stderr}"
    );
    assert!(stdout.contains("Startup configuration availability (no network probe)"));
    assert!(stdout.contains("Providers (configuration only)"));
    assert!(stdout.contains("Public models (configuration only)"));
    assert_table_column(&stdout, "openai (", false);
    assert_table_column(&stdout, "gpt-5.6-sol (chat, responses)", true);
    assert_table_column(
        &stdout,
        "text-embedding-3-small (no executable route after configuration)",
        false,
    );
    assert!(!combined_output.contains("startup-downstream-key"));
    assert!(!combined_output.contains("synthetic-startup-key"));
    assert!(!combined_output.contains("synthetic-startup-refresh"));
    assert!(!combined_output.contains("synthetic-startup-account"));
    assert!(!combined_output.contains(&access_token));
    assert!(!combined_output.contains(&id_token));
    assert!(!combined_output.contains("chatgpt-auth.json"));
}

/// Verifies that one rendered item appears on the expected side of a dual-list table.
fn assert_table_column(output: &str, item: &str, expected_left: bool) {
    // Locate the unique rendered row and its column separator.
    let line = output
        .lines()
        .find(|line| line.contains(item))
        .unwrap_or_else(|| panic!("missing table item '{item}' in output: {output}"));
    let separator = line
        .find(" | ")
        .unwrap_or_else(|| panic!("missing table separator in row: {line}"));
    let item_offset = line.find(item).expect("located row must contain the item");

    // Compare the item position with the separator without depending on dynamic column widths.
    assert_eq!(
        item_offset < separator,
        expected_left,
        "item '{item}' appeared in the wrong table column: {line}"
    );
}

#[test]
fn probe_help_exits_before_loading_private_startup_files() {
    // Invoke the real CLI help path with no bootstrap locator available.
    let output = Command::new(env!("CARGO_BIN_EXE_openbridge-probe"))
        .arg("--help")
        .env_remove("OPENBRIDGE_CONFIG")
        .output()
        .unwrap();

    // Verify help succeeds and returns only static usage text.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("cargo run --bin openbridge-probe -- models"));
    assert!(stdout.contains("cargo run --bin openbridge-probe -- generation"));
    assert!(stdout.contains("--capability"));
    assert!(output.stderr.is_empty());
}
