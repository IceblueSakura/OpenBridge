//! Minimal MCP Streamable HTTP ingress for the authenticated local tool catalog.
//!
//! This module implements only protocol revision `2026-07-28`, discovery, tool listing, and the
//! side-effect-free `hello` test tool. It owns no Provider routing and deliberately rejects browser
//! Origins, legacy sessions, and every unregistered tool or protocol method.

mod tools;

use axum::{
    Json,
    body::Bytes,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use http::{
    HeaderMap, StatusCode,
    header::{ACCEPT, CONTENT_TYPE, ORIGIN},
};
use serde_json::{Map, Value, json};

const PROTOCOL_VERSION: &str = "2026-07-28";
const PROTOCOL_VERSION_META: &str = "io.modelcontextprotocol/protocolVersion";
const CLIENT_INFO_META: &str = "io.modelcontextprotocol/clientInfo";
const CLIENT_CAPABILITIES_META: &str = "io.modelcontextprotocol/clientCapabilities";
const SERVER_INFO_META: &str = "io.modelcontextprotocol/serverInfo";

const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const PARSE_ERROR: i64 = -32700;
const HEADER_MISMATCH: i64 = -32020;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// Rejects every browser Origin before downstream authentication or JSON-RPC dispatch.
pub(super) async fn reject_origin(request: Request, next: Next) -> Response {
    // Fail closed because this loopback MCP service has no configured browser Origin allowlist.
    if request.headers().contains_key(ORIGIN) {
        return error_response(
            StatusCode::FORBIDDEN,
            None,
            INVALID_REQUEST,
            "Origin header is not allowed",
            None,
        );
    }

    // Forward originless local-client requests into the existing Bearer boundary.
    next.run(request).await
}

/// Handles one stateless MCP request and returns a single JSON response.
pub(super) async fn endpoint(headers: HeaderMap, body: Bytes) -> Response {
    // Validate the JSON-only request and the response formats required by Streamable HTTP clients.
    if !has_json_content_type(&headers) {
        return error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            None,
            INVALID_REQUEST,
            "Content-Type must be application/json",
            None,
        );
    }
    if !accepts_media_type(&headers, "application/json")
        || !accepts_media_type(&headers, "text/event-stream")
    {
        return error_response(
            StatusCode::NOT_ACCEPTABLE,
            None,
            INVALID_REQUEST,
            "Accept must include application/json and text/event-stream",
            None,
        );
    }

    // Parse and validate the bounded JSON-RPC request envelope.
    let document = match serde_json::from_slice::<Value>(&body) {
        Ok(document) => document,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                None,
                PARSE_ERROR,
                "Parse error",
                None,
            );
        }
    };
    let request = match ParsedRequest::parse(document) {
        Ok(request) => request,
        Err(id) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                id,
                INVALID_REQUEST,
                "Invalid JSON-RPC request",
                None,
            );
        }
    };

    // Verify the mirrored transport headers before dispatching any protocol method.
    if required_header(&headers, "mcp-method") != Some(request.method.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            HEADER_MISMATCH,
            "Mcp-Method header does not match the request method",
            None,
        );
    }
    let Some(protocol_header) = required_header(&headers, "mcp-protocol-version") else {
        return error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            HEADER_MISMATCH,
            "MCP-Protocol-Version header is missing or malformed",
            None,
        );
    };

    // Validate the required per-request metadata and negotiate the sole supported revision.
    let metadata = match request_metadata(&request.params) {
        Some(metadata) => metadata,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                Some(request.id),
                INVALID_PARAMS,
                "Request metadata is invalid",
                None,
            );
        }
    };
    let requested_version = metadata[PROTOCOL_VERSION_META].as_str().unwrap();
    if protocol_header != requested_version {
        return error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            HEADER_MISMATCH,
            "MCP-Protocol-Version header does not match request metadata",
            None,
        );
    }
    if requested_version != PROTOCOL_VERSION {
        return error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
            Some(json!({
                "supported": [PROTOCOL_VERSION],
                "requested": requested_version
            })),
        );
    }

    // Validate the tool-specific routing mirror before selecting a local implementation.
    if let Some(response) = tool_name_header_error(&headers, &request) {
        return response;
    }

    // Dispatch only discovery, the deterministic tool catalog, and registered local tools.
    match request.method.as_str() {
        "server/discover" if discover_params_are_valid(&request.params) => {
            result_response(request.id, discover_result())
        }
        "tools/list" if list_params_are_valid(&request.params) => {
            result_response(request.id, tools_list_result())
        }
        "tools/call" => call_tool(request),
        "server/discover" | "tools/list" => error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            INVALID_PARAMS,
            "Method parameters are invalid",
            None,
        ),
        _ => error_response(
            StatusCode::NOT_FOUND,
            Some(request.id),
            METHOD_NOT_FOUND,
            "Method not found",
            None,
        ),
    }
}

