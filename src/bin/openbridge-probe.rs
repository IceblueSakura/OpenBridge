//! Local CLI for explicit upstream model discovery and basic registered-API probes.
//!
//! The tool prints only a JSON report, does not start the downstream HTTP service, and does not
//! modify the code registry.

use std::{collections::BTreeSet, env};

use anyhow::{Context, Result};
use openbridge::{
    config::BootstrapConfigPath,
    probe::{
        ProbeGenerationMode, ProbeOptions, ProbeReasoningEffort, probe_upstream_target,
        probe_upstream_target_with_oauth2,
    },
    provider::ProviderKind,
    providers::build_compiled_registry_with_active_pools,
    transport::upstream::UpstreamClient,
    upstream_credentials::UpstreamCredentialConfigPath,
};

#[tokio::main]
/// Parses probe arguments, binds the trusted target, and prints a redacted observation report.
async fn main() -> Result<()> {
    // Parse CLI selections.
    let arguments = ProbeArguments::parse(env::args().skip(1))?;

    // Load bootstrap and private credential configuration before compiling active Target eligibility.
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let upstream_credentials_file = bootstrap.upstream_credentials_file().to_owned();
    let upstream_configuration = UpstreamCredentialConfigPath::new(&upstream_credentials_file)
        .load()
        .context("failed to load upstream credentials")?;

    // Derive the redacted pool activation set without retaining credential material in the registry.
    let active_pool_ids = upstream_configuration
        .active_pool_ids()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    // Compile only the statically registered Targets selected by the startup pool set.
    let registry = build_compiled_registry_with_active_pools(bootstrap, &active_pool_ids)
        .context("failed to build OpenBridge code registry")?;
    let target = registry
        .upstream_target(&arguments.upstream_target_id)
        .context("selected upstream target is not registered")?;
    if !target.enabled() {
        anyhow::bail!(
            "configured upstream target '{}' is disabled",
            arguments.upstream_target_id
        );
    }

    // Build the shared upstream client with the same transport constraints as the data plane.
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    // Select the credential lifecycle that matches the fixed target kind without opening unrelated sources.
    let report = if target.kind() == ProviderKind::ChatGpt {
        let oauth2_credentials = upstream_configuration
            .load_oauth2_for(&registry, [target.credential_pool_id()])
            .context("failed to bind the selected ChatGPT OAuth2 credential")?;
        probe_upstream_target_with_oauth2(
            &registry,
            &arguments.upstream_target_id,
            &upstream,
            &oauth2_credentials,
            arguments.selection,
        )
        .await
    } else {
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
    }
    .context("probe could not be prepared")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("probe report is serializable")
    );
    Ok(())
}

struct ProbeArguments {
    upstream_target_id: String,
    selection: ProbeOptions,
}

