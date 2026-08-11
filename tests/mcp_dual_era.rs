//! Verifies the authenticated MCP server through real rmcp client handshakes.
//!
//! Covers the dual-era migration: legacy `initialize` clients (2025-11-25 and
//! earlier) and stateless `server/discover` clients (2026-07-28) must both reach
//! the same `/mcp` endpoint behind the shared downstream Bearer boundary.

mod support;

use std::sync::Arc;

use openbridge::{
    ingress::{GatewayState, build_router},
    transport::upstream::UpstreamClient,
};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient,
    model::{CallToolRequestParams, ClientInfo, ListToolsResult, ProtocolVersion},
    service::RunningService,
    transport::streamable_http_client::{
        StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
    },
};

const DOWNSTREAM_TOKEN: &str = "downstream-test-token-00000000000";

/// Builds the production Router with only synthetic in-memory credentials.
fn test_app() -> axum::Router {
    let registry = support::registry("mcp-test", "code-primary", "test-model");
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .unwrap();
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_TOKEN, &registry, "upstream-test-token");
    build_router(GatewayState::new(
        Arc::new(registry),
        Arc::new(upstream),
        users,
        credentials,
    ))
}

/// Spawns the production Router on an ephemeral loopback port and returns its base URL.
async fn spawn_test_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, test_app()).await.unwrap();
    });
    format!("http://{addr}/mcp")
}

/// Builds one rmcp Streamable HTTP client transport with the shared downstream token.
fn client_transport(url: &str) -> StreamableHttpClientTransport<reqwest::Client> {
    StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(url).auth_header(DOWNSTREAM_TOKEN),
    )
}

/// Builds an rmcp client connected through the given lifecycle mode.
async fn connect(
    url: &str,
    lifecycle: ClientLifecycleMode,
) -> RunningService<RoleClient, ClientInfo> {
    ClientInfo::default()
        .serve_with_lifecycle(client_transport(url), lifecycle)
        .await
        .expect("rmcp client should connect through the /mcp endpoint")
}

#[tokio::test]
async fn legacy_initialize_client_handshakes_and_lists_hello_tool() {
    let url = spawn_test_server().await;

    // A legacy client uses the initialize/initialized handshake and negotiates a
    // pre-2026-07-28 protocol revision with the server.
    let client = connect(&url, ClientLifecycleMode::Initialize).await;

    // The same static hello catalog is discoverable through the legacy session.
    let tools = client.list_tools(None).await.expect("list tools");
    assert!(
        tools.tools.iter().any(|tool| tool.name == "hello"),
        "legacy client must see the hello tool"
    );

    // Call the hello tool through the legacy session.
    let result = client
        .call_tool(
            CallToolRequestParams::new("hello").with_arguments(
                serde_json::json!({ "name": "Ada" })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call hello");
    assert!(!result.is_error.unwrap_or(false), "hello must succeed");
    let text = result
        .content
        .iter()
        .find_map(|block| block.as_text())
        .expect("text block");
    assert_eq!(text.text, "Hi, Ada!");

    client.cancel().await.expect("cancel client");
}

#[tokio::test]
async fn stateless_discover_client_handshakes_and_lists_hello_tool() {
    let url = spawn_test_server().await;

    // A 2026-07-28 client probes with server/discover and sends self-contained
    // per-request protocol metadata instead of holding a session.
    let client = connect(
        &url,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
    )
    .await;

    // The same static hello catalog is discoverable through the stateless path.
    let tools = client.list_tools(None).await.expect("list tools");
    assert!(
        tools.tools.iter().any(|tool| tool.name == "hello"),
        "stateless client must see the hello tool"
    );

    // Call the hello tool through the stateless path.
    let result = client
        .call_tool(
            CallToolRequestParams::new("hello").with_arguments(
                serde_json::json!({ "name": "Ada" })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call hello");
    assert!(!result.is_error.unwrap_or(false), "hello must succeed");
    let text = result
        .content
        .iter()
        .find_map(|block| block.as_text())
        .expect("text block");
    assert_eq!(text.text, "Hi, Ada!");

    client.cancel().await.expect("cancel client");
}

#[tokio::test]
async fn auto_client_prefers_stateless_and_falls_back_to_legacy_handshake() {
    let url = spawn_test_server().await;

    // Auto lifecycle probes server/discover first and falls back to the legacy
    // initialize handshake only when the peer proves it is legacy. Either path
    // must produce a usable client against the same endpoint.
    let client = connect(
        &url,
        ClientLifecycleMode::Auto {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            legacy_version: Some(ProtocolVersion::V_2025_11_25),
        },
    )
    .await;

    let tools: ListToolsResult = client.list_tools(None).await.expect("list tools");
    assert!(
        tools.tools.iter().any(|tool| tool.name == "hello"),
        "auto client must see the hello tool"
    );

    client.cancel().await.expect("cancel client");
}
