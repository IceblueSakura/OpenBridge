//! Verifies loopback HTTP/SSE compatibility with OpenAI Python and Node SDKs installed at runtime.
//!
//! These tests are ignored by default; external SDKs are installed or invoked only when ignored tests are explicitly run.

mod support;

use std::{
    convert::Infallible,
    env, fs,
    process::{Command, Output},
    sync::Arc,
};

use axum::body::Body;
use bytes::Bytes;
use futures_util::{future::BoxFuture, stream};
use http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

struct SdkFixtureTransport;

impl UpstreamTransport for SdkFixtureTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let path = request.relative_uri().path().to_owned();
        let request: Value = serde_json::from_slice(request.body()).expect("SDK request is JSON");
        assert_eq!(request["model"], "upstream-model");
        assert_tool_result_identity(&path, &request);
        let stream_requested = request["stream"].as_bool().unwrap_or(false);
        Box::pin(async move { Ok(fixture_response(&path, &request, stream_requested)) })
    }
}

#[tokio::test]
#[ignore = "downloads OpenAI Python/Node SDKs into tool caches and starts a loopback server"]
async fn openai_python_and_node_sdks_consume_native_chat_responses_and_tools() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app().into_make_service())
            .await
            .expect("SDK fixture server should run");
    });

    let python = run_command({
        let mut command = Command::new(command_path("OPENBRIDGE_UV", "uv"));
        command.args([
            "run",
            "--isolated",
            "--with",
            "openai",
            "python",
            "tests/sdk/openai_python_compat.py",
            &base_url,
        ]);
        command
    })
    .await;
    assert_success("OpenAI Python SDK", &python);

    let node_root =
        std::env::temp_dir().join(format!("openbridge-openai-node-{}", std::process::id()));
    let _ = fs::remove_dir_all(&node_root);
    fs::create_dir_all(&node_root).expect("Node SDK temporary directory should be created");
    let install = run_command(node_sdk_install_command(&node_root)).await;
    assert_success("OpenAI Node SDK install", &install);

    let node = run_command({
        let mut command = Command::new(command_path("OPENBRIDGE_NODE", "node"));
        command
            .env("NODE_PATH", node_root.join("node_modules"))
            .args(["tests/sdk/openai_node_compat.cjs", &base_url]);
        command
    })
    .await;
    let _ = fs::remove_dir_all(&node_root);
    assert_success("OpenAI Node SDK", &node);

    server.abort();
}

fn app() -> axum::Router {
    let registry = support::registry("sdk-compatibility", "public-model", "upstream-model");
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "upstream-token",
    );
    build_router(GatewayState::new(
        Arc::new(registry),
        Arc::new(SdkFixtureTransport),
        users,
        credentials,
    ))
}

fn fixture_response(path: &str, request: &Value, stream_requested: bool) -> UpstreamResponse {
    if is_error_probe(path, request) {
        return fixture_rate_limit_response();
    }

    let tool_count = request
        .get("tools")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let has_tools = tool_count > 0;
    let parallel_tools = tool_count > 1;
    let has_chat_tool_result = request
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages
                .iter()
                .any(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
        });
    let has_response_tool_result =
        request
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("type").and_then(Value::as_str) == Some("function_call_output")
                })
            });
    let (content_type, chunks) = match (path, stream_requested, has_tools) {
        ("/v1/chat/completions", false, true) if has_chat_tool_result => (
            "application/json",
            vec![Bytes::from(chat_completion().to_string())],
        ),
        ("/v1/chat/completions", false, true) => (
            "application/json",
            vec![Bytes::from(chat_tool_call(parallel_tools).to_string())],
        ),
        ("/v1/chat/completions", false, false) => (
            "application/json",
            vec![Bytes::from(chat_completion().to_string())],
        ),
        ("/v1/chat/completions", true, true) => (
            "text/event-stream",
            fragmented_sse_chunks(chat_tool_stream()),
        ),
        ("/v1/chat/completions", true, false) => {
            ("text/event-stream", fragmented_sse_chunks(chat_stream()))
        }
        ("/v1/responses", false, true) if has_response_tool_result => (
            "application/json",
            vec![Bytes::from(response("resp_tool_result").to_string())],
        ),
        ("/v1/responses", false, true) => (
            "application/json",
            vec![Bytes::from(
                response_with_tool_call("resp_tool_call", parallel_tools).to_string(),
            )],
        ),
        ("/v1/responses", false, false) => (
            "application/json",
            vec![Bytes::from(response("resp_nonstream").to_string())],
        ),
        ("/v1/responses", true, true) => (
            "text/event-stream",
            fragmented_sse_chunks(responses_tool_stream()),
        ),
        ("/v1/responses", true, false) => (
            "text/event-stream",
            fragmented_sse_chunks(responses_stream()),
        ),
        _ => panic!("unexpected OpenAI SDK path {path}"),
    };
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    UpstreamResponse::new(
        StatusCode::OK,
        headers,
        Body::from_stream(stream::iter(chunks.into_iter().map(Ok::<_, Infallible>))),
    )
}

