//! 显式执行上游模型发现与协议能力 probe 的本地 CLI。
//!
//! 该工具只输出 JSON report，不启动下游 HTTP 服务，也不修改代码注册表。

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
/// 解析 probe 参数、绑定受信 target 并输出脱敏 capability report。
async fn main() -> Result<()> {
    // 解析 CLI 选择。
    let arguments = ProbeArguments::parse(env::args().skip(1))?;

    // 读取 bootstrap 和私有上游 credential 配置。
    let bootstrap = BootstrapConfigPath::from_environment()
        .load()
        .context("failed to load OpenBridge bootstrap configuration")?;
    let upstream_configuration =
        UpstreamCredentialConfigPath::new(bootstrap.upstream_credentials_file())
            .load()
            .context("failed to load upstream credentials")?;

    // 构造与数据面相同的受信 registry 和共享 upstream client。
    let registry =
        build_compiled_registry(bootstrap).context("failed to build OpenBridge code registry")?;
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .context("failed to initialize upstream HTTP client")?;
    // 只把管理员选中 target 的上游 pool 移入不可变 credential 快照。
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
    // 执行管理员显式选择的 probe 并只输出脱敏 JSON 报告。
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
    /// 将命令行 selector 解析为一个 target 和固定 probe 选择集合。
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        // 逐项解析 target 和 probe 选择，不接受未声明的 CLI 参数。
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
        // 校验 target 必填，并在未指定选择时默认执行全部 probe。
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

/// 打印不含凭证和运行时状态的本地 probe 用法说明。
fn print_usage() {
    println!(
        "Usage: cargo run --bin openbridge-probe -- --target <id> [--list-models] [--chat] [--responses] [--function-calling] [--all]\n\
         \n\
         No probe selector runs --all. The command only prints a report; it never modifies the code registry."
    );
}
