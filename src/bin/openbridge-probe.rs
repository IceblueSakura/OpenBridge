//! Local CLI for explicit upstream model discovery and protocol capability probes.
//!
//! The tool prints only a JSON report, does not start the downstream HTTP service, and does not
//! modify the code registry.

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use openbridge::{
    codex_auth::load_codex_auth_file_for_target,
    codex_identity::CodexRequestIdentity,
    config::BootstrapConfigPath,
    probe::{ProbeOptions, probe_chatgpt_upstream_target, probe_upstream_target},
    provider::ProviderKind,
    providers::build_compiled_registry,
    transport::upstream::UpstreamClient,
    upstream_credentials::UpstreamCredentialConfigPath,
};

#[tokio::main]
/// Parses probe arguments, binds the trusted target, and prints a redacted capability report.
async fn main() -> Result<()> {
    // Parse CLI selections.
    let arguments = ProbeArguments::parse(env::args().skip(1))?;

    // Load bootstrap and compile the trusted registry before selecting a credential source.
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let upstream_credentials_file = bootstrap.upstream_credentials_file().to_owned();
    let registry =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let target = registry
        .upstream_target(&arguments.upstream_target_id)
        .context("selected upstream target is not registered")?;
    arguments.validate_for_provider(target.kind())?;

    // Build the shared upstream client with the same transport constraints as the data plane.
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    // Load only the Provider-specific credential source and run its closed probe entry point.
    let report = if target.kind() == ProviderKind::ChatGpt {
        let auth_file = arguments
            .codex_auth_file
            .as_deref()
            .expect("ChatGPT arguments were validated");
        let credentials =
            load_codex_auth_file_for_target(auth_file, &registry, &arguments.upstream_target_id)
                .context("failed to load the selected Codex ChatGPT credential")?;
        credentials
            .validate_registry(&registry)
            .context("selected credential violates registry state-affinity constraints")?;
        let identity = CodexRequestIdentity::current();
        probe_chatgpt_upstream_target(
            &registry,
            &arguments.upstream_target_id,
            &upstream,
            &credentials,
            arguments.selection,
            &identity,
        )
        .await
        .context("ChatGPT probe could not be prepared")?
    } else {
        let upstream_configuration = UpstreamCredentialConfigPath::new(upstream_credentials_file)
            .load()
            .context("failed to load upstream credentials")?;
        let credentials = upstream_configuration
            .into_builder_for(&registry, [target.credential_pool_id()])
            .context("failed to bind the selected upstream credential pool")?
            .build();
        credentials
            .validate_registry(&registry)
            .context("selected credential pool violates registry state-affinity constraints")?;
        probe_upstream_target(
            &registry,
            &arguments.upstream_target_id,
            &upstream,
            &credentials,
            arguments.selection,
        )
        .await
        .context("probe could not be prepared")?
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("probe report is serializable")
    );
    Ok(())
}

struct ProbeArguments {
    upstream_target_id: String,
    selection: ProbeOptions,
    selection_explicit: bool,
    codex_auth_file: Option<PathBuf>,
}

impl ProbeArguments {
    /// Parses command-line selectors into one target and a fixed set of probes.
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        // Parse target and probe selections one at a time and reject undeclared CLI arguments.
        let mut upstream_target_id = None;
        let mut selection = ProbeOptions::default();
        let mut selection_explicit = false;
        let mut codex_auth_file = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--target" => {
                    if upstream_target_id.is_some() {
                        anyhow::bail!("--target may be provided only once");
                    }
                    let value = arguments
                        .next()
                        .context("--target requires a configured upstream target id")?;
                    upstream_target_id = Some(value);
                }
                "--list-models" => {
                    selection.list_models = true;
                    selection_explicit = true;
                }
                "--chat" => {
                    selection.chat = true;
                    selection_explicit = true;
                }
                "--responses" => {
                    selection.responses = true;
                    selection_explicit = true;
                }
                "--function-calling" => {
                    selection.function_calling = true;
                    selection_explicit = true;
                }
                "--all" => {
                    selection = ProbeOptions::all();
                    selection_explicit = true;
                }
                "--codex-auth-file" => {
                    if codex_auth_file.is_some() {
                        anyhow::bail!("--codex-auth-file may be provided only once");
                    }
                    codex_auth_file = Some(PathBuf::from(
                        arguments
                            .next()
                            .context("--codex-auth-file requires a path")?,
                    ));
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("unknown argument '{argument}'; run with --help"),
            }
        }
        // Require a target and default to all probes when no selection was specified.
        let upstream_target_id = upstream_target_id.context("--target is required")?;
        if selection.is_empty() {
            selection = ProbeOptions::all();
        }
        Ok(Self {
            upstream_target_id,
            selection,
            selection_explicit,
            codex_auth_file,
        })
    }

    /// Enforces the closed credential and operation selectors for the resolved Provider.
    fn validate_for_provider(&self, provider: ProviderKind) -> Result<()> {
        // Require the Codex auth file and an explicit first-stage selection only for ChatGPT.
        if provider == ProviderKind::ChatGpt {
            if self.codex_auth_file.is_none() {
                anyhow::bail!("ChatGPT probe requires --codex-auth-file");
            }
            if !self.selection_explicit
                || self.selection.is_empty()
                || self.selection.chat
                || self.selection.function_calling
            {
                anyhow::bail!(
                    "ChatGPT first-stage probe accepts only --list-models and --responses"
                );
            }
            return Ok(());
        }

        // Reject the Codex-local auth selector for every ordinary Provider.
        if self.codex_auth_file.is_some() {
            anyhow::bail!("Codex auth-file selector is valid only for the ChatGPT probe target");
        }
        Ok(())
    }
}

