//! Ignored loopback acceptance for the real OpenAI Python Responses SDK.
//!
//! Each case uses the production Router and a socket-backed synthetic upstream. The upstream is an
//! independent wire oracle: it validates request shape without generating expected data through the
//! production codec, and returns fixed synthetic JSON or SSE responses.

mod support;

use std::{
    env,
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use http::{StatusCode, header::CONTENT_TYPE};
use serde_json::{Value, json};
use support::{catalog_replay, process_replay};
use tokio::net::TcpListener;

const SDK_SCRIPT: &str = "tests/sdk/openai_responses_tool_loop.py";
const EXPECTED_ARGUMENTS: &str = "{\"location\":\"Shanghai\"}";
const EXPECTED_TOOL_OUTPUT: &str = "{\"condition\":\"sunny\",\"temperature_c\":25}";
const EXPECTED_TEXT: &str = "Shanghai is sunny at 25C.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WireMode {
    Json,
    Sse,
}

impl WireMode {
    const fn is_sse(self) -> bool {
        matches!(self, Self::Sse)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ablation {
    None,
    MissingCallId,
    WrongToolResult,
}

#[derive(Debug, Default)]
struct OracleObservation {
    requests: usize,
    first_request_matches: usize,
    second_request_matches: usize,
}

#[derive(Clone)]
struct OracleState {
    mode: WireMode,
    observation: Arc<Mutex<OracleObservation>>,
}

fn schema_matches(body: &Value) -> bool {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find(|tool| {
                tool.get("type").and_then(Value::as_str) == Some("function")
                    && tool.get("name").and_then(Value::as_str) == Some("get_weather")
            })
        })
        .is_some_and(|tool| {
            tool["strict"] == true
                && tool.get("parameters")
                    == Some(&json!({
                        "type": "object",
                        "properties": {"location": {"type": "string"}},
                        "required": ["location"],
                        "additionalProperties": false
                    }))
        })
}

fn common_request_matches(body: &Value, mode: WireMode) -> bool {
    body.get("model").and_then(Value::as_str) == Some("upstream-model")
        && body.get("stream").and_then(Value::as_bool) == Some(mode.is_sse())
        && body.get("store").and_then(Value::as_bool) == Some(false)
        && body.get("previous_response_id").is_none()
        && body.get("conversation").is_none()
        && body["max_output_tokens"] == 64
        && body["instructions"]
            == "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
        && schema_matches(body)
}

fn first_request_matches(body: &Value, mode: WireMode) -> bool {
    common_request_matches(body, mode)
        && body["tool_choice"] == "required"
        && body["input"] == json!([{"role":"user","content":"What is the weather in Shanghai?"}])
        && body
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("role").and_then(Value::as_str) == Some("user")
                        && item.get("content").is_some()
                })
            })
}

fn second_request_matches(body: &Value, mode: WireMode) -> bool {
    let Some(items) = body.get("input").and_then(Value::as_array) else {
        return false;
    };
    let has_call = items.iter().any(|item| {
        item.get("type").and_then(Value::as_str) == Some("function_call")
            && item.get("id").and_then(Value::as_str) == Some("fc_weather_shanghai")
            && item.get("call_id").and_then(Value::as_str) == Some("call_weather_shanghai")
            && item.get("name").and_then(Value::as_str) == Some("get_weather")
            && item.get("arguments").and_then(Value::as_str) == Some(EXPECTED_ARGUMENTS)
    });
    let has_output = items.iter().any(|item| {
        item.get("type").and_then(Value::as_str) == Some("function_call_output")
            && item.get("output").and_then(Value::as_str) == Some(EXPECTED_TOOL_OUTPUT)
            && item.get("call_id").and_then(Value::as_str) == Some("call_weather_shanghai")
    });
    common_request_matches(body, mode)
        && body["tool_choice"] == "none"
        && items.len() == 3
        && items[0]["role"] == "user"
        && items[0]["content"] == "What is the weather in Shanghai?"
        && items[1]["type"] == "function_call"
        && items[2]["type"] == "function_call_output"
        && has_call
        && has_output
}

