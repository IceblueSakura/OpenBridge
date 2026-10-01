//! Official SDK -> production Router -> synthetic HTTP Provider -> the same Router.
#[path = "../support/gateway.rs"]
mod support;
use super::{ServerGuard, chat_sdk::wire as chat_wire, wire};
use axum::{
    Router, body::Body, extract::State, http::HeaderMap, response::Response, routing::post,
};
use openbridge::gateway::Limits;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpListener,
    process::Command,
    sync::oneshot,
};
async fn bounded_output(reader: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut bytes = Vec::new();
    reader
        .take((64 << 10) + 1)
        .read_to_end(&mut bytes)
        .await
        .unwrap();
    bytes
}
fn owner_response() -> Value {
    let mut value = wire::response(2);
    value["output"] = json!([
        {"id":"empty-owner","type":"message","role":"assistant","content":[],"status":"completed"},
        {"id":"lookup-call","type":"function_call","call_id":"call-local","name":"lookup","arguments":"{\"n\":1}","status":"completed"}
    ]);
    value
}
fn owner_events() -> Vec<Value> {
    let completed = owner_response();
    let mut initial = completed.clone();
    initial["status"] = json!("in_progress");
    initial["output"] = json!([]);
    initial["usage"] = Value::Null;
    initial["completed_at"] = Value::Null;
    vec![
        json!({"type":"response.created","response":initial}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"empty-owner","type":"message","role":"assistant","content":[],"status":"in_progress"}}),
        json!({"type":"response.output_item.done","output_index":0,"item":completed["output"][0]}),
        json!({"type":"response.output_item.added","output_index":1,"item":{"id":"lookup-call","type":"function_call","call_id":"call-local","name":"lookup","arguments":"","status":"in_progress"}}),
        json!({"type":"response.function_call_arguments.delta","output_index":1,"item_id":"lookup-call","delta":"{\"n\":1}"}),
        json!({"type":"response.function_call_arguments.done","output_index":1,"item_id":"lookup-call","arguments":"{\"n\":1}"}),
        json!({"type":"response.output_item.done","output_index":1,"item":completed["output"][1]}),
        json!({"type":"response.completed","response":completed}),
    ]
}
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Scenario {
    chat: bool,
    stream: bool,
    empty_owner: bool,
}
#[derive(Clone, Default)]
struct Observed(Arc<Mutex<BTreeMap<Scenario, u8>>>);
async fn provider(
    State(state): State<Observed>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Value>,
) -> Response {
    assert_eq!(
        headers["authorization"],
        "Bearer synthetic-upstream-credential-0001"
    );
    assert_eq!(request["model"], "private-model");
    let chat = request.get("messages").is_some();
    let stream = request["stream"] == true;
    let owner_case = request["metadata"]["case"] == "empty-owner";
    if owner_case {
        assert!(
            !chat,
            "SDK item validation uses a complete Responses envelope"
        );
    }
    assert_eq!(
        request[if chat {
            "max_completion_tokens"
        } else {
            "max_output_tokens"
        }],
        32
    );
    let turn = {
        let mut seen = state.0.lock().unwrap();
        let count = seen
            .entry(Scenario {
                chat,
                stream,
                empty_owner: owner_case,
            })
            .or_default();
        *count += 1;
        *count
    };
    assert!(turn <= if owner_case { 1 } else { 2 });
    if turn == 2 {
        let history = request[if chat { "messages" } else { "input" }]
            .as_array()
            .unwrap();
        if chat {
            assert!(history.iter().any(|item| item["role"] == "tool"
                && item["tool_call_id"] == "call-local"
                && item["content"] == "{\"n\":1}"));
        } else {
            assert!(history.iter().any(|item|item["type"]=="function_call" && item["arguments"]=="{\"n\":1}"));
            assert!(
                history
                    .iter()
                    .any(|item| item["type"] == "function_call_output"
                        && item["call_id"] == "c_lookup"
                        && item["output"] == "{\"n\":1}")
            );
            assert!(history.iter().any(
                |item| item["type"] == "custom_tool_call_output" && item["call_id"] == "c_sql"
            ));
            assert!(history.iter().any(|item| item["type"] == "reasoning"
                && item["encrypted_content"] == "synthetic-final-token"));
        }
    }
    let snapshot = |mut value: Value| {
        value["model"] = json!("private-model");
        if !chat {
            value["created_at"] = json!(1);
            if !value["completed_at"].is_null() {
                value["completed_at"] = json!(2);
            }
            value["max_output_tokens"] = json!(32);
        }
        value
    };
    let bytes = if stream {
        let mut output = Vec::new();
        let frames = if owner_case {
            owner_events()
        } else if chat {
            chat_wire::events(turn)
        } else {
            wire::events(turn)
        };
        for (sequence, mut value) in frames.into_iter().enumerate() {
            if !chat {
                value["sequence_number"] = json!(sequence);
            }
            if chat {
                value["model"] = json!("private-model");
            } else if value.get("response").is_some() {
                value["response"] = snapshot(value["response"].take());
            }
            if chat {
                output.extend_from_slice(format!("data: {value}\n\n").as_bytes());
            } else {
                output.extend_from_slice(
                    &openbridge::protocol::openai::sse::encode_frame(&value, 1 << 20).unwrap(),
                );
            }
        }
        if chat {
            output.extend_from_slice(b"data: [DONE]\n\n");
        }
        output
    } else {
        serde_json::to_vec(&snapshot(if owner_case {
            owner_response()
        } else if chat {
            chat_wire::response(turn)
        } else {
            wire::response(turn)
        }))
        .unwrap()
    };
    Response::builder()
        .header(
            "content-type",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .body(Body::from(bytes))
        .unwrap()
}
#[tokio::test]
#[ignore = "requires the pinned SDK; real gateway Router and synthetic HTTP Provider only"]
async fn sdk_uses_gateway_for_both_protocols_and_deliveries() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let observed = Observed::default();
    let state = observed.clone();
    let upstream = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/chat/completions", post(provider))
                .route("/responses", post(provider))
                .with_state(state),
        )
        .await
        .unwrap();
    });
    let _upstream = ServerGuard(upstream.abort_handle());
    let gateway = support::gateway(
        &origin,
        Limits {
            default_output_tokens: 32,
            ..Limits::default()
        },
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (stop, stopped) = oneshot::channel();
    let serving = tokio::spawn(gateway.serve(listener, async {
        let _ = stopped.await;
    }));
    let _server = ServerGuard(serving.abort_handle());
    let mut command =
        Command::new(std::env::var_os("OPENBRIDGE_SDK_PYTHON").unwrap_or_else(|| "python3".into()));
    command
        .args(["tests/sdk/gateway_text_loop.py", &base])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if text.starts_with("OPENAI_") || text.starts_with("OPENBRIDGE_") {
            command.env_remove(name);
        }
    }
    command.kill_on_drop(true);
    let mut child = command.spawn().expect("locked SDK Python");
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(35), async {
        tokio::join!(child.wait(), bounded_output(stdout), bounded_output(stderr))
    })
    .await;
    let (status, stdout, stderr) = match result {
        Ok(result) => result,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            panic!("bounded SDK gateway process timed out");
        }
    };
    assert!(
        stdout.len() <= 64 << 10 && stderr.len() <= 64 << 10,
        "SDK report exceeded capture limit"
    );
    assert!(
        status.unwrap().success(),
        "SDK gateway failed: {}",
        String::from_utf8_lossy(&stderr)
    );
    let report: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(report["requests"], 10);
    {
        let observed = observed.0.lock().unwrap();
        assert_eq!(observed.len(), 6);
        assert!(
            observed
                .iter()
                .all(|(scenario, count)| *count == if scenario.empty_owner { 1 } else { 2 })
        );
    }
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), serving)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
