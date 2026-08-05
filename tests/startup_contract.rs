//! Verifies startup file boundaries and the real process composition root without contacting a Provider.

use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use openbridge::{
    config::{BootstrapConfigFileError, BootstrapConfigPath},
    identity::{UserConfigFileError, UserConfigPath},
    providers,
    upstream_credentials::{UpstreamCredentialConfigFileError, UpstreamCredentialConfigPath},
};

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
fn process_loads_all_startup_snapshots_before_reporting_a_bound_listener_failure() {
    let workspace = TempWorkspace::new("process-startup");

    // Reserve a loopback address so the child reaches the final bind step and fails deterministically.
    let occupied_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let listen = occupied_listener.local_addr().unwrap();

    // Write valid synthetic downstream and upstream credential snapshots for every compiled pool.
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
        .enumerate()
    {
        upstream.push_str(&format!(
            "\n[[credential_pools]]\nid = \"{}\"\napi_keys = [\"synthetic-startup-key-{index}\"]\n",
            pool.id
        ));
    }
    let upstream = workspace.write("upstream-credentials.toml", &upstream);
    let bootstrap = workspace.write(
        "bootstrap.toml",
        &format!(
            r#"schema_version = 2
listen = "{listen}"
users_file = "{}"
upstream_credentials_file = "{}"
max_request_body_bytes = 1048576
max_json_response_body_bytes = 16777216
max_replay_body_bytes = 262144
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#,
            toml_path(&users),
            toml_path(&upstream)
        ),
    );

    // Launch the real binary and verify all composition stages completed before the expected bind error.
    let output = Command::new(env!("CARGO_BIN_EXE_openbridge"))
        .env("OPENBRIDGE_CONFIG", &bootstrap)
        .env("RUST_LOG", "off")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "child unexpectedly succeeded");
    assert!(
        stderr.contains("failed to bind OpenBridge"),
        "unexpected startup error: {stderr}"
    );
    assert!(!stderr.contains("startup-downstream-key"));
    assert!(!stderr.contains("synthetic-startup-key"));
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
    assert!(stdout.contains("Usage: cargo run --bin openbridge-probe"));
    assert!(stdout.contains("No probe selector runs --all"));
    assert!(output.stderr.is_empty());
}
