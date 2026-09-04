//! Process-level contract tests for the explicit OAuth2 administrative CLI.
//!
//! These cases exit before configuration or network access and verify that rejected values are not
//! reflected into diagnostics.

use std::process::Command;

#[test]
fn help_exits_before_loading_private_configuration() {
    // Invoke static help with no bootstrap locator available.
    let output = Command::new(env!("CARGO_BIN_EXE_openbridge-auth"))
        .arg("--help")
        .env_remove("OPENBRIDGE_CONFIG")
        .output()
        .unwrap();

    // Verify help exposes only the fixed commands and override prohibition.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Usage: openbridge-auth login <provider>"));
    assert!(stdout.contains("Providers: chatgpt, grok"));
    assert!(stdout.contains("cannot be overridden"));
    assert!(output.stderr.is_empty());
}

#[test]
fn disallowed_auth_file_selector_is_rejected_without_echoing_its_value() {
    let sensitive_locator = "synthetic-sensitive-auth-locator";

    // Supply a forbidden locator override with no valid bootstrap configuration.
    let output = Command::new(env!("CARGO_BIN_EXE_openbridge-auth"))
        .args(["login", "chatgpt", "--auth-file", sensitive_locator])
        .env_remove("OPENBRIDGE_CONFIG")
        .output()
        .unwrap();

    // Confirm parsing failed before configuration/network work and did not reflect the value.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("expected the fixed command"));
    assert!(!stderr.contains(sensitive_locator));
    assert!(output.stdout.is_empty());
}
