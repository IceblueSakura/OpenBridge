//! Local CLI for explicit upstream model discovery and basic registered-API probes.
//!
//! The tool prints only a JSON report, does not start the downstream HTTP service, and does not
//! modify the code registry.

use std::{collections::BTreeSet, env};

use anyhow::{Context, Result};
use openbridge::{
    config::BootstrapConfigPath,
    probe::{
        ProbeGenerationCase, ProbeGenerationMode, ProbeGenerationSelection, ProbeOptions,
        ProbeProtocol, probe_upstream_target, probe_upstream_target_with_oauth2,
        resolve_generation_probe_target,
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
    let upstream_target_id = resolve_generation_probe_target(
        &registry,
        arguments.provider,
        arguments.explicit_target.as_deref(),
    )
    .context("failed to select a trusted Provider deployment")?;
    let target = registry
        .upstream_target(&upstream_target_id)
        .context("selected upstream target is not registered")?;
    if !target.enabled() {
        anyhow::bail!(
            "configured upstream target '{}' is disabled",
            upstream_target_id
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
    let report = if matches!(target.kind(), ProviderKind::ChatGpt | ProviderKind::Grok) {
        let oauth2_credentials = upstream_configuration
            .load_oauth2_for(&registry, [target.credential_pool_id()])
            .context("failed to bind the selected OAuth2 credential")?;
        probe_upstream_target_with_oauth2(
            &registry,
            &upstream_target_id,
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
            &upstream_target_id,
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

#[derive(Debug)]
struct ProbeArguments {
    provider: ProviderKind,
    explicit_target: Option<String>,
    selection: ProbeOptions,
}

impl ProbeArguments {
    /// Parses one provider-scoped discovery or Generation capability command.
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut arguments = arguments.into_iter();
        let command = arguments
            .next()
            .context("a probe command is required; expected models or generation")?;
        if matches!(command.as_str(), "--help" | "-h") {
            print_usage();
            std::process::exit(0);
        }
        if !matches!(command.as_str(), "models" | "generation") {
            anyhow::bail!("unknown probe command '{command}'; expected models or generation");
        }

        // Parse one closed built-in case; no endpoint, path, prompt, schema, or body is accepted.
        let mut provider = None;
        let mut explicit_target = None;
        let mut selection = ProbeOptions::default();
        let mut protocol = None;
        let mut mode = None;
        let mut generation_case = None;
        let mut custom_prompt = None;
        let mut custom_schema = None;
        let mut custom_schema_name = None;
        let mut seen_flags = BTreeSet::new();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--provider" => {
                    reject_duplicate_flag(&mut seen_flags, "--provider")?;
                    let value = next_value(&mut arguments, "--provider", "a Provider slug")?;
                    provider = Some(parse_provider(&value)?);
                }
                "--target" => {
                    reject_duplicate_flag(&mut seen_flags, "--target")?;
                    explicit_target = Some(next_value(
                        &mut arguments,
                        "--target",
                        "a configured upstream Target ID",
                    )?);
                }
                "--model" => {
                    reject_duplicate_flag(&mut seen_flags, "--model")?;
                    selection.upstream_model = Some(next_value(
                        &mut arguments,
                        "--model",
                        "an upstream model ID",
                    )?);
                }
                "--allow-unbounded-streaming-output" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--allow-unbounded-streaming-output")?;
                    selection.allow_unbounded_streaming_output = true;
                }
                "--protocol" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--protocol")?;
                    let value = next_value(&mut arguments, "--protocol", "chat or responses")?;
                    protocol = Some(match value.as_str() {
                        "chat" => ProbeProtocol::ChatCompletions,
                        "responses" => ProbeProtocol::Responses,
                        _ => anyhow::bail!("invalid --protocol value '{value}'"),
                    });
                }
                "--delivery" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--delivery")?;
                    let value =
                        next_value(&mut arguments, "--delivery", "non-streaming or streaming")?;
                    mode = Some(match value.as_str() {
                        "non-streaming" => ProbeGenerationMode::NonStreaming,
                        "streaming" => ProbeGenerationMode::Streaming,
                        _ => anyhow::bail!("invalid --delivery value '{value}'"),
                    });
                }
                "--case" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--case")?;
                    let value = next_value(&mut arguments, "--case", "a built-in unit case")?;
                    generation_case = Some(
                        ProbeGenerationCase::from_wire(&value)
                            .with_context(|| format!("unknown --case value '{value}'"))?,
                    );
                }
                "--prompt" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--prompt")?;
                    let value = next_value(
                        &mut arguments,
                        "--prompt",
                        "an admin-authored prompt of at most 4096 bytes",
                    )?;
                    custom_prompt = Some(value);
                }
                "--schema" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--schema")?;
                    let value = next_value(
                        &mut arguments,
                        "--schema",
                        "an admin-authored JSON schema object of at most 8192 bytes",
                    )?;
                    custom_schema = Some(value);
                }
                "--schema-name" if command == "generation" => {
                    reject_duplicate_flag(&mut seen_flags, "--schema-name")?;
                    let value = next_value(
                        &mut arguments,
                        "--schema-name",
                        "an admin-authored response-format schema name",
                    )?;
                    custom_schema_name = Some(value);
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("unknown argument '{argument}'; run with --help"),
            }
        }

        let provider = provider.context("--provider is required")?;
        match command.as_str() {
            "models" => selection.list_models = true,
            "generation" => {
                if selection.upstream_model.is_none() {
                    anyhow::bail!("generation requires --model");
                }
                selection.generation = Some(ProbeGenerationSelection {
                    protocol: protocol.unwrap_or(ProbeProtocol::ChatCompletions),
                    mode: mode.unwrap_or(ProbeGenerationMode::NonStreaming),
                    case: generation_case.unwrap_or(ProbeGenerationCase::Text),
                    custom_prompt,
                    custom_schema,
                    custom_schema_name,
                });
            }
            _ => unreachable!(),
        }
        selection.validate().context("invalid probe selection")?;
        Ok(Self {
            provider,
            explicit_target,
            selection,
        })
    }
}

