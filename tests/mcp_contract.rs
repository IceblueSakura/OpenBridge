//! Verifies the authenticated, originless MCP server and its local test tool catalog.
//!
//! The transport is the official `rmcp` `StreamableHttpService`: it owns JSON-RPC validation,
//! protocol version negotiation (stateless `2026-07-28` plus legacy `initialize` sessions), and
//! response serialization. These tests assert the HTTP boundary and the local hello tool through
//! that service, not rmcp internals.

mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{
        HeaderValue, Request, Response, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, ORIGIN},
    },
};
use openbridge::{
    ingress::{GatewayState, build_router},
    transport::upstream::UpstreamClient,
};
use serde_json::{Value, json};
use tower::ServiceExt;

const DOWNSTREAM_TOKEN: &str = "downstream-test-token-00000000000";
const MCP_PROTOCOL_VERSION: &str = "2026-07-28";

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

/// Builds one stateless MCP request with matching body and routing headers.
///
/// The rmcp server requires `_meta` to carry the protocol version, client info, and client
/// capabilities, and requires the `MCP-Protocol-Version` HTTP header to match `_meta`.
fn mcp_request(method: &str, id: Value, extra_params: Value) -> Request<Body> {
    let mut params = extra_params.as_object().cloned().unwrap();
    params.insert(
        "_meta".to_owned(),
        json!({
            "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientInfo": {
                "name": "openbridge-contract-test",
                "version": "1.0.0"
            },
            "io.modelcontextprotocol/clientCapabilities": {}
        }),
    );
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });

    Request::post("/mcp")
        .header(AUTHORIZATION, format!("Bearer {DOWNSTREAM_TOKEN}"))
        .header(CONTENT_TYPE, "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("host", "127.0.0.1:8080")
        .header("mcp-protocol-version", MCP_PROTOCOL_VERSION)
        .header("mcp-method", method)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Builds one tool call with the standard MCP request and `Mcp-Name` routing metadata.
fn mcp_tool_call(id: Value, tool_name: &str, arguments: Value) -> Request<Body> {
    let mut request = mcp_request(
        "tools/call",
        id,
        json!({ "name": tool_name, "arguments": arguments }),
    );
    request.headers_mut().insert(
        "mcp-name",
        HeaderValue::from_str(tool_name).expect("test tool names must be valid header values"),
    );
    request
}

/// Parses one bounded JSON response body for protocol assertions.
async fn response_json(response: Response<Body>) -> Value {
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn mcp_server_discover_negotiates_current_protocol() {
    let app = test_app();

    // Stateless discovery reports the server identity and tools capability.
    let response = app
        .clone()
        .oneshot(mcp_request(
            "server/discover",
            json!("discover-1"),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document = response_json(response).await;
    assert_eq!(document["jsonrpc"], "2.0");
    assert_eq!(document["id"], "discover-1");
    assert_eq!(document["result"]["resultType"], "complete");
    assert_eq!(
        document["result"]["supportedVersions"],
        json!([
            "2024-11-05",
            "2025-03-26",
            "2025-06-18",
            "2025-11-25",
            "2026-07-28"
        ])
    );
    assert_eq!(
        document["result"]["_meta"]["io.modelcontextprotocol/serverInfo"],
        json!({ "name": "openbridge", "version": env!("CARGO_PKG_VERSION") })
    );
    assert_eq!(
        document["result"]["capabilities"],
        json!({ "tools": { "listChanged": true } })
    );

    // Listing the deterministic hello tool catalog is stateless and complete.
    let response = app
        .oneshot(mcp_request("tools/list", json!(2), json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document = response_json(response).await;
    assert_eq!(document["id"], 2);
    let tools = document["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "hello");
    assert_eq!(
        tools[0]["description"],
        "Returns a greeting for the provided name."
    );
    assert_eq!(tools[0]["inputSchema"]["type"], "object");
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["name"]));
    assert_eq!(
        tools[0]["inputSchema"]["properties"]["name"]["type"],
        "string"
    );
}

#[tokio::test]
async fn mcp_hello_tool_greets_name_and_reports_invalid_arguments() {
    let app = test_app();

    // Execute the local test tool and preserve its exact text result.
    let response = app
        .clone()
        .oneshot(mcp_tool_call(
            json!("hello-1"),
            "hello",
            json!({ "name": "Ada" }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document = response_json(response).await;
    assert_eq!(document["id"], "hello-1");
    assert_eq!(document["result"]["content"][0]["type"], "text");
    assert_eq!(document["result"]["content"][0]["text"], "Hi, Ada!");
    assert!(!document["result"]["isError"].as_bool().unwrap_or(false));

    // Report wrong types and extra properties as invalid tool-call parameters.
    for (id, arguments) in [
        ("hello-2", json!({ "name": 42 })),
        ("hello-3", json!({ "name": "Ada", "extra": true })),
    ] {
        let response = app
            .clone()
            .oneshot(mcp_tool_call(json!(id), "hello", arguments))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let document = response_json(response).await;
        assert_eq!(document["id"], id);
        assert!(document["result"]["isError"].as_bool().unwrap_or(false));
        let text = document["result"]["content"][0]["text"]
            .as_str()
            .expect("error text");
        assert!(
            text.contains("Invalid params") || text.contains("failed to deserialize"),
            "unexpected error text: {text}"
        );
    }
}

#[tokio::test]
async fn mcp_server_fails_closed_before_tool_execution() {
    let app = test_app();

    // Keep MCP discovery behind the same static downstream Bearer boundary.
    let mut request = mcp_request("server/discover", json!(1), json!({}));
    request.headers_mut().remove(AUTHORIZATION);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()["www-authenticate"], "Bearer");

    // Reject every browser Origin before authentication or JSON-RPC dispatch.
    let mut request = mcp_request("server/discover", json!(2), json!({}));
    request.headers_mut().remove(AUTHORIZATION);
    request
        .headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("https://example.invalid"));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let document = response_json(response).await;
    assert_eq!(document["error"]["code"], -32600);

    // A stateless request missing the MCP protocol version header is rejected.
    let mut request = mcp_request("tools/list", json!(3), json!({}));
    request.headers_mut().remove("mcp-protocol-version");
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 3);
    assert_eq!(document["error"]["code"], -32020);

    // An unknown tool is rejected as an invalid tool-call parameter.
    let response = app
        .clone()
        .oneshot(mcp_tool_call(json!(4), "future_tool", json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 4);
    assert_eq!(document["error"]["code"], -32602);
    assert_eq!(document["error"]["message"], "tool not found");

    // Keep the GET stream and DELETE session lifecycle outside the endpoint boundary.
    // rmcp requires a session ID for GET streams, so an unauthenticated GET still
    // fails at authentication before reaching the MCP service.
    let mut request = Request::get("/mcp").body(Body::empty()).unwrap();
    request.headers_mut().remove(AUTHORIZATION);
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
