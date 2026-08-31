//! Local CLI for explicit upstream model discovery and basic registered-API probes.
//!
//! The tool prints only a JSON report, does not start the downstream HTTP service, and does not
//! modify the code registry.

use std::{collections::BTreeSet, env};

use anyhow::{Context, Result};
use openbridge::{
    config::BootstrapConfigPath,
    probe::{
        ProbeGenerationCapability, ProbeGenerationMode, ProbeOptions, ProbeReasoningEffort,
        probe_upstream_target, probe_upstream_target_with_oauth2, resolve_generation_probe_target,
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
    let report = if target.kind() == ProviderKind::ChatGpt {
        let oauth2_credentials = upstream_configuration
            .load_oauth2_for(&registry, [target.credential_pool_id()])
            .context("failed to bind the selected ChatGPT OAuth2 credential")?;
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

        // Parse only closed built-in axes; no endpoint, path, prompt, schema, or body is accepted.
        let mut provider = None;
        let mut explicit_target = None;
        let mut selection = ProbeOptions::default();
        let mut protocols = Vec::new();
        let mut modes = Vec::new();
        let mut capabilities = Vec::new();
        let mut reasoning = Vec::new();
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
                    let value =
                        next_value(&mut arguments, "--protocol", "chat, responses, or all")?;
                    match value.as_str() {
                        "chat" => push_unique(&mut protocols, "chat")?,
                        "responses" => push_unique(&mut protocols, "responses")?,
                        "all" if protocols.is_empty() => {
                            protocols.extend(["chat", "responses"]);
                        }
                        "all" => {
                            anyhow::bail!("--protocol all cannot be combined with other values")
                        }
                        _ => anyhow::bail!("invalid --protocol value '{value}'"),
                    }
                }
                "--delivery" if command == "generation" => {
                    let value = next_value(
                        &mut arguments,
                        "--delivery",
                        "non-streaming, streaming, or all",
                    )?;
                    match value.as_str() {
                        "non-streaming" => {
                            push_unique(&mut modes, ProbeGenerationMode::NonStreaming)?
                        }
                        "streaming" => push_unique(&mut modes, ProbeGenerationMode::Streaming)?,
                        "all" if modes.is_empty() => modes.extend(ProbeGenerationMode::ALL),
                        "all" => {
                            anyhow::bail!("--delivery all cannot be combined with other values")
                        }
                        _ => anyhow::bail!("invalid --delivery value '{value}'"),
                    }
                }
                "--capability" if command == "generation" => {
                    let value = next_value(
                        &mut arguments,
                        "--capability",
                        "text, json-object, json-schema, json-schema-strict, or all",
                    )?;
                    if value == "all" {
                        if !capabilities.is_empty() {
                            anyhow::bail!("--capability all cannot be combined with other values");
                        }
                        capabilities.extend(ProbeGenerationCapability::ALL);
                    } else {
                        let capability = ProbeGenerationCapability::from_wire(&value)
                            .with_context(|| format!("--capability '{value}' is unknown"))?;
                        push_unique(&mut capabilities, capability)?;
                    }
                }
                "--reasoning" => {
                    if command != "generation" {
                        anyhow::bail!("--reasoning is valid only for generation");
                    }
                    let value = next_value(&mut arguments, "--reasoning", "a level or all")?;
                    if value == "all" {
                        if !reasoning.is_empty() {
                            anyhow::bail!("--reasoning all cannot be combined with other values");
                        }
                        reasoning.extend(ProbeReasoningEffort::ALL);
                    } else {
                        let effort = ProbeReasoningEffort::from_wire(&value)
                            .with_context(|| format!("invalid --reasoning value '{value}'"))?;
                        push_unique(&mut reasoning, effort)?;
                    }
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
                if protocols.is_empty() {
                    protocols.extend(["chat", "responses"]);
                }
                selection.chat = protocols.contains(&"chat");
                selection.responses = protocols.contains(&"responses");
                selection.generation_modes = if modes.is_empty() {
                    vec![ProbeGenerationMode::NonStreaming]
                } else {
                    modes
                };
                selection.generation_capabilities = if capabilities.is_empty() {
                    vec![ProbeGenerationCapability::Text]
                } else {
                    capabilities
                };
                selection.reasoning_efforts = if reasoning.is_empty() {
                    vec![ProbeReasoningEffort::Omitted]
                } else {
                    reasoning
                };
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

fn push_unique<T: Copy + Eq>(values: &mut Vec<T>, value: T) -> Result<()> {
    if values.contains(&value) {
        anyhow::bail!("probe matrix values must not be repeated");
    }
    values.push(value);
    Ok(())
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
         cargo run --bin openbridge-probe -- generation --provider <slug> --model <upstream-model-id> [--target <id>] [--protocol <chat|responses|all>]... [--capability <text|json-object|json-schema|json-schema-strict|all>]... [--delivery <non-streaming|streaming|all>]... [--reasoning <level|all>]... [--allow-unbounded-streaming-output]\n\
         \n\
         Generation defaults to both protocols, non-streaming delivery, omitted reasoning, and the text capability. Structured cases use a fixed conflict prompt and fixed schema, then report supported, not_honored, or inconclusive without retaining generated text. Explicit all values expand potentially billable requests. --allow-unbounded-streaming-output removes only the fixed streaming output budget and may increase cost. Provider selection resolves only registered enabled Generation Targets; --target disambiguates trusted deployments and cannot change endpoint, path, credential, headers, prompt, or schema. The command prints a redacted report and never modifies the code registry."
    );
}

#[cfg(test)]
mod tests {
    //! Verifies probe CLI target and fixed-selector parsing.

    use super::ProbeArguments;
    use openbridge::{
        probe::{
            ProbeGenerationCapability, ProbeGenerationMode, ProbeOptions, ProbeReasoningEffort,
        },
        provider::ProviderKind,
    };

    fn parse(arguments: &[&str]) -> anyhow::Result<ProbeArguments> {
        ProbeArguments::parse(arguments.iter().map(|argument| (*argument).to_owned()))
    }

    #[test]
    fn parser_defaults_to_models_discovery_or_bounded_generation() {
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
        assert!(generation.selection.chat);
        assert!(generation.selection.responses);
        assert_eq!(
            generation.selection.generation_modes,
            [ProbeGenerationMode::NonStreaming]
        );
        assert_eq!(
            generation.selection.generation_capabilities,
            [ProbeGenerationCapability::Text]
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
    fn parser_accepts_an_explicit_generation_matrix_and_target_disambiguation() {
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
            "--capability",
            "json-schema-strict",
            "--reasoning",
            "all",
            "--allow-unbounded-streaming-output",
        ])
        .unwrap();
        assert_eq!(parsed.explicit_target.as_deref(), Some("deepseek-primary"));
        assert!(!parsed.selection.chat);
        assert!(parsed.selection.responses);
        assert_eq!(
            parsed.selection.generation_modes,
            [ProbeGenerationMode::Streaming]
        );
        assert_eq!(
            parsed.selection.generation_capabilities,
            [ProbeGenerationCapability::JsonSchemaStrict]
        );
        assert_eq!(
            parsed.selection.reasoning_efforts,
            ProbeReasoningEffort::ALL.to_vec()
        );
        assert!(parsed.selection.allow_unbounded_streaming_output);

        let parsed = parse(&[
            "generation",
            "--provider",
            "deepseek",
            "--model",
            "candidate-model",
            "--capability",
            "text",
            "--capability",
            "json-object",
        ])
        .unwrap();
        assert_eq!(
            parsed.selection.generation_capabilities,
            [
                ProbeGenerationCapability::Text,
                ProbeGenerationCapability::JsonObject,
            ]
        );
    }

    #[test]
    fn parser_rejects_invalid_or_duplicate_matrix_selectors() {
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
                "extreme",
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
                "all",
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
                "--delivery",
                "streaming",
            ],
        ] {
            assert!(
                parse(&arguments).is_err(),
                "accepted invalid args: {arguments:?}"
            );
        }
    }
}
