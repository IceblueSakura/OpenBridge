//! Local construction of the Codex-compatible request identity used by the ChatGPT probe.
//!
//! The format is ported from the pinned Codex source baseline: originator/version, `os_info`
//! fields, architecture, and an environment-derived terminal token. OpenBridge never launches,
//! links to, or reads state from a Codex executable; the complete User-Agent remains private to
//! the trusted Provider header boundary.

use std::fmt;

use http::{HeaderMap, HeaderValue, header::USER_AGENT};

const CODEX_ORIGINATOR: &str = "codex_cli_rs";

/// Codex release profile pinned for the first-stage ChatGPT backend compatibility probe.
pub const CODEX_COMPAT_VERSION: &str = "0.145.0";

/// Codex-compatible request identity constructed entirely inside OpenBridge.
pub struct CodexRequestIdentity {
    user_agent: HeaderValue,
    version: &'static str,
    platform_family: &'static str,
    platform_os: &'static str,
}

impl CodexRequestIdentity {
    /// Builds the pinned Codex-compatible identity from the current OS and terminal environment.
    pub fn current() -> Self {
        // Read the same OS information source used by the pinned Codex implementation.
        let os_info = os_info::get();
        let architecture = os_info.architecture().unwrap_or("unknown");

        // Format and sanitize the private User-Agent through the source-compatible rules.
        Self::from_components(
            os_info.os_type().to_string(),
            os_info.version().to_string(),
            architecture,
            terminal_user_agent(),
        )
    }

    /// Returns the pinned Codex compatibility version used in the User-Agent and models query.
    pub fn version(&self) -> &str {
        self.version
    }

    /// Returns the platform family used for redacted compatibility reporting.
    pub fn platform_family(&self) -> &str {
        self.platform_family
    }

    /// Returns the platform OS used for redacted compatibility reporting.
    pub fn platform_os(&self) -> &str {
        self.platform_os
    }

    /// Builds the exact ordinary headers consumed by the fixed ChatGPT Provider hook.
    pub(crate) fn request_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, self.user_agent.clone());
        headers.insert("originator", HeaderValue::from_static(CODEX_ORIGINATOR));
        headers
    }

    /// Builds a deterministic source-compatible identity for crate tests.
    #[cfg(test)]
    pub(crate) fn for_test(
        os_type: &str,
        os_version: &str,
        architecture: &str,
        terminal: &str,
    ) -> Self {
        Self::from_components(
            os_type.to_owned(),
            os_version.to_owned(),
            architecture,
            terminal.to_owned(),
        )
    }

    /// Formats a Codex-compatible identity from already detected non-secret components.
    fn from_components(
        os_type: String,
        os_version: String,
        architecture: &str,
        terminal: String,
    ) -> Self {
        // Match Codex's originator, release, OS, architecture, and terminal token ordering.
        let prefix = format!(
            "{CODEX_ORIGINATOR}/{CODEX_COMPAT_VERSION} ({os_type} {os_version}; {architecture}) {terminal}"
        );
        let user_agent = sanitize_user_agent(prefix);
        let user_agent = HeaderValue::from_str(&user_agent)
            .unwrap_or_else(|_| HeaderValue::from_static(CODEX_ORIGINATOR));

        // Retain only validated headers and non-sensitive report metadata.
        Self {
            user_agent,
            version: CODEX_COMPAT_VERSION,
            platform_family: std::env::consts::FAMILY,
            platform_os: std::env::consts::OS,
        }
    }
}

impl fmt::Debug for CodexRequestIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexRequestIdentity")
            .field("version", &self.version)
            .field("platform_family", &self.platform_family)
            .field("platform_os", &self.platform_os)
            .field("user_agent", &"[OMITTED]")
            .finish()
    }
}

/// Derives the terminal token from environment variables in the same precedence as Codex.
fn terminal_user_agent() -> String {
    // Prefer the explicit terminal program and its optional version.
    if let Some(program) = environment_value("TERM_PROGRAM") {
        return match environment_value("TERM_PROGRAM_VERSION") {
            Some(version) => sanitize_terminal_token(format!("{program}/{version}")),
            None => sanitize_terminal_token(program),
        };
    }

    // Apply the remaining environment-only terminal detectors in Codex order.
    let detected = if environment_present("WEZTERM_VERSION") {
        terminal_with_version("WezTerm", "WEZTERM_VERSION")
    } else if any_environment_present(&["ITERM_SESSION_ID", "ITERM_PROFILE", "ITERM_PROFILE_NAME"])
    {
        "iTerm.app".to_owned()
    } else if environment_present("TERM_SESSION_ID") {
        "Apple_Terminal".to_owned()
    } else if environment_present("KITTY_WINDOW_ID")
        || environment_value("TERM").is_some_and(|term| term.contains("kitty"))
    {
        "kitty".to_owned()
    } else if environment_present("ALACRITTY_SOCKET")
        || environment_value("TERM").as_deref() == Some("alacritty")
    {
        "Alacritty".to_owned()
    } else if environment_present("KONSOLE_VERSION") {
        terminal_with_version("Konsole", "KONSOLE_VERSION")
    } else if environment_present("GNOME_TERMINAL_SCREEN") {
        "gnome-terminal".to_owned()
    } else if environment_present("VTE_VERSION") {
        terminal_with_version("VTE", "VTE_VERSION")
    } else if environment_present("WT_SESSION") {
        "WindowsTerminal".to_owned()
    } else if let Some(term) = environment_value("TERM") {
        term
    } else {
        "unknown".to_owned()
    };
    sanitize_terminal_token(detected)
}

/// Formats a known terminal token with a non-empty environment version when available.
fn terminal_with_version(name: &str, variable: &str) -> String {
    environment_value(variable)
        .map_or_else(|| name.to_owned(), |version| format!("{name}/{version}"))
}

/// Returns the first-class presence state used by Codex terminal detection.
fn any_environment_present(names: &[&str]) -> bool {
    names.iter().any(|name| environment_present(name))
}

/// Returns whether a Unicode environment value is present, including an empty value.
fn environment_present(name: &str) -> bool {
    std::env::var(name).is_ok()
}

/// Reads one non-empty Unicode environment value without exposing it outside identity assembly.
fn environment_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Replaces terminal-token characters outside Codex's HTTP-safe token alphabet.
fn sanitize_terminal_token(value: String) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// Applies Codex's printable-ASCII fallback for the complete User-Agent value.
fn sanitize_user_agent(candidate: String) -> String {
    if HeaderValue::from_str(&candidate).is_ok() {
        return candidate;
    }

    let sanitized = candidate
        .chars()
        .map(|character| {
            if matches!(character, ' '..='~') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if !sanitized.is_empty() && HeaderValue::from_str(&sanitized).is_ok() {
        sanitized
    } else {
        CODEX_ORIGINATOR.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_matches_the_pinned_codex_format_and_hides_the_complete_user_agent() {
        // Build the known Windows example through the ported source-compatible formatter.
        let identity =
            CodexRequestIdentity::for_test("Windows", "11", "x86_64", "WindowsTerminal/1.0");
        let headers = identity.request_headers();
        assert_eq!(
            headers[USER_AGENT],
            "codex_cli_rs/0.145.0 (Windows 11; x86_64) WindowsTerminal/1.0"
        );
        assert_eq!(headers["originator"], "codex_cli_rs");
        assert!(!headers.contains_key("version"));

        // Keep the complete environment-bearing value out of Debug output.
        assert!(!format!("{identity:?}").contains("WindowsTerminal/1.0"));
    }
}
