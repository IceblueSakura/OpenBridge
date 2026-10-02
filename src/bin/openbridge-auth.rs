//! Presentation and cancellation for the shared manager; no product protocol dispatch.
#[cfg(unix)]
mod unix {
    use openbridge::credential::{
        CredentialError, CredentialManager, LoginMethod, LoginOptions, LoginPrompt, LogoutOutcome,
        builtin_drivers,
    };
    use std::{collections::BTreeMap, path::PathBuf, process::ExitCode};

    const HELP: &str = "OpenBridge file credential management (Unix only)
Usage:
  openbridge-auth PROFILE login --store DIR --account ALIAS [--client-id ID]
      [--method device|browser] [--callback-port PORT] [--proxy URL]
  openbridge-auth PROFILE refresh --store DIR --account ALIAS [--proxy URL]
  openbridge-auth PROFILE logout --store DIR --account ALIAS [--revoke] [--proxy URL]
  openbridge-auth [PROFILE] list --store DIR

Registered profiles: grok, codex. Supported methods are validated by the driver.
Grok defaults to its pinned product client; --client-id is an explicit override.\nDevice is the default, browser is explicit. Product metadata does not prove registration eligibility.
Codex supports private device and explicit browser login with its product client, NOT SIWC.
Browser callback is literal loopback. Grok defaults to an OS port; Codex defaults to 1455.
Codex also accepts explicit --callback-port 1457; no random port or automatic fallback.
The explicit DIR is your own private store, never another application's auth cache.
Readable account JSON files contain secrets. Old accounts.json snapshots are rejected.
Proxy must be explicit HTTP(S), without credentials; no ambient proxy or retry/fallback.
Logout is local unless --revoke is supplied. No inference or automatic account routing.
Ctrl-C cancels the operation; interrupted refresh requires login.";

