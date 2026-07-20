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
    config::{ConfigManager, ResolvedDeployment, load_registry},
    ingress::{AppState, StaticBearerCredential, build_router},
    provider::{CredentialSource, UpstreamRequestParts},
    transport::upstream::{UpstreamError, UpstreamResponse, UpstreamTransport},
};
use secrecy::SecretString;
use serde_json::{Value, json};
use tokio::net::TcpListener;

const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
allowed_origins = ["https://api.openai.com"]
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

const ROUTES: &str = r#"
schema_version = 1
config_version = "sdk-compatibility"
[[providers]]
id = "openai"
kind = "openai"
[providers.credential]
id = "openai-primary"
kind = "api_key"
secret_ref = "env://OPENAI_API_KEY"
[[deployments]]
id = "openai-main"
provider = "openai"
upstream_model = "upstream-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities]
chat = true
responses = true
streaming = true
function_tools = true
structured_output = false
previous_response_id = false
background = false
response_store = false
[[aliases]]
name = "public-model"
candidates = ["openai-main"]
"#;

struct SdkFixtureTransport;

impl UpstreamTransport for SdkFixtureTransport {
    fn send<'a>(
        &'a self,
        _deployment: &'a ResolvedDeployment,
        request: UpstreamRequestParts,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
        let path = request.relative_uri().path().to_owned();
        let request: Value = serde_json::from_slice(request.body()).expect("SDK request is JSON");
        assert_eq!(request["model"], "upstream-model");
        let stream_requested = request["stream"].as_bool().unwrap_or(false);
        Box::pin(async move { Ok(fixture_response(&path, stream_requested)) })
    }
}

#[tokio::test]
#[ignore = "downloads OpenAI Python/Node SDKs into tool caches and starts a loopback server"]
async fn openai_python_and_node_sdks_consume_native_chat_and_responses() {
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
            "openai==2.46.0",
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
    let install = run_command({
        let mut command = Command::new(command_path("OPENBRIDGE_NPM", "npm"));
        command.args([
            "install",
            "--no-save",
            "--prefix",
            node_root.to_str().unwrap(),
            "openai@6.48.0",
        ]);
        command
    })
    .await;
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
    let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
    build_router(AppState::new(
        Arc::new(ConfigManager::new(snapshot)),
        Arc::new(SdkFixtureTransport),
        StaticBearerCredential::new(SecretString::from("downstream-token".to_owned())),
        CredentialSource::fixed(
            "OPENAI_API_KEY",
            SecretString::from("upstream-token".to_owned()),
        ),
    ))
}

fn fixture_response(path: &str, stream_requested: bool) -> UpstreamResponse {
    let (content_type, chunks) = match (path, stream_requested) {
        ("/v1/chat/completions", false) => (
            "application/json",
            vec![Bytes::from(chat_completion().to_string())],
        ),
        ("/v1/chat/completions", true) => {
            ("text/event-stream", fragmented_sse_chunks(chat_stream()))
        }
        ("/v1/responses", false) => (
            "application/json",
            vec![Bytes::from(response("resp_nonstream").to_string())],
        ),
        ("/v1/responses", true) => (
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

fn fragmented_sse_chunks(body: String) -> Vec<Bytes> {
    let body = body.into_bytes();
    let split = body
        .windows(2)
        .position(|window| window == [0xc3, 0xa9])
        .expect("fixture contains a multi-byte UTF-8 character")
        + 1;
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

async fn run_command(mut command: Command) -> Output {
    tokio::task::spawn_blocking(move || command.output().expect("SDK command should start"))
        .await
        .expect("SDK command task should complete")
}

fn command_path(variable: &str, fallback: &str) -> std::ffi::OsString {
    env::var_os(variable).unwrap_or_else(|| fallback.into())
}

fn assert_success(name: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
