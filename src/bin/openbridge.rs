//! Minimal loopback bootstrap; startup never prints credential values.
use openbridge::gateway::bootstrap::Bootstrap;
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let bootstrap = match Bootstrap::from_env() {
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
    match bootstrap.gateway.serve(listener, shutdown).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("OpenBridge server stopped with an I/O error");
            std::process::ExitCode::FAILURE
        }
    }
}