/// Returns a transport error when `Mcp-Name` does not mirror a tool call body.
fn tool_name_header_error(headers: &HeaderMap, request: &ParsedRequest) -> Option<Response> {
    // Leave name-independent methods to their existing header and parameter validation.
    if request.method != "tools/call" {
        return None;
    }

    // Let protocol parameter validation report a missing or non-string body name.
    let tool_name = request.params.get("name")?.as_str()?;

    // Reject missing, malformed, duplicate, or mismatched tool routing metadata.
    if required_header(headers, "mcp-name") != Some(tool_name) {
        return Some(error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id.clone()),
            HEADER_MISMATCH,
            "Mcp-Name header does not match the requested tool",
            None,
        ));
    }
    None
}

/// Dispatches one schema-valid tool call to the static local catalog.
fn call_tool(request: ParsedRequest) -> Response {
    // Parse the closed call shape before consulting the tool catalog.
    let Some((tool_name, arguments)) = tool_call_params(&request.params) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            INVALID_PARAMS,
            "Tool call parameters are invalid",
            None,
        );
    };

    // Execute only a registered local tool and decorate its result with server identity.
    match tools::call(tool_name, arguments) {
        Ok(mut result) => {
            result["_meta"] = server_metadata();
            result_response(request.id, result)
        }
        Err(tools::ToolDispatchError::UnknownTool) => error_response(
            StatusCode::BAD_REQUEST,
            Some(request.id),
            INVALID_PARAMS,
            "Unknown tool",
            None,
        ),
    }
}

struct ParsedRequest {
    id: Value,
    method: String,
    params: Map<String, Value>,
}

impl ParsedRequest {
    /// Parses the closed JSON-RPC envelope while preserving the caller's request ID.
    fn parse(document: Value) -> Result<Self, Option<Value>> {
        // Capture a valid request ID first so later envelope failures can correlate their response.
        let Some(object) = document.as_object() else {
            return Err(None);
        };
        let id = object
            .get("id")
            .filter(|id| id.is_string() || id.is_number())
            .cloned();

        // Require the MCP request-shaped JSON-RPC fields and an object parameter payload.
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(id);
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Err(id);
        };
        let Some(params) = object.get("params").and_then(Value::as_object) else {
            return Err(id);
        };
        let Some(id) = id else {
            return Err(None);
        };

        // Own the validated envelope without retaining unrelated top-level fields.
        Ok(Self {
            id,
            method: method.to_owned(),
            params: params.clone(),
        })
    }
}

/// Returns valid MCP request metadata from the parameter object.
fn request_metadata(params: &Map<String, Value>) -> Option<&Map<String, Value>> {
    // Require the current protocol version and one object-valued capability declaration.
    let metadata = params.get("_meta")?.as_object()?;
    metadata.get(PROTOCOL_VERSION_META)?.as_str()?;
    metadata.get(CLIENT_CAPABILITIES_META)?.as_object()?;

    // Validate optional self-reported client identity without using it for authorization decisions.
    if let Some(client_info) = metadata.get(CLIENT_INFO_META) {
        let client_info = client_info.as_object()?;
        client_info.get("name")?.as_str()?;
        client_info.get("version")?.as_str()?;
    }
    Some(metadata)
}