fn json_response(first: bool) -> Value {
    if first {
        json!({
            "id": "resp_weather_first",
            "object": "response",
            "model": "upstream-model",
            "status": "completed",
            "output": [{
                "id": "fc_weather_shanghai",
                "type": "function_call",
                "status": "completed",
                "call_id": "call_weather_shanghai",
                "name": "get_weather",
                "arguments": EXPECTED_ARGUMENTS
            }],
            "usage": {"input_tokens": 7, "output_tokens": 3, "total_tokens": 10}
        })
    } else {
        json!({
            "id": "resp_weather_final",
            "object": "response",
            "model": "upstream-model",
            "status": "completed",
            "output": [{
                "id": "msg_weather_final",
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": EXPECTED_TEXT,
                    "annotations": []
                }]
            }],
            "usage": {"input_tokens": 15, "output_tokens": 7, "total_tokens": 22}
        })
    }
}

fn sse_response(first: bool) -> &'static str {
    if first {
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_weather_first\",\"object\":\"response\",\"model\":\"upstream-model\",\"status\":\"in_progress\",\"output\":[]}}\n\n",
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_weather_shanghai\",\"type\":\"function_call\",\"status\":\"in_progress\",\"call_id\":\"call_weather_shanghai\",\"name\":\"get_weather\",\"arguments\":\"\"}}\n\n",
            "event: response.function_call_arguments.delta\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_weather_shanghai\",\"output_index\":0,\"delta\":\"{\\\"location\\\":\\\"Shanghai\\\"}\"}\n\n",
            "event: response.function_call_arguments.done\n",
            "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc_weather_shanghai\",\"output_index\":0,\"arguments\":\"{\\\"location\\\":\\\"Shanghai\\\"}\"}\n\n",
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_weather_shanghai\",\"type\":\"function_call\",\"status\":\"completed\",\"call_id\":\"call_weather_shanghai\",\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Shanghai\\\"}\"}}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_weather_first\",\"object\":\"response\",\"model\":\"upstream-model\",\"status\":\"completed\",\"output\":[{\"id\":\"fc_weather_shanghai\",\"type\":\"function_call\",\"status\":\"completed\",\"call_id\":\"call_weather_shanghai\",\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Shanghai\\\"}\"}],\"usage\":{\"input_tokens\":7,\"output_tokens\":3,\"total_tokens\":10}}}\n\n"
        )
    } else {
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_weather_final\",\"object\":\"response\",\"model\":\"upstream-model\",\"status\":\"in_progress\",\"output\":[]}}\n\n",
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_weather_final\",\"type\":\"message\",\"status\":\"in_progress\",\"role\":\"assistant\",\"content\":[]}}\n\n",
            "event: response.content_part.added\n",
            "data: {\"type\":\"response.content_part.added\",\"output_index\":0,\"item_id\":\"msg_weather_final\",\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"\",\"annotations\":[]}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg_weather_final\",\"output_index\":0,\"content_index\":0,\"delta\":\"Shanghai is \"}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg_weather_final\",\"output_index\":0,\"content_index\":0,\"delta\":\"sunny at 25C.\"}\n\n",
            "event: response.output_text.done\n",
            "data: {\"type\":\"response.output_text.done\",\"item_id\":\"msg_weather_final\",\"output_index\":0,\"content_index\":0,\"text\":\"Shanghai is sunny at 25C.\"}\n\n",
            "event: response.content_part.done\n",
            "data: {\"type\":\"response.content_part.done\",\"output_index\":0,\"item_id\":\"msg_weather_final\",\"content_index\":0,\"part\":{\"type\":\"output_text\",\"text\":\"Shanghai is sunny at 25C.\",\"annotations\":[]}}\n\n",
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"msg_weather_final\",\"type\":\"message\",\"status\":\"completed\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Shanghai is sunny at 25C.\",\"annotations\":[]}]}}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_weather_final\",\"object\":\"response\",\"model\":\"upstream-model\",\"status\":\"completed\",\"output\":[{\"id\":\"msg_weather_final\",\"type\":\"message\",\"status\":\"completed\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Shanghai is sunny at 25C.\",\"annotations\":[]}]}],\"usage\":{\"input_tokens\":15,\"output_tokens\":7,\"total_tokens\":22}}}\n\n"
        )
    }
}