impl ProbeArguments {
    /// Parses command-line selectors into one target and a fixed set of probes.
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        // Parse target and probe selections one at a time and reject undeclared CLI arguments.
        let mut upstream_target_id = None;
        let mut selection = ProbeOptions::default();
        let mut explicit_modes = Vec::new();
        let mut explicit_reasoning = Vec::new();
        let mut reasoning_all = false;
        let mut seen_flags = BTreeSet::new();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--target" => {
                    if upstream_target_id.is_some() {
                        anyhow::bail!("--target may be provided only once");
                    }
                    let value = arguments
                        .next()
                        .filter(|value| !value.starts_with("--"))
                        .context("--target requires a configured upstream target id")?;
                    upstream_target_id = Some(value);
                }
                "--model" => {
                    if selection.upstream_model.is_some() {
                        anyhow::bail!("--model may be provided only once");
                    }
                    selection.upstream_model = Some(
                        arguments
                            .next()
                            .filter(|value| !value.starts_with("--"))
                            .context("--model requires an upstream model id")?,
                    );
                }
                "--list-models" => {
                    reject_duplicate_flag(&mut seen_flags, "--list-models")?;
                    selection.list_models = true;
                }
                "--chat" => {
                    reject_duplicate_flag(&mut seen_flags, "--chat")?;
                    selection.chat = true;
                }
                "--responses" => {
                    reject_duplicate_flag(&mut seen_flags, "--responses")?;
                    selection.responses = true;
                }
                "--embeddings" => {
                    reject_duplicate_flag(&mut seen_flags, "--embeddings")?;
                    selection.embeddings = true;
                }
                "--all" => {
                    reject_duplicate_flag(&mut seen_flags, "--all")?;
                    select_all_operations(&mut selection);
                }
                "--streaming" => {
                    reject_duplicate_flag(&mut seen_flags, "--streaming")?;
                    explicit_modes.push(ProbeGenerationMode::Streaming);
                }
                "--non-streaming" => {
                    reject_duplicate_flag(&mut seen_flags, "--non-streaming")?;
                    explicit_modes.push(ProbeGenerationMode::NonStreaming);
                }
                "--allow-unbounded-streaming-output" => {
                    reject_duplicate_flag(&mut seen_flags, "--allow-unbounded-streaming-output")?;
                    selection.allow_unbounded_streaming_output = true;
                }
                "--reasoning" => {
                    let value = arguments
                        .next()
                        .context("--reasoning requires a level or 'all'")?;
                    if value == "all" {
                        if reasoning_all || !explicit_reasoning.is_empty() {
                            anyhow::bail!(
                                "--reasoning all cannot be repeated or combined with explicit levels"
                            );
                        }
                        reasoning_all = true;
                    } else {
                        if reasoning_all {
                            anyhow::bail!(
                                "explicit --reasoning levels cannot be combined with --reasoning all"
                            );
                        }
                        let effort = ProbeReasoningEffort::from_wire(&value).with_context(|| {
                            format!(
                                "invalid --reasoning value '{value}'; expected omitted, none, minimal, low, medium, high, xhigh, max, or all"
                            )
                        })?;
                        if explicit_reasoning.contains(&effort) {
                            anyhow::bail!("--reasoning {value} may be provided only once");
                        }
                        explicit_reasoning.push(effort);
                    }
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
            select_all_operations(&mut selection);
        }
        if !explicit_modes.is_empty() {
            selection.generation_modes = explicit_modes;
        }
        if reasoning_all {
            selection.reasoning_efforts = ProbeReasoningEffort::ALL.to_vec();
        } else if !explicit_reasoning.is_empty() {
            selection.reasoning_efforts = explicit_reasoning;
        }
        selection.validate().context("invalid probe selection")?;
        Ok(Self {
            upstream_target_id,
            selection,
        })
    }
}

fn select_all_operations(selection: &mut ProbeOptions) {
    selection.list_models = true;
    selection.chat = true;
    selection.responses = true;
    selection.embeddings = true;
}

fn reject_duplicate_flag(seen: &mut BTreeSet<&'static str>, flag: &'static str) -> Result<()> {
    if !seen.insert(flag) {
        anyhow::bail!("{flag} may be provided only once");
    }
    Ok(())
}

/// Prints local probe usage without credentials or runtime state.
fn print_usage() {
    println!(
        "Usage: cargo run --bin openbridge-probe -- --target <id> [--model <upstream-model-id>] [--list-models] [--chat] [--responses] [--embeddings] [--all] [--streaming] [--non-streaming] [--reasoning <level|all>]... [--allow-unbounded-streaming-output]\n\
         \n\
         No probe selector runs --all. Generation defaults to both delivery modes, omitted reasoning, and a 16-token upstream output limit; --reasoning all explicitly runs omitted plus none/minimal/low/medium/high/xhigh/max. --allow-unbounded-streaming-output explicitly removes that limit from streaming probes for backends that reject it and may increase cost. Candidate Generation probes require a Generation-capable target. Enabled API-key targets use configured credentials; ChatGPT targets use the selected OAuth2 auth bundle. --model correlates Models visibility and changes only the model field on fixed Generation probes; it requires --list-models, --chat, or --responses and cannot change endpoint, path, credential, headers, or prompt. The command prints a redacted report and never modifies the code registry."
    );
}

