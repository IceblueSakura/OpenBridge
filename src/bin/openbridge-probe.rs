//! Local CLI for explicit upstream model discovery and protocol capability probes.
//!
//! The tool prints only a JSON report, does not start the downstream HTTP service, and does not
//! modify the code registry.

use std::env;

use anyhow::{Context, Result};
use openbridge::{
    config::BootstrapConfigPath,
    probe::{ProbeOptions, probe_upstream_target},
    providers::build_compiled_registry,
    transport::upstream::UpstreamClient,
    upstream_credentials::UpstreamCredentialConfigPath,
};

#[tokio::main]
/// Parses probe arguments, binds the trusted target, and prints a redacted capability report.
async fn main() -> Result<()> {
    // Parse CLI selections.
    let arguments = ProbeArguments::parse(env::args().skip(1))?;

    // Load bootstrap and private upstream credential configuration.
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let upstream_configuration =
        UpstreamCredentialConfigPath::new(bootstrap.upstream_credentials_file())
            .load()
            .context("failed to load upstream credentials")?;

    // Build the trusted registry and shared upstream client used by the data plane.
    let registry =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    // Move only the selected target's upstream pool into the immutable credential snapshot.
    let required_pool_id = registry
        .upstream_target(&arguments.upstream_target_id)
        .map(|target| target.credential_pool_id());
    let credentials = upstream_configuration
        .into_builder_for(&registry, required_pool_id)
        .context("failed to bind the selected upstream credential pool")?
        .build();
    credentials
        .validate_registry(&registry)
        .context("selected credential pool violates registry state-affinity constraints")?;
    // Run the administrator-selected probes and print only the redacted JSON report.
    let report = probe_upstream_target(
        &registry,
        &arguments.upstream_target_id,
        &upstream,
        &credentials,
        arguments.selection,
    )
    .await
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
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--target" => {
                    let value = arguments
                        .next()
                        .context("--target requires a configured upstream target id")?;
                    upstream_target_id = Some(value);
                }
                "--list-models" => selection.list_models = true,
                "--chat" => selection.chat = true,
                "--responses" => selection.responses = true,
                "--function-calling" => selection.function_calling = true,
                "--all" => selection = ProbeOptions::all(),
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
        })
    }
}

/// Prints local probe usage without credentials or runtime state.
fn print_usage() {
    println!(
        "Usage: cargo run --bin openbridge-probe -- --target <id> [--list-models] [--chat] [--responses] [--function-calling] [--all]\n\
         \n\
         No probe selector runs --all. The command only prints a report; it never modifies the code registry."
    );
}