fn is_error_probe(path: &str, request: &Value) -> bool {
    match path {
        "/v1/chat/completions" => request
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages.iter().any(|message| {
                    message.get("content").and_then(Value::as_str) == Some("trigger-upstream-error")
                })
            }),
        "/v1/responses" => {
            request.get("input").and_then(Value::as_str) == Some("trigger-upstream-error")
        }
        _ => false,
    }
}

fn fixture_rate_limit_response() -> UpstreamResponse {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    UpstreamResponse::new(
        StatusCode::TOO_MANY_REQUESTS,
        headers,
        Body::from(
            json!({
                "error": {
                    "message": "fixture rate limited",
                    "type": "rate_limit_error",
                    "code": "sdk_fixture_rate_limited"
                }
            })
            .to_string(),
        ),
    )
}

fn assert_tool_result_identity(path: &str, request: &Value) {
    match path {
        "/v1/chat/completions" => {
            let tool_call_ids = request
                .get("messages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
                .filter_map(|message| message.get("tool_call_id").and_then(Value::as_str))
                .collect::<Vec<_>>();
            if !tool_call_ids.is_empty() {
                let expected = if tool_count(request) > 1 {
                    vec!["call_sdk_chat_1", "call_sdk_chat_2"]
                } else {
                    vec!["call_sdk_chat_1"]
                };
                assert_eq!(tool_call_ids, expected);
            }
        }
        "/v1/responses" => {
            let call_ids = request
                .get("input")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("function_call_output")
                })
                .filter_map(|item| item.get("call_id").and_then(Value::as_str))
                .collect::<Vec<_>>();
            if !call_ids.is_empty() {
                let expected = if tool_count(request) > 1 {
                    vec!["call_sdk_response_1", "call_sdk_response_2"]
                } else {
                    vec!["call_sdk_response_1"]
                };
                assert_eq!(call_ids, expected);
            }
        }
        _ => panic!("unexpected OpenAI SDK path {path}"),
    }
}

fn tool_count(request: &Value) -> usize {
    request
        .get("tools")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn fragmented_sse_chunks(body: String) -> Vec<Bytes> {
    let body = body.into_bytes();
    let split = body
        .windows(2)
        .position(|window| window == [0xc3, 0xa9])
        .map(|offset| offset + 1)
        .unwrap_or_else(|| body.len() / 2);
    vec![
        Bytes::copy_from_slice(&body[..split]),
        Bytes::copy_from_slice(&body[split..]),
    ]
}

fn chat_completion() -> Value {
    json!({
        "id": "chatcmpl_nonstream",
        "object": "chat.completion",
        "created": 0,
        "model": "upstream-model",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello", "refusal": null},
            "logprobs": null,
            "finish_reason": "stop"
        }]
    })
}

fn chat_tool_call(parallel: bool) -> Value {
    let mut tool_calls = vec![json!({
        "id": "call_sdk_chat_1",
        "type": "function",
        "function": {"name": "get_weather", "arguments": "{\"city\":\"Shanghai\"}"}
    })];
    if parallel {
        tool_calls.push(json!({
            "id": "call_sdk_chat_2",
            "type": "function",
            "function": {"name": "get_time", "arguments": "{\"zone\":\"Asia/Shanghai\"}"}
        }));
    }
    json!({
        "id": "chatcmpl_tool_call",
        "object": "chat.completion",
        "created": 0,
        "model": "upstream-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": tool_calls
            },
            "logprobs": null,
            "finish_reason": "tool_calls"
        }]
    })
}

fn chat_stream() -> String {
    let first = json!({
        "id": "chatcmpl_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{"index": 0, "delta": {"content": "hé"}, "finish_reason": null}]
    });
    let second = json!({
        "id": "chatcmpl_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{"index": 0, "delta": {"content": "llo"}, "finish_reason": null}]
    });
    let terminal = json!({
        "id": "chatcmpl_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });
    format!("data: {first}\n\ndata: {second}\n\ndata: {terminal}\n\ndata: [DONE]\n\n")
}

fn chat_tool_stream() -> String {
    let first = json!({
        "id": "chatcmpl_tool_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "tool_calls": [{
                    "index": 0,
                    "id": "call_sdk_chat_1",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{\"city\":\""}
                }]
            },
            "finish_reason": null
        }]
    });
    let second = json!({
        "id": "chatcmpl_tool_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{
            "index": 0,
            "delta": {"tool_calls": [{"index": 0, "function": {"arguments": "Shanghai\"}"}}]},
            "finish_reason": null
        }]
    });
    let terminal = json!({
        "id": "chatcmpl_tool_stream",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "upstream-model",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]
    });
    format!("data: {first}\n\ndata: {second}\n\ndata: {terminal}\n\ndata: [DONE]\n\n")
}