#[cfg(test)]
mod tests {
    //! Verifies probe CLI target and fixed-selector parsing.

    use super::ProbeArguments;
    use openbridge::probe::{ProbeGenerationMode, ProbeOptions, ProbeReasoningEffort};

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

        let alternate =
            parse(&["--target", "openai-main", "--list-models", "--embeddings"]).unwrap();
        assert_eq!(
            alternate.selection,
            ProbeOptions {
                list_models: true,
                embeddings: true,
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

        let swallowed_flag = parse(&["--target", "openai-main", "--model", "--chat"])
            .err()
            .unwrap();
        assert!(swallowed_flag.to_string().contains("--model requires"));

        // Reject arguments outside the closed basic-probe selector set.
        let unknown = parse(&["--target", "openai-main", "--unknown"])
            .err()
            .unwrap();
        assert!(unknown.to_string().contains("unknown argument '--unknown'"));

        let removed = parse(&["--target", "openai-main", "--function-calling"])
            .err()
            .unwrap();
        assert!(
            removed
                .to_string()
                .contains("unknown argument '--function-calling'")
        );
    }

    #[test]
    fn parser_accepts_a_custom_model_generation_matrix() {
        // Select one unregistered upstream model, both protocols, one delivery, and every effort.
        let parsed = parse(&[
            "--target",
            "openai-main",
            "--model",
            "candidate-model",
            "--chat",
            "--responses",
            "--streaming",
            "--reasoning",
            "all",
        ])
        .unwrap();

        assert_eq!(
            parsed.selection,
            ProbeOptions {
                chat: true,
                responses: true,
                upstream_model: Some("candidate-model".to_owned()),
                generation_modes: vec![ProbeGenerationMode::Streaming],
                reasoning_efforts: ProbeReasoningEffort::ALL.to_vec(),
                ..ProbeOptions::default()
            }
        );

        // Matrix-only selectors still default operations to all without losing their values.
        let implicit_all = parse(&[
            "--target",
            "openai-main",
            "--model",
            "candidate-model",
            "--non-streaming",
            "--reasoning",
            "low",
        ])
        .unwrap();
        assert!(implicit_all.selection.list_models);
        assert!(implicit_all.selection.chat);
        assert!(implicit_all.selection.responses);
        assert!(implicit_all.selection.embeddings);
        assert_eq!(
            implicit_all.selection.upstream_model.as_deref(),
            Some("candidate-model")
        );
        assert_eq!(
            implicit_all.selection.generation_modes,
            [ProbeGenerationMode::NonStreaming]
        );
        assert_eq!(
            implicit_all.selection.reasoning_efforts,
            [ProbeReasoningEffort::Low]
        );

        let unbounded = parse(&[
            "--target",
            "openai-main",
            "--responses",
            "--streaming",
            "--allow-unbounded-streaming-output",
        ])
        .unwrap();
        assert!(unbounded.selection.allow_unbounded_streaming_output);
    }

    #[test]
    fn parser_rejects_invalid_or_duplicate_matrix_selectors() {
        for arguments in [
            vec!["--target", "openai-main", "--model", ""],
            vec!["--target", "openai-main", "--reasoning", "extreme"],
            vec!["--target", "openai-main", "--chat", "--chat"],
            vec![
                "--target",
                "openai-main",
                "--reasoning",
                "all",
                "--reasoning",
                "high",
            ],
            vec!["--target", "openai-main", "--streaming", "--streaming"],
            vec![
                "--target",
                "openai-main",
                "--allow-unbounded-streaming-output",
                "--allow-unbounded-streaming-output",
            ],
        ] {
            assert!(
                parse(&arguments).is_err(),
                "accepted invalid args: {arguments:?}"
            );
        }
    }
}
