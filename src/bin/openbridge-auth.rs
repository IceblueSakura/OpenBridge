//! Explicit administrative CLI for OpenBridge-owned upstream OAuth credentials.
//!
//! The command accepts no authority, client, endpoint, header, cache, or auth-file override. It
//! resolves the sole destination for the selected Provider from private configuration and never
//! starts the gateway.

use std::{env, ffi::OsString, process::ExitCode};

use openbridge::{
    config::BootstrapConfigPath,
    oauth2_credentials::{OAuth2LoginError, login_chatgpt, login_grok},
    provider::ProviderKind,
    providers::build_compiled_registry,
    upstream_credentials::UpstreamCredentialConfigPath,
};
use thiserror::Error;
use tokio::signal;

#[tokio::main]
/// Parses one fixed administrative operation and reports only redacted failures.
async fn main() -> ExitCode {
    // Parse the closed command shape before reading any configuration or starting network work.
    let action = match AuthAction::parse(env::args_os().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("Run openbridge-auth --help for usage.");
            return ExitCode::from(2);
        }
    };
    if action == AuthAction::Help {
        print_usage();
        return ExitCode::SUCCESS;
    }

    // Run the selected lifecycle operation and keep diagnostic output value-free.
    match login(action).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Loads the trusted destination for the selected Provider and runs one cancellable login.
async fn login(action: AuthAction) -> Result<(), AuthCliError> {
    // Load bootstrap and private upstream bindings without echoing their locators on failure.
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .map_err(|_| AuthCliError::Configuration)?;
    let upstream_credentials_file = bootstrap.upstream_credentials_file().to_owned();
    let registry = build_compiled_registry(bootstrap).map_err(|_| AuthCliError::Registry)?;
    let configuration = UpstreamCredentialConfigPath::new(upstream_credentials_file)
        .load()
        .map_err(|_| AuthCliError::Configuration)?;
    let provider = match action {
        AuthAction::LoginChatGpt => ProviderKind::ChatGpt,
        AuthAction::LoginGrok => ProviderKind::Grok,
        AuthAction::Help => return Err(AuthCliError::Usage),
    };
    let target = configuration
        .oauth2_login_target_for(&registry, provider)
        .map_err(|_| AuthCliError::Configuration)?;

    // Race the short-lived login state against an explicit terminal cancellation signal.
    type LoginFuture<'a> = std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        openbridge::oauth2_credentials::OAuth2LoginOutcome,
                        OAuth2LoginError,
                    >,
                > + Send
                + 'a,
        >,
    >;
    let login: LoginFuture<'_> = match provider {
        ProviderKind::ChatGpt => Box::pin(login_chatgpt(&target, |prompt| {
            println!("Sign in to the ChatGPT subscription configured for OpenBridge:");
            println!("  1. Open {}", prompt.verification_uri());
            println!("  2. Enter code: {}", prompt.user_code());
            println!(
                "The code expires within {} minutes. Device codes are a phishing target; never share it.",
                prompt.expires_in().as_secs() / 60
            );
            println!("Waiting for authorization. Press Ctrl+C to cancel.");
        })),
        ProviderKind::Grok => Box::pin(login_grok(&target, |prompt| {
            println!("Sign in to the Grok subscription configured for OpenBridge:");
            println!("  1. Open {}", prompt.verification_uri());
            println!("  2. Enter code: {}", prompt.user_code());
            println!(
                "The code expires within {} minutes. Device codes are a phishing target; never share it.",
                prompt.expires_in().as_secs() / 60
            );
            println!("Waiting for authorization. Press Ctrl+C to cancel.");
        })),
        _ => return Err(AuthCliError::Usage),
    };
    let provider_label = match provider {
        ProviderKind::ChatGpt => "ChatGPT",
        _ => "Grok",
    };
    tokio::select! {
        result = login => {
            let outcome = result.map_err(AuthCliError::Login)?;
            println!(
                "{provider_label} login completed for credential pool '{}'.",
                outcome.pool_id()
            );
            Ok(())
        }
        signal = signal::ctrl_c() => {
            signal.map_err(|_| AuthCliError::CancellationSignal)?;
            Err(AuthCliError::Cancelled)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthAction {
    Help,
    LoginChatGpt,
    LoginGrok,
}

impl AuthAction {
    /// Parses only the fixed login commands and static help, rejecting every override-like argument.
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, AuthCliError> {
        // Collect a small closed command shape without retaining unknown argument text in errors.
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        match arguments.as_slice() {
            [argument] if argument == "--help" || argument == "-h" => Ok(Self::Help),
            [verb, provider] if verb == "login" && provider == "chatgpt" => Ok(Self::LoginChatGpt),
            [verb, provider] if verb == "login" && provider == "grok" => Ok(Self::LoginGrok),
            _ => Err(AuthCliError::Usage),
        }
    }
}

/// Prints static CLI usage without reading private runtime state.
fn print_usage() {
    println!(
        "Usage: openbridge-auth login <provider>\n\
         \n\
         Providers: chatgpt, grok\n\
         \n\
         Starts the fixed device login flow for the selected Provider. Provider endpoints, public client registration, and the managed auth-file destination cannot be overridden from this command."
    );
}

/// Redacted failure returned by the administrative OAuth CLI.
#[derive(Debug, Error)]
enum AuthCliError {
    /// Arguments do not match one of the fixed supported operations.
    #[error("expected the fixed command 'login chatgpt' or 'login grok'")]
    Usage,
    /// Bootstrap or private upstream configuration could not be safely loaded and bound.
    #[error("OpenBridge OAuth configuration could not be loaded")]
    Configuration,
    /// The compile-time registry is inconsistent.
    #[error("OpenBridge code registry could not be built")]
    Registry,
    /// The device login or credential transaction failed.
    #[error("upstream login failed: {0}")]
    Login(#[source] OAuth2LoginError),
    /// The operating system cancellation handler could not be installed.
    #[error("cancellation signal handler could not be installed")]
    CancellationSignal,
    /// The administrator cancelled this device session.
    #[error("upstream login was cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_only_help_or_fixed_logins() {
        // Accept the closed command forms.
        assert_eq!(parse(&["--help"]).unwrap(), AuthAction::Help);
        assert_eq!(
            parse(&["login", "chatgpt"]).unwrap(),
            AuthAction::LoginChatGpt
        );
        assert_eq!(parse(&["login", "grok"]).unwrap(), AuthAction::LoginGrok);

        // Reject endpoint, client, file, header, and cache selectors before configuration loading.
        for arguments in [
            &["login", "chatgpt", "--issuer", "https://example.invalid"][..],
            &["login", "chatgpt", "--client-id", "synthetic-client"][..],
            &["login", "chatgpt", "--auth-file", "synthetic-file"][..],
            &["login", "chatgpt", "--header", "synthetic-header"][..],
            &["login", "chatgpt", "--codex-cache"][..],
            &["login", "grok", "--issuer", "https://example.invalid"][..],
            &["login", "grok", "--client-id", "synthetic-client"][..],
            &["login", "grok", "--auth-file", "synthetic-file"][..],
            &["login", "unknown"][..],
        ] {
            assert!(matches!(parse(arguments), Err(AuthCliError::Usage)));
        }
    }

    fn parse(arguments: &[&str]) -> Result<AuthAction, AuthCliError> {
        AuthAction::parse(arguments.iter().map(OsString::from))
    }
}
