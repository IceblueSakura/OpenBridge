//! Separate SDK HTTP boundary; no gateway/catalog or real Provider is constructed.
#[allow(dead_code)]
#[path = "../support/client_carrier.rs"]
mod client_wire;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use openbridge::{
    adapter::{Adapter, Dialect},
    protocol::openai::Profile,
    semantic::{task::generation::*, value::ReplayOrigin},
};
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, process::Command};
fn adapter() -> Adapter {
    Adapter::new(
        Profile::Responses,
        Dialect::OpenBridge,
        Some(ReplayOrigin::new("synthetic-client").unwrap()),
    )
}
#[derive(Clone, Default)]
struct Context(Arc<Mutex<(usize, Option<&'static str>)>>);
fn failure(context: &Context, name: &'static str) -> Response {
    context.0.lock().unwrap().1 = Some(name);
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::empty())
        .unwrap()
}
async fn handle(State(context): State<Context>, headers: HeaderMap, body: Bytes) -> Response {
    if headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        != Some("Bearer synthetic-local-token")
        || body.len() > 64 << 10
    {
        return failure(&context, "request boundary");
    }
    let request = match adapter().decode_request(&body) {
        Ok(request) => request,
        Err(_) => return failure(&context, "client decode"),
    };
    if request.model != "fixture-model" {
        return failure(&context, "model");
    }
    let turn = {
        let mut value = context.0.lock().unwrap();
        value.0 += 1;
        value.0
    };
    if turn == 2 {
        let items = request.task.semantic.items();
        if items.len() != 7
            || !matches!(&items[1].1,Item::Reasoning(r) if r.replay.as_ref().is_some_and(|value|value.format()==ReplayFormat::GoogleInteractionsV1Thought))
            || !matches!(&items[3].1,Item::ToolCall(c) if c.message==Some(ItemId::new(3)) && c.arguments.as_structured().is_some_and(|value|value.value()["n"].as_number().and_then(serde_json::Number::as_u128)==Some(18446744073709551616001)))
            || !matches!(&items[6].1,Item::ToolResult(r) if matches!(r.execution,Some(ToolExecution::Failed{..})) && matches!(r.output,ToolOutput::Structured(_)))
        {
            return failure(&context, "returned semantic attachment");
        }
        let mut response = crate::wire::response(2);
        response["created_at"] = json!(1);
        response["completed_at"] = json!(2);
        return Response::builder()
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap();
    }
    if turn != 1 {
        return failure(&context, "turn count");
    }
    let mut fixture = client_wire::response();
    fixture["usage"] = Value::Null;
    fixture["_openbridge"]["usage"] = json!([
        {"scope":{"item":"call-one"},"basis":"final","output_relation":"unreported","total_relation":"unreported","input_tokens":2},
        {"scope":"session","basis":"cumulative","output_relation":"excludes_reasoning","total_relation":"input_output_and_reasoning","input_tokens":5,"output_tokens":3,"reasoning_tokens":2,"total_tokens":10}
    ]);
    let decoded = adapter()
        .decode_response(&serde_json::to_vec(&fixture).unwrap())
        .unwrap();
    let mut contract = openbridge::lowering::generation::GenerationRepresentationContract::full();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    if !request.delivery.streaming() {
        let value = adapter().encode_response(&decoded, &contract).unwrap();
        return Response::builder()
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap();
    }
    let mut decoder = adapter().event_decoder();
    let mut events = vec![];
    for value in client_wire::events_for(fixture) {
        events.extend(decoder.push(&value).unwrap());
    }
    decoder.finish().unwrap();
    let mut encoder = openbridge::protocol::openai::events::EventEncoder::new(
        Profile::Responses,
        decoded.metadata,
    )
    .unwrap()
    .with_contract(adapter().contract(&contract));
    let mut bytes = Vec::new();
    for event in events {
        for payload in encoder.encode(&event, decoder.fidelity()).unwrap() {
            let name = payload["type"].as_str().unwrap();
            bytes.extend_from_slice(format!("event: {name}\ndata: {payload}\n\n").as_bytes());
        }
    }
    encoder.finish().unwrap();
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from(bytes))
        .unwrap()
}
#[tokio::test]
#[ignore = "requires locked SDK; explicit synthetic HTTP JSON/SSE client attachments"]
async fn sdk_client_attachments_persist_and_return_without_inventing_scope() {
    for stream in [false, true] {
        let context = Context::default();
        let observed = context.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/responses", post(handle))
                    .with_state(context),
            )
            .await
            .unwrap();
        });
        let guard = crate::ServerGuard(server.abort_handle());
        let mut command = Command::new(
            std::env::var_os("OPENBRIDGE_SDK_PYTHON").unwrap_or_else(|| "python3".into()),
        );
        command
            .args([
                "tests/sdk/client_carrier_loop.py",
                &format!("http://{address}/v1"),
                if stream { "sse" } else { "json" },
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in [
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "OPENAI_ORG_ID",
            "OPENAI_PROJECT_ID",
            "OPENBRIDGE_CONFIG",
        ] {
            command.env_remove(name);
        }
        let output =
            crate::child_process::run(&mut command, b"", Duration::from_secs(35), 64 << 10)
                .await
                .unwrap();
        guard.0.abort();
        let _ = server.await;
        assert!(
            output.status.success(),
            "synthetic SDK carrier failure {:?}: {}",
            observed.0.lock().unwrap().1,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(observed.0.lock().unwrap().0, 2);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["turns"], json!(2));
    }
}
