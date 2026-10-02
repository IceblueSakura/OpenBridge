//! Actual CLI and independent file expectations; all credentials are synthetic.
#[cfg(unix)]
mod unix {
    use serde_json::{Value, json};
    use std::{
        fs::{DirBuilder, OpenOptions},
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
        path::{Path, PathBuf},
    };
    use tokio::{
        process::Command,
        time::{Duration, timeout},
    };

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes).unwrap();
            let suffix: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir().join(format!("openbridge-auth-cli-{suffix}"));
            DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    async fn command(args: &[&std::ffi::OsStr]) -> std::process::Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_openbridge-auth"));
        command
            .args(args)
            .env_clear()
            .env("XAI_API_KEY", "unused-synthetic-key")
            .kill_on_drop(true);
        timeout(Duration::from_secs(10), command.output())
            .await
            .expect("bounded CLI operation")
            .unwrap()
    }
    fn save(root: &Path, profile: &str, alias: &str, value: &Value) {
        let dir = root.join(profile);
        if !dir.exists() {
            DirBuilder::new().mode(0o700).create(&dir).unwrap();
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.join(format!("{alias}.json")))
            .unwrap();
        file.write_all(serde_json::to_vec_pretty(value).unwrap().as_slice())
            .unwrap();
    }
    fn account(profile: &str, alias: &str, subject: &str) -> Value {
        let codex = profile == "codex";
        json!({
            "profile":profile,"alias":alias,
            "client_id":if codex { "app_EMoamEEZ73f0CkXaXp7hrann" } else { "synthetic-client" },
            "identity":{"subject":subject,"scope":if codex {Some("synthetic-workspace")} else {None}},
            "revision":2,"generation":1,"login_attempt":null,"state":"active",
            "credential":{"access":"synthetic-access","refresh":"synthetic-refresh",
                "id_token":if codex {Some("synthetic-id-token")} else {None},
                "expires_at":4000000000u64,
                "scopes":if codex {None} else {Some("openid profile email offline_access grok-cli:access api:access")}}
        })
    }
    fn read(root: &Path, profile: &str, alias: &str) -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(profile).join(format!("{alias}.json"))).unwrap(),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn binary_uses_account_files_and_account_locks_without_cross_profile_effects() {
        let dir = Directory::new();
        save(
            &dir.0,
            "grok",
            "one",
            &account("grok", "one", "synthetic-person-one"),
        );
        save(
            &dir.0,
            "grok",
            "two",
            &account("grok", "two", "synthetic-person-two"),
        );
        save(
            &dir.0,
            "codex",
            "one",
            &account("codex", "one", "synthetic-person-one"),
        );
        let path = dir.0.as_os_str();
        let all = command(&["list".as_ref(), "--store".as_ref(), path]).await;
        assert!(all.status.success());
        let statuses: Value = serde_json::from_slice(&all.stdout).unwrap();
        assert_eq!(statuses.as_array().unwrap().len(), 3);
        for secret in [
            "synthetic-person",
            "synthetic-access",
            "synthetic-refresh",
            "synthetic-id-token",
            "synthetic-workspace",
            "unused-synthetic-key",
        ] {
            assert!(!String::from_utf8_lossy(&all.stdout).contains(secret));
            assert!(!String::from_utf8_lossy(&all.stderr).contains(secret));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.0.join("grok/one.lock"))
            .unwrap();
        lock.lock().unwrap();
        let busy = command(&[
            "grok".as_ref(),
            "logout".as_ref(),
            "--store".as_ref(),
            path,
            "--account".as_ref(),
            "one".as_ref(),
        ])
        .await;
        assert!(!busy.status.success());
        assert!(String::from_utf8_lossy(&busy.stderr).contains("store is busy"));
        let list = command(&["grok".as_ref(), "list".as_ref(), "--store".as_ref(), path]).await;
        assert!(list.status.success());
        let unrelated = command(&[
            "grok".as_ref(),
            "logout".as_ref(),
            "--store".as_ref(),
            path,
            "--account".as_ref(),
            "two".as_ref(),
        ])
        .await;
        assert!(unrelated.status.success());
        assert!(read(&dir.0, "grok", "two")["credential"].is_null());
        assert_eq!(
            read(&dir.0, "grok", "one")["credential"]["refresh"],
            "synthetic-refresh"
        );
        assert_eq!(
            read(&dir.0, "codex", "one")["credential"]["refresh"],
            "synthetic-refresh"
        );
        drop(lock);
        let logout = command(&[
            "grok".as_ref(),
            "logout".as_ref(),
            "--store".as_ref(),
            path,
            "--account".as_ref(),
            "one".as_ref(),
        ])
        .await;
        assert!(logout.status.success());
        let snapshot = read(&dir.0, "grok", "one");
        assert_eq!(snapshot["state"], "signed_out");
        assert!(snapshot["credential"].is_null());
        assert_eq!(snapshot["identity"]["subject"], "synthetic-person-one");
        let refresh = command(&[
            "grok".as_ref(),
            "refresh".as_ref(),
            "--store".as_ref(),
            path,
            "--account".as_ref(),
            "one".as_ref(),
        ])
        .await;
        assert!(!refresh.status.success());
        assert!(String::from_utf8_lossy(&refresh.stderr).contains("login required"));
        let store_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.0.join("store.lock"))
            .unwrap();
        store_lock.lock().unwrap();
        let busy = command(&["list".as_ref(), "--store".as_ref(), path]).await;
        assert!(!busy.status.success());
    }
    #[tokio::test]
    async fn binary_refuses_old_format_and_reports_quarantine_without_exposing_tokens() {
        let legacy = Directory::new();
        std::fs::write(legacy.0.join("accounts.json"), "synthetic obsolete data").unwrap();
        let output = command(&["list".as_ref(), "--store".as_ref(), legacy.0.as_os_str()]).await;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("legacy"));
        assert_eq!(
            std::fs::read_to_string(legacy.0.join("accounts.json")).unwrap(),
            "synthetic obsolete data"
        );

        let dir = Directory::new();
        save(
            &dir.0,
            "codex",
            "one",
            &account("codex", "one", "synthetic-person"),
        );
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.0.join("codex/one.pending"))
            .unwrap();
        let output = command(&["list".as_ref(), "--store".as_ref(), dir.0.as_os_str()]).await;
        assert!(output.status.success());
        let statuses: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(statuses[0]["state"], "needs_reauthorization");
        assert_eq!(statuses[0]["access"], "unavailable");
        assert_eq!(statuses[0]["recovery_required"], true);
        let logout = command(&[
            "codex".as_ref(),
            "logout".as_ref(),
            "--store".as_ref(),
            dir.0.as_os_str(),
            "--account".as_ref(),
            "one".as_ref(),
            "--revoke".as_ref(),
        ])
        .await;
        assert!(!logout.status.success());
        assert!(String::from_utf8_lossy(&logout.stdout).contains("NOT confirmed"));
        assert!(read(&dir.0, "codex", "one")["credential"].is_null());
        assert!(!dir.0.join("codex/one.pending").exists());
    }

    #[tokio::test]
    async fn browser_registration_errors_are_rejected_before_store_creation() {
        let dir = Directory::new();
        let store = dir.0.join("not-created");
        for flags in [
            vec![
                "codex",
                "login",
                "--method",
                "browser",
                "--callback-port",
                "0",
            ],
            vec![
                "codex",
                "login",
                "--method",
                "browser",
                "--callback-port",
                "1456",
            ],
            vec![
                "codex",
                "login",
                "--method",
                "browser",
                "--client-id",
                "unrelated-client",
            ],
            vec!["grok", "login", "--method", "browser"],
        ] {
            let mut args: Vec<&std::ffi::OsStr> = flags.iter().map(AsRef::as_ref).collect();
            args.extend([
                "--store".as_ref(),
                store.as_os_str(),
                "--account".as_ref(),
                "personal".as_ref(),
            ]);
            let result = command(&args).await;
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(!store.exists());
        }
    }

    #[tokio::test]
    async fn binary_browser_callback_and_ctrl_c_are_owned_without_authority_traffic() {
        use std::process::Stdio;
        use tokio::{
            io::{AsyncBufReadExt, AsyncReadExt, BufReader},
            net::TcpListener,
        };
        let dir = Directory::new();
        let store = dir.0.join("sessions");
        let egress = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy = format!("http://{}", egress.local_addr().unwrap());
        let mut child = Command::new(env!("CARGO_BIN_EXE_openbridge-auth"))
            .args([
                "grok",
                "login",
                "--account",
                "personal",
                "--client-id",
                "synthetic-cli-client",
                "--method",
                "browser",
                "--callback-port",
                "0",
                "--store",
            ])
            .arg(&store)
            .args(["--proxy", &proxy])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut stderr = BufReader::new(child.stderr.take().unwrap().take(8192));
        let redirect = timeout(Duration::from_secs(5), async {
            for _ in 0..4 {
                let mut line = String::new();
                assert!(stderr.read_line(&mut line).await.unwrap() > 0);
                if let Some(uri) = line.strip_prefix("Callback: ") {
                    return uri.trim().to_owned();
                }
            }
            panic!("browser callback prompt missing");
        })
        .await
        .unwrap();
        let uri = url::Url::parse(&redirect).unwrap();
        assert_eq!(uri.scheme(), "http");
        assert_eq!(uri.host_str(), Some("127.0.0.1"));
        assert_eq!(uri.path(), "/callback");
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], uri.port().unwrap()));
        let response = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
            .get(format!("{redirect}?state=unrelated&error=access_denied"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert!(child.try_wait().unwrap().is_none());
        // This PID belongs to the child above; kill_on_drop is the failure-path guard.
        assert_eq!(
            unsafe { libc::kill(child.id().unwrap() as libc::pid_t, libc::SIGINT) },
            0
        );
        assert!(
            !timeout(Duration::from_secs(5), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        let account = read(&store, "grok", "personal");
        assert!(account["login_attempt"].is_null());
        assert!(account["credential"].is_null());
        assert_eq!(account["state"], "signed_out");
        drop(TcpListener::bind(address).await.unwrap());
        assert!(
            timeout(Duration::from_millis(20), egress.accept())
                .await
                .is_err()
        );
    }
}