async fn upstream_responses(State(state): State<OracleState>, body: Bytes) -> Response {
    let parsed = serde_json::from_slice::<Value>(&body).ok();
    let request_index = {
        let mut observation = state
            .observation
            .lock()
            .expect("oracle mutex must not be poisoned");
        let index = observation.requests;
        observation.requests += 1;
        if let Some(body) = &parsed {
            if index == 0 && first_request_matches(body, state.mode) {
                observation.first_request_matches += 1;
            }
            if index == 1 && second_request_matches(body, state.mode) {
                observation.second_request_matches += 1;
            }
        }
        index
    };

    let valid = parsed.is_some_and(|body| match request_index {
        0 => first_request_matches(&body, state.mode),
        1 => second_request_matches(&body, state.mode),
        _ => false,
    });
    if !valid {
        return Response::builder()
            .status(if request_index == 1 {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::BAD_REQUEST
            })
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"error":{"message":"synthetic wire oracle rejected request"}}"#,
            ))
            .expect("oracle rejection response must build");
    }

    if state.mode.is_sse() {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from(sse_response(request_index == 0)))
            .expect("oracle SSE response must build")
    } else {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(json_response(request_index == 0).to_string()))
            .expect("oracle JSON response must build")
    }
}

async fn wait_for_ready(address: std::net::SocketAddr) {
    // The listener is already bound; verify the actual gateway endpoint, not any HTTP response.
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        client.get(format!("http://{address}/healthz")).send(),
    )
    .await
    .expect("loopback readiness must be bounded")
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Kill and reap the direct Python child if the async test is cancelled or panics.
struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Abort test-owned listeners on every exit path, including setup and child failures.
struct ServerGuard(tokio::task::AbortHandle);
impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn run_child(child: Child) -> Output {
    let mut owned = ChildGuard(Some(child));
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        match owned
            .0
            .as_mut()
            .unwrap()
            .try_wait()
            .expect("SDK child status must be readable")
        {
            Some(_) => {
                return owned
                    .0
                    .take()
                    .unwrap()
                    .wait_with_output()
                    .expect("SDK child output must be readable");
            }
            None if Instant::now() >= deadline => {
                panic!("OpenAI SDK fixture exceeded bounded timeout")
            }
            None => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    }
}

fn sdk_command(base_url: String, mode: WireMode, ablation: Ablation) -> Command {
    // Provision the pinned SDK outside the test; spawn Python directly so timeout cleanup owns it.
    let python = env::var_os("OPENBRIDGE_SDK_PYTHON").unwrap_or_else(|| "python3".into());
    let mut command = Command::new(python);
    command
        .args([SDK_SCRIPT, &base_url])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg(if mode.is_sse() { "sse" } else { "json" })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("OPENAI_ORG_ID")
        .env_remove("OPENAI_PROJECT_ID");
    match ablation {
        Ablation::None => {}
        Ablation::MissingCallId => {
            command.arg("missing-call-id");
        }
        Ablation::WrongToolResult => {
            command.arg("wrong-tool-result");
        }
    }
    command
}

