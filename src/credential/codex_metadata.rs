//! Fixed Codex CLI 0.160.0 default auth metadata, without application suffixes or overrides.
//! Format: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/auth/default_client.rs
//! Terminal precedence: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/terminal-detection/src/lib.rs
//! Both reference sources are Apache-2.0; this module implements only their auth metadata contract.
use reqwest::header::HeaderValue;
use std::sync::LazyLock;

pub(super) const ORIGINATOR: &str = "codex_cli_rs";
const CLI_VERSION: &str = "0.160.0";

pub(super) fn user_agent() -> &'static str {
    // OS discovery may execute platform helpers; never repeat it per auth request.
    static AGENT: LazyLock<String> = LazyLock::new(|| {
        let os = os_info::get();
        format_agent(
            &os.os_type().to_string(),
            &os.version().to_string(),
            os.architecture().unwrap_or("unknown"),
            &terminal_token(|name| std::env::var(name).ok()),
        )
    });
    &AGENT
}

fn format_agent(os: &str, version: &str, architecture: &str, terminal: &str) -> String {
    let candidate =
        format!("{ORIGINATOR}/{CLI_VERSION} ({os} {version}; {architecture}) {terminal}");
    if HeaderValue::from_str(&candidate).is_ok() {
        return candidate;
    }
    candidate
        .chars()
        .map(|ch| if matches!(ch, ' '..='~') { ch } else { '_' })
        .collect()
}

fn terminal_token(read: impl Fn(&str) -> Option<String>) -> String {
    // Read only terminal metadata, not auth/proxy/configuration environment.
    let value = |name| read(name).filter(|s| s.len() <= 256);
    let nonempty = |name| value(name).filter(|s| !s.trim().is_empty());
    let versioned = |name: &str, version: Option<String>| match version {
        Some(version) => format!("{name}/{version}"),
        None => name.to_owned(),
    };
    let term = value("TERM");
    let raw = if let Some(program) =
        nonempty("TERM_PROGRAM").filter(|program| !program.eq_ignore_ascii_case("tmux"))
    {
        versioned(&program, nonempty("TERM_PROGRAM_VERSION"))
    } else if nonempty("GHOSTTY_RESOURCES_DIR").is_some() {
        "Ghostty".into()
    } else if value("WEZTERM_VERSION").is_some() {
        versioned("WezTerm", nonempty("WEZTERM_VERSION"))
    } else if ["ITERM_SESSION_ID", "ITERM_PROFILE", "ITERM_PROFILE_NAME"]
        .iter()
        .any(|name| value(name).is_some())
    {
        "iTerm.app".into()
    } else if value("TERM_SESSION_ID").is_some() {
        "Apple_Terminal".into()
    } else if value("KITTY_WINDOW_ID").is_some()
        || term.as_ref().is_some_and(|term| term.contains("kitty"))
    {
        "kitty".into()
    } else if value("ALACRITTY_SOCKET").is_some() || term.as_deref() == Some("alacritty") {
        "Alacritty".into()
    } else if value("KONSOLE_VERSION").is_some() {
        versioned("Konsole", nonempty("KONSOLE_VERSION"))
    } else if value("GNOME_TERMINAL_SCREEN").is_some() {
        "gnome-terminal".into()
    } else if value("VTE_VERSION").is_some() {
        versioned("VTE", nonempty("VTE_VERSION"))
    } else if value("WT_SESSION").is_some() {
        "WindowsTerminal".into()
    } else {
        nonempty("TERM").unwrap_or_else(|| "unknown".into())
    };
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(fields: &[(&str, &str)]) -> String {
        terminal_token(|name| {
            fields
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn official_agent_shape_uses_os_version_architecture_and_terminal_without_project_suffix() {
        assert_eq!(
            format_agent("NixOS", "26.05", "x86_64", "xterm-256color"),
            "codex_cli_rs/0.160.0 (NixOS 26.05; x86_64) xterm-256color"
        );
        assert_eq!(
            format_agent("Unknown", "Unknown", "unknown", "unknown"),
            "codex_cli_rs/0.160.0 (Unknown Unknown; unknown) unknown"
        );
        assert_eq!(
            format_agent("Linux\r\nInjected", "1", "x86_64", "unknown"),
            "codex_cli_rs/0.160.0 (Linux__Injected 1; x86_64) unknown"
        );
        assert!(HeaderValue::from_str(user_agent()).is_ok());
        assert!(!user_agent().contains("OpenBridge") && !user_agent().contains("openbridge"));
    }

    #[test]
    fn explicit_terminal_program_wins_and_tmux_does_not_hide_the_underlying_terminal() {
        assert_eq!(
            terminal(&[
                ("TERM_PROGRAM", "WezTerm"),
                ("TERM_PROGRAM_VERSION", "2026"),
                ("GHOSTTY_RESOURCES_DIR", "synthetic"),
                ("WT_SESSION", "synthetic")
            ]),
            "WezTerm/2026"
        );
        assert_eq!(
            terminal(&[
                ("TERM_PROGRAM", "TmUx"),
                ("TERM_PROGRAM_VERSION", "3.6"),
                ("GHOSTTY_RESOURCES_DIR", "synthetic"),
                ("TERM", "screen")
            ]),
            "Ghostty"
        );
    }

    #[test]
    fn indicator_presence_version_absence_and_capability_fallback_are_distinct() {
        assert_eq!(terminal(&[("WEZTERM_VERSION", "")]), "WezTerm");
        assert_eq!(terminal(&[("KONSOLE_VERSION", "2604")]), "Konsole/2604");
        assert_eq!(terminal(&[("TERM", "xterm-kitty")]), "kitty");
        assert_eq!(terminal(&[("TERM", "xterm-256color")]), "xterm-256color");
        assert_eq!(terminal(&[("TERM", " \t")]), "unknown");
        assert_eq!(terminal(&[]), "unknown");
    }

    #[test]
    fn terminal_values_are_sanitized_and_bounded_without_reading_unrelated_environment() {
        assert_eq!(
            terminal(&[
                ("TERM_PROGRAM", "term\r\n☃"),
                ("TERM_PROGRAM_VERSION", "1 beta")
            ]),
            "term___/1_beta"
        );
        assert_eq!(
            terminal(&[("TERM_PROGRAM", &"x".repeat(257)), ("TERM", "dumb")]),
            "dumb"
        );
        assert_eq!(
            terminal_token(|name| {
                assert!(
                    name != "CODEX_INTERNAL_ORIGINATOR_OVERRIDE" && name != "USER_AGENT_SUFFIX"
                );
                None
            }),
            "unknown"
        );
    }
}