/// Returns whether discovery contains no method-specific parameters.
fn discover_params_are_valid(params: &Map<String, Value>) -> bool {
    params.keys().all(|key| key == "_meta")
}

/// Returns whether tool listing contains only metadata and an optional string cursor.
fn list_params_are_valid(params: &Map<String, Value>) -> bool {
    // Reject unknown list parameters before constructing the static catalog.
    if !params.keys().all(|key| key == "_meta" || key == "cursor") {
        return false;
    }

    // Accept omission or one opaque string cursor even though the static list has no next page.
    params.get("cursor").is_none_or(Value::is_string)
}

/// Returns the tool name and optional arguments from a closed call parameter object.
fn tool_call_params(params: &Map<String, Value>) -> Option<(&str, Option<&Map<String, Value>>)> {
    // Reject fields that do not belong to the supported one-round tool call shape.
    if !params
        .keys()
        .all(|key| key == "_meta" || key == "name" || key == "arguments")
    {
        return None;
    }

    // Require a string tool name and, when present, an object argument payload.
    let tool_name = params.get("name")?.as_str()?;
    let arguments = match params.get("arguments") {
        Some(arguments) => Some(arguments.as_object()?),
        None => None,
    };
    Some((tool_name, arguments))
}

/// Returns one required, unique, visible HTTP header value.
fn required_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    // Reject missing, duplicate, non-text, and empty routing metadata.
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if value.is_empty() || values.next().is_some() {
        return None;
    }
    Some(value)
}

/// Returns whether exactly one JSON Content-Type is present.
fn has_json_content_type(headers: &HeaderMap) -> bool {
    // Reject missing or duplicate values before normalizing an optional charset parameter.
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next().and_then(|value| value.to_str().ok()) else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    value
        .split(';')
        .next()
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

/// Returns whether any Accept field explicitly lists the required media type.
fn accepts_media_type(headers: &HeaderMap, required: &str) -> bool {
    // Parse every comma-delimited range while ignoring optional quality parameters.
    headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|range| range.split(';').next())
        .any(|media_type| media_type.trim().eq_ignore_ascii_case(required))
}

/// Builds the current server identity metadata included on every successful response.
fn server_metadata() -> Value {
    json!({
        SERVER_INFO_META: {
            "name": "openbridge",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// Builds the cache-disabled discovery result for the local test server.
fn discover_result() -> Value {
    json!({
        "resultType": "complete",
        "supportedVersions": [PROTOCOL_VERSION],
        "capabilities": { "tools": {} },
        "_meta": server_metadata(),
        "instructions": "This OpenBridge MCP endpoint provides a local hello test tool.",
        "ttlMs": 0,
        "cacheScope": "private"
    })
}

/// Builds the deterministic, cache-disabled local tool list.
fn tools_list_result() -> Value {
    json!({
        "resultType": "complete",
        "tools": tools::catalog(),
        "_meta": server_metadata(),
        "ttlMs": 0,
        "cacheScope": "private"
    })
}

/// Returns one successful JSON-RPC response.
fn result_response(id: Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

/// Returns one HTTP-bound JSON-RPC error without exposing request content.
fn error_response(
    status: StatusCode,
    id: Option<Value>,
    code: i64,
    message: &'static str,
    data: Option<Value>,
) -> Response {
    // Build the stable error object and attach protocol-defined data only when required.
    let mut error = json!({ "code": code, "message": message });
    if let Some(data) = data {
        error["data"] = data;
    }

    // Preserve a valid caller ID when available and omit it for pre-envelope failures.
    let mut response = json!({ "jsonrpc": "2.0", "error": error });
    if let Some(id) = id {
        response["id"] = id;
    }
    (status, Json(response)).into_response()
}
