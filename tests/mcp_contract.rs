//! Verifies the authenticated, originless MCP server and its local test tool catalog.

mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{
        HeaderValue, Request, Response, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN},
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
    // Compile the deterministic registry and its shared upstream client without making egress possible.
    let registry = support::registry("mcp-test", "code-primary", "test-model");
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .unwrap();

    // Bind one synthetic downstream user and assemble the production ingress Router.
    let (users, credentials) =
        support::users_and_credentials(DOWNSTREAM_TOKEN, &registry, "upstream-test-token");
    build_router(GatewayState::new(
        Arc::new(registry),
        Arc::new(upstream),
        users,
        credentials,
    ))
}

/// Builds one current MCP Streamable HTTP request with matching body and routing headers.
fn mcp_request(method: &str, id: Value, extra_params: Value) -> Request<Body> {
    mcp_request_for_version(method, id, extra_params, MCP_PROTOCOL_VERSION)
}

/// Builds one MCP request for an explicitly selected protocol version.
fn mcp_request_for_version(
    method: &str,
    id: Value,
    extra_params: Value,
    protocol_version: &str,
) -> Request<Body> {
    // Merge the per-request MCP metadata with method-specific parameters.
    let mut params = extra_params.as_object().cloned().unwrap();
    params.insert(
        "_meta".to_owned(),
        json!({
            "io.modelcontextprotocol/protocolVersion": protocol_version,
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

    // Mirror the method and protocol version into the required HTTP headers.
    Request::post("/mcp")
        .header(AUTHORIZATION, format!("Bearer {DOWNSTREAM_TOKEN}"))
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", protocol_version)
        .header("mcp-method", method)
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Builds one tool call with matching body and `Mcp-Name` routing metadata.
fn mcp_tool_call(id: Value, tool_name: &str, arguments: Value) -> Request<Body> {
    // Build the standard MCP request before adding the tool-specific mirror header.
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
    // Bound the in-memory body read independently of the production request limit.
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn mcp_server_discovers_current_protocol_and_lists_hello_tool() {
    let app = test_app();

    // Discover the current stateless server identity and its local tool capability.
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
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    assert!(!response.headers().contains_key("mcp-session-id"));
    let document = response_json(response).await;
    assert_eq!(document["jsonrpc"], "2.0");
    assert_eq!(document["id"], "discover-1");
    assert_eq!(document["result"]["resultType"], "complete");
    assert_eq!(
        document["result"]["supportedVersions"],
        json!([MCP_PROTOCOL_VERSION])
    );
    assert_eq!(document["result"]["capabilities"], json!({ "tools": {} }));
    assert_eq!(
        document["result"]["_meta"]["io.modelcontextprotocol/serverInfo"],
        json!({ "name": "openbridge", "version": env!("CARGO_PKG_VERSION") })
    );
    assert_eq!(document["result"]["ttlMs"], 0);
    assert_eq!(document["result"]["cacheScope"], "private");

    // List the deterministic hello tool catalog without minting transport session state.
    let response = app
        .oneshot(mcp_request("tools/list", json!(2), json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("mcp-session-id"));
    let document = response_json(response).await;
    assert_eq!(document["id"], 2);
    assert_eq!(document["result"]["resultType"], "complete");
    assert_eq!(
        document["result"]["tools"],
        json!([{
            "name": "hello",
            "description": "Returns a greeting for the provided name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name to greet."
                    }
                },
                "required": ["name"],
                "additionalProperties": false
            }
        }])
    );
    assert_eq!(document["result"]["ttlMs"], 0);
    assert_eq!(document["result"]["cacheScope"], "private");
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
    assert_eq!(document["result"]["resultType"], "complete");
    assert_eq!(
        document["result"]["content"],
        json!([{ "type": "text", "text": "Hi, Ada!" }])
    );
    assert_eq!(document["result"]["isError"], false);
    assert_eq!(
        document["result"]["_meta"]["io.modelcontextprotocol/serverInfo"],
        json!({ "name": "openbridge", "version": env!("CARGO_PKG_VERSION") })
    );

    // Report wrong types and extra properties as actionable tool execution errors.
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
        assert_eq!(document["result"]["resultType"], "complete");
        assert_eq!(document["result"]["isError"], true);
        assert_eq!(
            document["result"]["content"],
            json!([{
                "type": "text",
                "text": "Invalid arguments: `name` must be a string and no other arguments are allowed."
            }])
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

    // Reject a routing-header mismatch with the MCP-specific transport error.
    let mut request = mcp_request("tools/list", json!(3), json!({}));
    request
        .headers_mut()
        .insert("mcp-method", HeaderValue::from_static("server/discover"));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 3);
    assert_eq!(document["error"]["code"], -32020);

    // Reject an internally consistent but unsupported legacy protocol revision.
    let request = mcp_request_for_version("server/discover", json!(4), json!({}), "2025-11-25");
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 4);
    assert_eq!(document["error"]["code"], -32022);
    assert_eq!(
        document["error"]["data"],
        json!({
            "supported": [MCP_PROTOCOL_VERSION],
            "requested": "2025-11-25"
        })
    );

    // Reject a mismatched tool routing header before dispatching the supported tool.
    let mut request = mcp_tool_call(json!(5), "hello", json!({ "name": "Ada" }));
    request
        .headers_mut()
        .insert("mcp-name", HeaderValue::from_static("future_tool"));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 5);
    assert_eq!(document["error"]["code"], -32020);

    // Reject an unknown tool as an invalid tool-call parameter.
    let response = app
        .clone()
        .oneshot(mcp_tool_call(json!(6), "future_tool", json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let document = response_json(response).await;
    assert_eq!(document["id"], 6);
    assert_eq!(document["error"]["code"], -32602);

    // Keep the removed GET stream and DELETE session lifecycle outside the endpoint.
    for request in [Request::get("/mcp"), Request::delete("/mcp")] {
        let response = app
            .clone()
            .oneshot(
                request
                    .header(AUTHORIZATION, format!("Bearer {DOWNSTREAM_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
