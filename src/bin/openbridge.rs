//! Minimal loopback bootstrap; startup never prints credential values.
use clap::Parser;
use openbridge::gateway::bootstrap::Bootstrap;
#[derive(Parser)]
struct Options {
    #[arg(long)]
    credentials_dir: std::path::PathBuf,
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let options = match Options::try_parse() {
        Ok(options) => options,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return std::process::ExitCode::SUCCESS;
            }
            eprintln!("Invalid startup arguments; use --help. Values are not echoed.");
            return std::process::ExitCode::FAILURE;
        }
    };
    let config = options
        .config
        .unwrap_or_else(|| options.credentials_dir.join("gateway.json"));
    let bootstrap = match Bootstrap::from_files(&config, &options.credentials_dir) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("OpenBridge startup failed: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(bootstrap.listen).await {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Loopback listener failed: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Ok(address) = listener.local_addr() {
        println!("OpenBridge listening on http://{address}");
    }
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let owner = bootstrap.gateway.clone();
    let result = bootstrap.gateway.serve(listener, shutdown).await;
    owner.flush_probe_diagnostics().await;
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("OpenBridge server stopped with an I/O error");
            std::process::ExitCode::FAILURE
        }
    }
}