fn response(id: &str) -> Value {
    json!({
        "id": id,
        "object": "response",
        "created_at": 0,
        "model": "upstream-model",
        "output": [],
        "parallel_tool_calls": true,
        "tool_choice": "auto",
        "tools": []
    })
}

fn response_with_tool_call(id: &str, parallel: bool) -> Value {
    let mut response = response(id);
    let mut output = vec![json!({
        "type": "function_call",
        "id": "fc_sdk_response_1",
        "call_id": "call_sdk_response_1",
        "name": "get_weather",
        "arguments": "{\"city\":\"Shanghai\"}",
        "status": "completed"
    })];
    if parallel {
        output.push(json!({
            "type": "function_call",
            "id": "fc_sdk_response_2",
            "call_id": "call_sdk_response_2",
            "name": "get_time",
            "arguments": "{\"zone\":\"Asia/Shanghai\"}",
            "status": "completed"
        }));
    }
    response["output"] = Value::Array(output);
    response
}

fn responses_stream() -> String {
    let delta = json!({
        "type": "response.output_text.delta",
        "sequence_number": 1,
        "item_id": "msg_stream",
        "output_index": 0,
        "content_index": 0,
        "delta": "héllo",
        "logprobs": []
    });
    let completed = json!({
        "type": "response.completed",
        "sequence_number": 2,
        "response": response("resp_stream")
    });
    let delta = delta.to_string();
    let split = delta
        .find(",\"delta\"")
        .expect("fixture delta has a second JSON property");
    format!(
        "event: response.output_text.delta\ndata: {}\ndata: {}\n\nevent: response.completed\ndata: {completed}\n\n",
        &delta[..split],
        &delta[split..],
    )
}

fn responses_tool_stream() -> String {
    let item = json!({
        "type": "function_call",
        "id": "fc_sdk_response_stream_1",
        "call_id": "call_sdk_response_stream_1",
        "name": "get_weather",
        "arguments": "{\"city\":\"Shanghai\"}",
        "status": "completed"
    });
    let added = json!({
        "type": "response.output_item.added",
        "sequence_number": 1,
        "output_index": 0,
        "item": item
    });
    let first_delta = json!({
        "type": "response.function_call_arguments.delta",
        "sequence_number": 2,
        "item_id": "fc_sdk_response_stream_1",
        "output_index": 0,
        "delta": "{\"city\":\""
    });
    let second_delta = json!({
        "type": "response.function_call_arguments.delta",
        "sequence_number": 3,
        "item_id": "fc_sdk_response_stream_1",
        "output_index": 0,
        "delta": "Shanghai\"}"
    });
    let arguments_done = json!({
        "type": "response.function_call_arguments.done",
        "sequence_number": 4,
        "item_id": "fc_sdk_response_stream_1",
        "output_index": 0,
        "arguments": "{\"city\":\"Shanghai\"}"
    });
    let item_done = json!({
        "type": "response.output_item.done",
        "sequence_number": 5,
        "output_index": 0,
        "item": response_with_tool_call("resp_tool_stream", false)["output"][0].clone()
    });
    let completed = json!({
        "type": "response.completed",
        "sequence_number": 6,
        "response": response_with_tool_call("resp_tool_stream", false)
    });
    format!(
        "event: response.output_item.added\ndata: {added}\n\
         \nevent: response.function_call_arguments.delta\ndata: {first_delta}\n\
         \nevent: response.function_call_arguments.delta\ndata: {second_delta}\n\
         \nevent: response.function_call_arguments.done\ndata: {arguments_done}\n\
         \nevent: response.output_item.done\ndata: {item_done}\n\
         \nevent: response.completed\ndata: {completed}\n\n"
    )
}

async fn run_command(mut command: Command) -> Output {
    tokio::task::spawn_blocking(move || command.output().expect("SDK command should start"))
        .await
        .expect("SDK command task should complete")
}

fn command_path(variable: &str, fallback: &str) -> std::ffi::OsString {
    env::var_os(variable).unwrap_or_else(|| fallback.into())
}

fn node_sdk_install_command(node_root: &std::path::Path) -> Command {
    if let Some(pnpm) = env::var_os("OPENBRIDGE_PNPM") {
        let mut command = Command::new(pnpm);
        command.args(["--dir", node_root.to_str().unwrap(), "add", "openai"]);
        return command;
    }

    let mut command = Command::new(command_path("OPENBRIDGE_NPM", "npm"));
    command.args([
        "install",
        "--no-save",
        "--prefix",
        node_root.to_str().unwrap(),
        "openai",
    ]);
    command
}

fn assert_success(name: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