/// Prints local probe usage without credentials or runtime state.
fn print_usage() {
    println!(
        "Usage: cargo run --bin openbridge-probe -- --target <id> [--list-models] [--chat] [--responses] [--function-calling] [--all]\n\
         ChatGPT: --target chatgpt-gpt-5-6-sol --codex-auth-file <path> [--list-models] [--responses]\n\
         \n\
         No probe selector runs --all for ordinary targets. ChatGPT requires explicit first-stage selectors. The command only prints a redacted report and never modifies the code registry or Codex auth file."
    );
}

#[cfg(test)]
mod tests {
    //! Verifies probe CLI target and fixed-selector parsing.

    use super::ProbeArguments;
    use openbridge::{probe::ProbeOptions, provider::ProviderKind};

    fn parse(arguments: &[&str]) -> anyhow::Result<ProbeArguments> {
        ProbeArguments::parse(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn parser_defaults_to_all_probes_and_preserves_explicit_selections() {
        // Default an otherwise empty selection to every implemented probe.
        let defaults = parse(&["--target", "openai-main"]).unwrap();
        assert_eq!(defaults.upstream_target_id, "openai-main");
        assert_eq!(defaults.selection, ProbeOptions::all());

        // Preserve each independent selector without implicitly enabling its peers.
        let selected = parse(&["--target", "openai-main", "--chat", "--responses"]).unwrap();
        assert_eq!(
            selected.selection,
            ProbeOptions {
                chat: true,
                responses: true,
                ..ProbeOptions::default()
            }
        );

        let alternate = parse(&[
            "--target",
            "openai-main",
            "--list-models",
            "--function-calling",
        ])
        .unwrap();
        assert_eq!(
            alternate.selection,
            ProbeOptions {
                list_models: true,
                function_calling: true,
                ..ProbeOptions::default()
            }
        );

        // Let the explicit all selector produce the same complete fixed selection.
        let all = parse(&["--all", "--target", "openai-main"]).unwrap();
        assert_eq!(all.selection, ProbeOptions::all());
    }

    #[test]
    fn parser_rejects_missing_targets_and_unknown_arguments() {
        // Require both the target flag and its following value.
        let missing_target = parse(&[]).err().unwrap();
        assert!(missing_target.to_string().contains("--target is required"));

        let missing_value = parse(&["--target"]).err().unwrap();
        assert!(missing_value.to_string().contains("--target requires"));

        // Reject arguments outside the closed CLI selector set.
        let unknown = parse(&["--target", "openai-main", "--unknown"])
            .err()
            .unwrap();
        assert!(unknown.to_string().contains("unknown argument '--unknown'"));
    }

    #[test]
    fn chatgpt_requires_the_auth_file_and_only_first_stage_operations() {
        // Accept the credential and first-stage operations without any Codex executable selector.
        let arguments = parse(&[
            "--target",
            "chatgpt-gpt-5-6-sol",
            "--codex-auth-file",
            "auth.json",
            "--list-models",
            "--responses",
        ])
        .unwrap();
        arguments
            .validate_for_provider(ProviderKind::ChatGpt)
            .unwrap();

        // Reject implicit all, a missing auth path, and out-of-scope operations.
        for input in [
            vec!["--target", "chatgpt-gpt-5-6-sol"],
            vec!["--target", "chatgpt-gpt-5-6-sol", "--responses"],
            vec![
                "--target",
                "chatgpt-gpt-5-6-sol",
                "--codex-auth-file",
                "auth.json",
                "--chat",
            ],
        ] {
            let arguments = parse(&input).unwrap();
            assert!(
                arguments
                    .validate_for_provider(ProviderKind::ChatGpt)
                    .is_err()
            );
        }
    }

    #[test]
    fn ordinary_targets_reject_codex_auth_and_the_cli_has_no_executable_selector() {
        // Keep local Codex credentials out of ordinary Provider probes.
        let arguments = parse(&["--target", "openai-main", "--codex-auth-file", "value"]).unwrap();
        assert!(
            arguments
                .validate_for_provider(ProviderKind::OpenAi)
                .is_err()
        );

        // Reject any attempt to make the OpenBridge runtime launch or depend on a Codex executable.
        let error = parse(&["--target", "chatgpt-gpt-5-6-sol", "--codex-cli", "codex"])
            .err()
            .unwrap();
        assert!(error.to_string().contains("unknown argument '--codex-cli'"));
    }
}