async fn run_case(mode: WireMode, ablation: Ablation) -> OracleObservation {
    let observation = Arc::new(Mutex::new(OracleObservation::default()));
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("synthetic upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("synthetic upstream address must exist");
    let upstream_task = process_replay::spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(upstream_responses))
            .with_state(OracleState {
                mode,
                observation: observation.clone(),
            }),
    );

    let _upstream_guard = ServerGuard(upstream_task.abort_handle());
    let gateway = process_replay::start_gateway_with_definition(
        upstream_address,
        catalog_replay::replay_definition(),
    )
    .await;
    let _gateway_guard = ServerGuard(gateway.task.abort_handle());
    wait_for_ready(gateway.address).await;

    let base_url = format!("http://{}/v1", gateway.address);
    let child = sdk_command(base_url, mode, ablation).spawn().expect(
        "Python with the pinned SDK must start (run this gate through uv --with openai==3.10.0)",
    );
    let output = run_child(child).await;

    gateway.task.abort();
    upstream_task.abort();
    let _ = gateway.task.await;
    let _ = upstream_task.await;

    if ablation == Ablation::None {
        assert!(
            output.status.success(),
            "OpenAI SDK fixture must complete successfully (status={:?}, stderr_bytes={})",
            output.status.code(),
            output.stderr.len()
        );
        let summary: Value = serde_json::from_slice(&output.stdout)
            .expect("successful SDK fixture must emit one JSON summary");
        assert_eq!(summary["ok"], true);
        assert_eq!(summary["turns"], 2);
        assert_eq!(summary["tool_calls"], 1);
        assert_eq!(summary["final_status_completed"], true);
        assert_eq!(summary["final_output_text_matches"], true);
        assert_eq!(summary["usage_present"], true);
        if mode == WireMode::Sse {
            assert!(summary["first_event_count"].as_u64().unwrap_or(0) >= 5);
            assert!(summary["final_event_count"].as_u64().unwrap_or(0) >= 8);
        }
    } else {
        assert!(
            output.status.success(),
            "negative-control SDK fixture must report its expected failure (status={:?}, stdout_len={}, stderr_len={})",
            output.status.code(),
            output.stdout.len(),
            output.stderr.len()
        );
        let summary: Value = serde_json::from_slice(&output.stdout)
            .expect("negative-control fixture must emit one JSON summary");
        assert_eq!(summary["ok"], false);
        assert_eq!(summary["expected_failure"], true);
        match ablation {
            Ablation::MissingCallId => {
                // The second request must fail at gateway preflight, not SDK validation or the mock.
                assert_eq!(summary["http_status"], 400);
                assert_eq!(summary["error_code"], "unsupported_model_capability");
                assert_eq!(observation.lock().unwrap().requests, 1);
            }
            Ablation::WrongToolResult => {
                // A shape-valid but incorrect result reaches the independent upstream oracle.
                assert_eq!(summary["http_status"], 422);
                assert_eq!(observation.lock().unwrap().requests, 2);
            }
            Ablation::None => unreachable!(),
        }
    }

    println!("SDK loopback {mode:?} {ablation:?}: client assertions passed");
    Arc::try_unwrap(observation)
        .expect("oracle observation must have no remaining owners")
        .into_inner()
        .expect("oracle mutex must not be poisoned")
}

#[tokio::test]
#[ignore = "requires openai==3.10.0 on Python PATH; explicit JSON/SSE loopback acceptance"]
async fn openai_responses_sdk_json_and_sse_tool_loops_round_trip_through_production_router() {
    for mode in [WireMode::Json, WireMode::Sse] {
        let observation = run_case(mode, Ablation::None).await;
        assert_eq!(observation.requests, 2);
        assert_eq!(observation.first_request_matches, 1);
        assert_eq!(observation.second_request_matches, 1);
    }
}

#[tokio::test]
#[ignore = "runs independent negative controls against the same loopback wire oracle"]
async fn openai_responses_sdk_tool_loop_negative_controls_reject_identity_and_result_ablations() {
    let missing_call_id = run_case(WireMode::Json, Ablation::MissingCallId).await;
    assert_eq!(missing_call_id.first_request_matches, 1);
    assert_eq!(missing_call_id.second_request_matches, 0);

    let wrong_result = run_case(WireMode::Json, Ablation::WrongToolResult).await;
    assert_eq!(wrong_result.requests, 2);
    assert_eq!(wrong_result.first_request_matches, 1);
    assert_eq!(wrong_result.second_request_matches, 0);
}