    #[derive(Debug)]
    enum Command {
        Login,
        Refresh,
        Logout { revoke: bool },
        List,
    }
    #[derive(Debug)]
    struct Options {
        command: Command,
        profile: Option<String>,
        store: PathBuf,
        account: Option<String>,
        login: LoginOptions,
        proxy: Option<String>,
    }
    fn parse(args: &[String]) -> Result<Options, &'static str> {
        let (profile, operation, offset) = match args.first().map(String::as_str) {
            Some("list") => (None, "list", 1),
            Some(profile) if !profile.starts_with('-') => (
                Some(profile.to_owned()),
                args.get(1).map(String::as_str).ok_or("missing operation")?,
                2,
            ),
            _ => return Err("expected PROFILE operation or list"),
        };
        let mut values = BTreeMap::new();
        let mut revoke = false;
        let mut index = offset;
        while index < args.len() {
            let flag = args[index].as_str();
            if flag == "--revoke" {
                if revoke {
                    return Err("duplicate flag");
                }
                revoke = true;
                index += 1;
                continue;
            }
            if !matches!(
                flag,
                "--store"
                    | "--account"
                    | "--client-id"
                    | "--method"
                    | "--callback-port"
                    | "--proxy"
            ) {
                return Err("unknown flag");
            }
            let value = args
                .get(index + 1)
                .filter(|v| !v.is_empty() && !v.starts_with("--"))
                .ok_or("missing flag value")?;
            if values.insert(flag, value.clone()).is_some() {
                return Err("duplicate flag");
            }
            index += 2;
        }
        let command = match operation {
            "login" if !revoke => Command::Login,
            "refresh" if !revoke => Command::Refresh,
            "logout" => Command::Logout { revoke },
            "list" if !revoke => Command::List,
            _ => return Err("unknown operation or incompatible flags"),
        };
        let store = values
            .remove("--store")
            .ok_or("--store is required")?
            .into();
        let account = values.remove("--account");
        let proxy = values.remove("--proxy");
        let client_id = values.remove("--client-id");
        let method = values.remove("--method");
        let port = values.remove("--callback-port");
        if !matches!(command, Command::Login)
            && (client_id.is_some() || method.is_some() || port.is_some())
        {
            return Err("login options only apply to login");
        }
        let method = match method.as_deref().unwrap_or("device") {
            "device" => LoginMethod::Device,
            "browser" => LoginMethod::Browser,
            _ => return Err("unknown login method"),
        };
        if port.is_some() && method != LoginMethod::Browser {
            return Err("--callback-port requires browser method");
        }
        let callback_port = port
            .map(|v| v.parse::<u16>().map_err(|_| "invalid callback port"))
            .transpose()?;
        match command {
            Command::List if account.is_some() || proxy.is_some() => {
                return Err("list only accepts --store");
            }
            Command::Login | Command::Refresh | Command::Logout { .. } if account.is_none() => {
                return Err("--account is required");
            }
            _ => {}
        }
        Ok(Options {
            command,
            profile,
            store,
            account,
            login: LoginOptions {
                method,
                client_id,
                callback_port,
            },
            proxy,
        })
    }
    fn show_prompt(prompt: &LoginPrompt) {
        match prompt {
            LoginPrompt::Device(prompt) => eprintln!(
                "Open {} and enter {}. Approve only the login you initiated.",
                prompt.verification_uri, prompt.user_code
            ),
            LoginPrompt::Browser(prompt) => eprintln!(
                "Open this first-party authorization URL in your browser:\n{}\nCallback: {}\nCheck CLI output for final login confirmation.",
                prompt.authorization_url, prompt.redirect_uri
            ),
        }
    }
    async fn execute(options: Options) -> Result<(), CredentialError> {
        let drivers = builtin_drivers(options.proxy.as_deref())?;
        if let Some(profile) = &options.profile {
            let driver = drivers
                .iter()
                .find(|d| d.profile() == profile)
                .ok_or(CredentialError::UnknownProfile)?;
            if matches!(options.command, Command::Login) {
                driver.login_client(&options.login)?;
            }
        }
        let manager = CredentialManager::new(options.store, drivers)?;
        let profile = options.profile.as_deref();
        let account = options.account.as_deref().unwrap_or("");
        match options.command {
            Command::Login => {
                let status = manager
                    .login(
                        profile.ok_or(CredentialError::InvalidInput)?,
                        account,
                        options.login,
                        show_prompt,
                    )
                    .await?;
                println!(
                    "{}",
                    serde_json::to_string(&status).map_err(|_| CredentialError::Storage)?
                );
            }
            Command::Refresh => {
                let status = manager
                    .refresh(profile.ok_or(CredentialError::InvalidInput)?, account)
                    .await?;
                println!(
                    "{}",
                    serde_json::to_string(&status).map_err(|_| CredentialError::Storage)?
                );
            }
            Command::Logout { revoke } => {
                let outcome = manager
                    .logout(
                        profile.ok_or(CredentialError::InvalidInput)?,
                        account,
                        revoke,
                    )
                    .await?;
                println!(
                    "{}",
                    match outcome {
                        LogoutOutcome::LocalOnly =>
                            "Local session cleared; remote revocation was not requested.",
                        LogoutOutcome::Revoked =>
                            "Local session cleared; remote revocation acknowledged.",
                        LogoutOutcome::RevocationUnconfirmed =>
                            "Local session cleared; remote revocation NOT confirmed.",
                        LogoutOutcome::AlreadySignedOut => "No local account session.",
                    }
                );
                if outcome == LogoutOutcome::RevocationUnconfirmed {
                    return Err(CredentialError::Network);
                }
            }
            Command::List => println!(
                "{}",
                serde_json::to_string(&manager.list(profile)?)
                    .map_err(|_| CredentialError::Storage)?
            ),
        }
        Ok(())
    }
    pub async fn run() -> ExitCode {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.iter().any(|v| v == "--help" || v == "-h") {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        let options = match parse(&args) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}\n{HELP}");
                return ExitCode::FAILURE;
            }
        };
        tokio::select! {
            result = execute(options) => match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => { eprintln!("Credential operation failed: {error}"); ExitCode::FAILURE }
            },
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Credential operation cancelled; an interrupted refresh requires login.");
                ExitCode::FAILURE
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn args(s: &str) -> Vec<String> {
            s.split_whitespace().map(str::to_owned).collect()
        }
        #[test]
        fn presentation_options_are_generic_and_operations_remain_explicit() {
            let options = parse(&args(
                "third login --store private --account one --method browser --callback-port 1456",
            ))
            .unwrap();
            assert_eq!(options.profile.as_deref(), Some("third"));
            assert_eq!(options.login.method, LoginMethod::Browser);
            assert_eq!(options.login.callback_port, Some(1456));
            for input in [
                "list --store private",
                "codex login --store private --account one",
                "codex login --store private --account one --method browser",
                "codex login --store private --account one --method browser --callback-port 1457",
                "grok login --store private --account one --client-id approved --proxy http://127.0.0.1:1234",
                "codex logout --store private --account one --revoke",
            ] {
                assert!(parse(&args(input)).is_ok());
            }
            for input in [
                "grok list",
                "list --store private --account one",
                "list --store private --method browser",
                "grok refresh --store private --account one --client-id approved",
                "grok login --store private --account one --callback-port 1456",
                "grok login --store private --account one --method browser --callback-port 65536",
                "grok logout --store private --account one --revoke --revoke",
                "list --store private --store other",
                "migrate-grok --store private",
            ] {
                assert!(parse(&args(input)).is_err(), "{input}");
            }
        }
        #[test]
        fn profile_rules_are_owned_by_drivers_not_parser_branches() {
            let drivers = builtin_drivers(None).unwrap();
            let grok = drivers.iter().find(|d| d.profile() == "grok").unwrap();
            let codex = drivers.iter().find(|d| d.profile() == "codex").unwrap();
            assert_eq!(
                grok.login_client(&LoginOptions::default()).unwrap(),
                "b1a00492-073a-47ea-816f-4c329264a828"
            );
            assert!(
                codex
                    .login_client(&LoginOptions {
                        method: LoginMethod::Browser,
                        ..LoginOptions::default()
                    })
                    .is_ok()
            );
            assert!(
                codex
                    .login_client(&LoginOptions {
                        client_id: Some("other".into()),
                        ..LoginOptions::default()
                    })
                    .is_err()
            );
        }
    }
}
#[cfg(unix)]
#[tokio::main]
async fn main() -> std::process::ExitCode {
    unix::run().await
}
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("OpenBridge credential management requires a Unix owner-only store.");
    std::process::ExitCode::FAILURE
}
