//! 显式执行上游模型发现与协议能力 probe 的本地 CLI。
//!
//! 该工具只输出 JSON report，不启动下游 HTTP 服务，也不修改代码注册表。

use std::env;

use anyhow::{Context, Result};
use openbridge::{
    config::{BootstrapPath, load_optional_dotenv},
    probe::{ProbeSelection, probe_upstream_target},
    provider::CredentialSource,
    providers::build_compiled_registry,
    transport::upstream::UpstreamClient,
};

#[tokio::main]
async fn main() -> Result<()> {
    load_optional_dotenv().context("failed to load optional .env file")?;
    let arguments = ProbeArguments::parse(env::args().skip(1))?;
    let bootstrap = BootstrapPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let snapshot =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let upstream = UpstreamClient::new(
        snapshot.upstream_policy().connect_timeout(),
        snapshot.upstream_policy().pool_idle_timeout(),
        snapshot.upstream_policy().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    let report = probe_upstream_target(
        &snapshot,
        &arguments.upstream_target_id,
        &upstream,
        &CredentialSource::environment(),
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
    selection: ProbeSelection,
}

impl ProbeArguments {
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut upstream_target_id = None;
        let mut selection = ProbeSelection::default();
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
                "--all" => selection = ProbeSelection::all(),
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("unknown argument '{argument}'; run with --help"),
            }
        }
        let upstream_target_id = upstream_target_id.context("--target is required")?;
        if selection.is_empty() {
            selection = ProbeSelection::all();
        }
        Ok(Self {
            upstream_target_id,
            selection,
        })
    }
}

fn print_usage() {
    println!(
        "Usage: cargo run --bin openbridge-probe -- --target <id> [--list-models] [--chat] [--responses] [--function-calling] [--all]\n\
         \n\
         No probe selector runs --all. The command only prints a report; it never modifies the code registry."
    );
}
