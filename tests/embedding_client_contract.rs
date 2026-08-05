//! Runs an independent standard-library Python client against the real Embeddings HTTP ingress.
//!
//! The loopback uses the checked-in registry and Provider adapter with a deterministic in-memory
//! upstream transport. It does not read private configuration, install dependencies, or call a
//! real Provider.

mod support;

use std::{
    env,
    ffi::OsString,
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::body::Body;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    providers::build_compiled_registry,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

const DOWNSTREAM_KEY: &str = "downstream-loopback-token-0000000000";

struct EmbeddingFixtureTransport {
    attempts: AtomicUsize,
}

impl UpstreamTransport for EmbeddingFixtureTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Verify the checked-in candidate reaches the dedicated trusted target and adapter path.
        assert_eq!(target.id(), "openai-text-embedding-3-small");
        assert_eq!(request.relative_uri().path(), "/v1/embeddings");
        let body: Value = serde_json::from_slice(request.body()).expect("request must be JSON");
        assert_eq!(body["model"], "text-embedding-3-small");
        assert_eq!(body["input"], json!(["alpha", "beta"]));
        assert_eq!(body["encoding_format"], "float");
        assert_eq!(body["user"], "synthetic-loopback-user");
        self.attempts.fetch_add(1, Ordering::Relaxed);

        // Return two ordered 1,536-dimension vectors under the bounded success contract.
        Box::pin(async move {
            let body = serde_json::to_vec(&json!({
                "object": "list",
                "data": [
                    {
                        "object": "embedding",
                        "embedding": vec![0.25_f64; 1_536],
                        "index": 0
                    },
                    {
                        "object": "embedding",
                        "embedding": vec![-0.5_f64; 1_536],
                        "index": 1
                    }
                ],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 2, "total_tokens": 2}
            }))
                .expect("fixture response must encode");
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from(Bytes::from(body)),
            ))
        })
    }
}

#[tokio::test]
#[ignore = "runs an independent Python client against a real loopback listener"]
async fn python_client_discovers_and_calls_the_checked_in_embedding_model() {
    // Build the checked-in registry and inject only synthetic test credentials.
    let registry = build_compiled_registry(support::bootstrap(support::BOOTSTRAP))
        .expect("the checked-in registry must compile");
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_KEY, &registry, "synthetic-upstream-token");
    let transport = Arc::new(EmbeddingFixtureTransport {
        attempts: AtomicUsize::new(0),
    });
    let app = build_router(GatewayState::new(
        Arc::new(registry),
        transport.clone(),
        users,
        credentials,
    ));

    // Serve the production Router on an ephemeral loopback socket.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the loopback listener must bind");
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("the loopback server must remain valid");
    });

    // Run the dependency-free client as a separate process and capture only its safe summary.
    let output = run_python_client(base_url).await;
    server.abort();
    assert_process_success(&output);
    let summary: Value = serde_json::from_slice(&output.stdout).expect("summary must be JSON");
    assert_eq!(
        summary,
        json!({
            "default_dimensions": 1536,
            "discovered_model": "text-embedding-3-small",
            "encoding": "float",
            "vectors": 2
        })
    );
    assert_eq!(transport.attempts.load(Ordering::Relaxed), 1);
}

/// Runs the independent Python client without blocking a Tokio worker thread.
async fn run_python_client(base_url: String) -> Output {
    // Resolve the caller override or the ordinary cross-platform Python command.
    let python = env::var_os("OPENBRIDGE_PYTHON").unwrap_or_else(|| OsString::from("python"));
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/clients/embedding_python_loopback.py");

    // Keep the blocking child process outside Tokio's async worker pool.
    tokio::task::spawn_blocking(move || {
        Command::new(python)
            .arg(script)
            .arg(base_url)
            .arg(DOWNSTREAM_KEY)
            .output()
            .expect("the Python client process must start")
    })
        .await
        .expect("the Python client task must complete")
}

/// Panics with captured child output when the independent client exits unsuccessfully.
fn assert_process_success(output: &Output) {
    if !output.status.success() {
        panic!(
            "Python loopback client failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
