//! Streamable HTTP transport for the MCP server.
//!
//! The official `rmcp` `StreamableHttpService` owns JSON-RPC validation, protocol version
//! negotiation (stateless `2026-07-28` plus legacy `initialize` sessions), and response
//! serialization. This module only wires the local `HelloServer` into a Tower service and keeps
//! the fail-closed browser Origin rejection used by the ingress router.

use super::tools::HelloServer;

use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use http::{StatusCode, header::ORIGIN};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde_json::json;

const INVALID_REQUEST: i64 = -32600;

/// Rejects every browser Origin before downstream authentication or JSON-RPC dispatch.
pub(crate) async fn reject_origin(request: Request, next: Next) -> Response {
    // Fail closed because this loopback MCP service has no configured browser Origin allowlist.
    if request.headers().contains_key(ORIGIN) {
        return error_response(
            StatusCode::FORBIDDEN,
            INVALID_REQUEST,
            "Origin header is not allowed",
        );
    }

    // Forward originless local-client requests into the existing Bearer boundary.
    next.run(request).await
}

/// Builds the rmcp Streamable HTTP Tower service mounted at `/mcp`.
///
/// The server negotiates both the stateless `2026-07-28` protocol and legacy `initialize`
/// sessions, so older clients keep working while newer ones opt in.
pub(crate) fn service() -> StreamableHttpService<HelloServer, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default().with_json_response(true);
    StreamableHttpService::new(
        || Ok(HelloServer),
        LocalSessionManager::default().into(),
        config,
    )
}

/// Builds the minimal JSON-RPC error response used by the Origin rejection middleware.
fn error_response(status: StatusCode, code: i64, message: &str) -> Response {
    let document = json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": { "code": code, "message": message }
    });
    (status, axum::Json(document)).into_response()
}