fn next_value(
    arguments: &mut impl Iterator<Item = String>,
    flag: &str,
    expected: &str,
) -> Result<String> {
    arguments
        .next()
        .filter(|value| !value.starts_with("--"))
        .with_context(|| format!("{flag} requires {expected}"))
}

fn parse_provider(value: &str) -> Result<ProviderKind> {
    match value {
        "chatgpt" => Ok(ProviderKind::ChatGpt),
        "grok" => Ok(ProviderKind::Grok),
        "openai" => Ok(ProviderKind::OpenAi),
        "longcat" => Ok(ProviderKind::LongCat),
        "deepseek" => Ok(ProviderKind::DeepSeek),
        "mimo" => Ok(ProviderKind::MiMo),
        "openrouter" => Ok(ProviderKind::OpenRouter),
        "nvidia" => Ok(ProviderKind::Nvidia),
        "bailian" => Ok(ProviderKind::Bailian),
        "kimi-cn" => Ok(ProviderKind::KimiCn),
        "zhipu-cn" => Ok(ProviderKind::ZhipuCn),
        _ => anyhow::bail!("unknown Provider '{value}'"),
    }
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
        "Usage:\n\
         cargo run --bin openbridge-probe -- models --provider <slug> [--target <id>] [--model <upstream-model-id>]\n\
         cargo run --bin openbridge-probe -- generation --provider <slug> --model <upstream-model-id> [--target <id>] [--protocol <chat|responses>] [--delivery <non-streaming|streaming>] [--case <text|reasoning-none|reasoning-minimal|reasoning-low|reasoning-medium|reasoning-high|reasoning-xhigh|reasoning-max|json-object|json-schema|json-schema-strict|image-input-inline-png|tool-auto|tool-none|tool-required|tool-named|tool-strict|tool-parallel-false|tool-parallel-true|reasoning-summary|include-encrypted-content|prompt-cache-key>] [--prompt <text>] [--schema <json>] [--schema-name <name>] [--allow-unbounded-streaming-output]\n\
         \n\
         Generation executes exactly one unit case and defaults to Chat, non-streaming delivery, and text. Reasoning is encoded by reasoning-* cases rather than a separate matrix axis. Every bounded case uses a 4096-token accuracy-oriented output budget, clamped by a registered model ceiling. Structured, inline-image, and first-turn function-tool cases use fixed prompts and assets, then report supported, not_honored, or inconclusive without retaining generated text, image bytes, or arguments. Tool cases never execute a tool or send continuation state. --prompt overrides the case's fixed user prompt (not allowed for tool cases); --schema and --schema-name override only a json-schema/json-schema-strict case's response-format object and name, and force an inconclusive verdict because no fixed oracle can judge an arbitrary schema. The report carries SHA-256 fingerprints of any override so evidence stays attributable, but never the override text. --allow-unbounded-streaming-output removes only the fixed streaming output budget and may increase cost. Provider selection resolves only registered enabled Generation Targets; --target disambiguates trusted deployments and cannot change endpoint, path, credential, headers, or tools. The command prints a redacted report and never modifies the code registry."
    );
}

#[cfg(test)]
mod tests {
    //! Verifies probe CLI target and one-case selector parsing.

    use super::ProbeArguments;
    use openbridge::{
        probe::{
            ProbeGenerationCase, ProbeGenerationMode, ProbeGenerationSelection, ProbeOptions,
            ProbeProtocol,
        },
        provider::ProviderKind,
    };

    fn parse(arguments: &[&str]) -> anyhow::Result<ProbeArguments> {
        ProbeArguments::parse(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn parser_defaults_to_models_discovery_or_one_bounded_chat_text_case() {
        let models = parse(&["models", "--provider", "bailian", "--model", "candidate"]).unwrap();
        assert_eq!(models.provider, ProviderKind::Bailian);
        assert!(models.explicit_target.is_none());
        assert_eq!(
            models.selection,
            ProbeOptions {
                list_models: true,
                upstream_model: Some("candidate".to_owned()),
                ..ProbeOptions::default()
            }
        );

        let generation = parse(&[
            "generation",
            "--provider",
            "deepseek",
            "--model",
            "candidate",
        ])
        .unwrap();
        assert_eq!(
            generation.selection.generation,
            Some(ProbeGenerationSelection {
                protocol: ProbeProtocol::ChatCompletions,
                mode: ProbeGenerationMode::NonStreaming,
                case: ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            })
        );
    }

    #[test]
    fn parser_rejects_missing_provider_model_and_arbitrary_inputs() {
        assert!(parse(&[]).is_err());
        assert!(
            parse(&["models"])
                .unwrap_err()
                .to_string()
                .contains("--provider")
        );
        assert!(
            parse(&["generation", "--provider", "bailian"])
                .unwrap_err()
                .to_string()
                .contains("requires --model")
        );
        assert!(
            parse(&["models", "--provider", "unknown"])
                .unwrap_err()
                .to_string()
                .contains("unknown Provider")
        );
        assert!(
            parse(&[
                "models",
                "--provider",
                "bailian",
                "--url",
                "https://example.com"
            ])
            .unwrap_err()
            .to_string()
            .contains("unknown argument '--url'")
        );
    }

    #[test]
    fn parser_accepts_one_explicit_generation_case_and_target_disambiguation() {
        let parsed = parse(&[
            "generation",
            "--provider",
            "deepseek",
            "--target",
            "deepseek-primary",
            "--model",
            "candidate-model",
            "--protocol",
            "responses",
            "--delivery",
            "streaming",
            "--case",
            "reasoning-high",
            "--allow-unbounded-streaming-output",
        ])
        .unwrap();
        assert_eq!(parsed.explicit_target.as_deref(), Some("deepseek-primary"));
        assert_eq!(
            parsed.selection.generation,
            Some(ProbeGenerationSelection {
                protocol: ProbeProtocol::Responses,
                mode: ProbeGenerationMode::Streaming,
                case: ProbeGenerationCase::ReasoningHigh,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            })
        );
        assert!(parsed.selection.allow_unbounded_streaming_output);
    }

    #[test]
    fn parser_maps_every_closed_unit_case_without_accepting_lists() {
        for (wire, expected) in [
            ("text", ProbeGenerationCase::Text),
            ("reasoning-none", ProbeGenerationCase::ReasoningNone),
            ("reasoning-minimal", ProbeGenerationCase::ReasoningMinimal),
            ("reasoning-low", ProbeGenerationCase::ReasoningLow),
            ("reasoning-medium", ProbeGenerationCase::ReasoningMedium),
            ("reasoning-high", ProbeGenerationCase::ReasoningHigh),
            ("reasoning-xhigh", ProbeGenerationCase::ReasoningXHigh),
            ("reasoning-max", ProbeGenerationCase::ReasoningMax),
            ("json-object", ProbeGenerationCase::JsonObject),
            ("json-schema", ProbeGenerationCase::JsonSchema),
            ("json-schema-strict", ProbeGenerationCase::JsonSchemaStrict),
            (
                "image-input-inline-png",
                ProbeGenerationCase::ImageInputInlinePng,
            ),
            ("tool-auto", ProbeGenerationCase::ToolAuto),
            ("tool-none", ProbeGenerationCase::ToolNone),
            ("tool-required", ProbeGenerationCase::ToolRequired),
            ("tool-named", ProbeGenerationCase::ToolNamed),
            ("tool-strict", ProbeGenerationCase::ToolStrict),
            (
                "tool-parallel-false",
                ProbeGenerationCase::ToolParallelDisabled,
            ),
            (
                "tool-parallel-true",
                ProbeGenerationCase::ToolParallelEnabled,
            ),
        ] {
            let parsed = parse(&[
                "generation",
                "--provider",
                "deepseek",
                "--model",
                "candidate-model",
                "--case",
                wire,
            ])
            .unwrap();
            assert_eq!(parsed.selection.generation.unwrap().case, expected);
        }
    }

    #[test]
    fn parser_rejects_matrix_or_duplicate_selectors() {
        for arguments in [
            vec!["models", "--provider", "bailian", "--reasoning", "high"],
            vec!["models", "--provider", "bailian", "--capability", "text"],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--protocol",
                "all",
            ],
            vec![
                "models",
                "--provider",
                "bailian",
                "--allow-unbounded-streaming-output",
            ],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--reasoning",
                "high",
            ],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--delivery",
                "all",
            ],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--capability",
                "text",
            ],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--case",
                "text",
                "--case",
                "json-object",
            ],
            vec![
                "generation",
                "--provider",
                "bailian",
                "--model",
                "m",
                "--delivery",
                "non-streaming",
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
