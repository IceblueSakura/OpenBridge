//! Small semantic replay harness for production Router bridge regressions.
//!
//! This module loads selected semantic cases and reference traces, builds only the fixed wire
//! shapes needed by those cases, and checks the upstream/downstream observations with independent
//! JSON/SSE projections. It deliberately does not call the production codec to make expectations.

use std::{path::Path, sync::Arc, time::Duration};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    core::{ApiProtocol, JsonSchemaSupport, StructuredOutputProfile},
    registry::UpstreamApiCapabilities,
};
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

use crate::support::{catalog_replay, process_replay};

const DOWNSTREAM_TOKEN: &str = "downstream-token-0000000000000000";
const MAX_BODY: usize = 512 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    ChatToResponses,
    ResponsesToChat,
}

impl Direction {
    pub const fn downstream(self) -> ApiProtocol {
        match self {
            Self::ChatToResponses => ApiProtocol::ChatCompletions,
            Self::ResponsesToChat => ApiProtocol::Responses,
        }
    }

    pub const fn upstream(self) -> ApiProtocol {
        match self {
            Self::ChatToResponses => ApiProtocol::Responses,
            Self::ResponsesToChat => ApiProtocol::ChatCompletions,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioKind {
    ToolHistory,
    ParallelArguments,
    StructuredOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticEvent {
    ToolCall {
        call_id: String,
        name: String,
        arguments: Value,
    },
    ToolResult {
        call_id: String,
        output: Value,
    },
    AssistantMessage {
        text: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestProjection {
    pub protocol: ApiProtocol,
    pub tools: Vec<(String, Value, bool)>,
    pub events: Vec<SemanticEvent>,
    pub structured_schema: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseProjection {
    pub protocol: ApiProtocol,
    pub events: Vec<SemanticEvent>,
    pub terminal_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticObservation {
    pub id: &'static str,
    pub direction: Direction,
    pub kind: ScenarioKind,
    pub stream: bool,
    pub request: RequestProjection,
    pub response: ResponseProjection,
    pub expected_request_events: Vec<SemanticEvent>,
    pub expected_response_events: Vec<SemanticEvent>,
    pub expected_tools: Vec<(String, Value, bool)>,
    pub expected_schema: Option<Value>,
}

struct Scenario {
    id: &'static str,
    direction: Direction,
    kind: ScenarioKind,
    stream: bool,
    client_request: Bytes,
    upstream_body: Bytes,
    semantic_case: Value,
    reference_events: Vec<SemanticEvent>,
}

#[derive(Clone)]
struct MockState {
    response: Bytes,
    content_type: &'static str,
    requests: Arc<Mutex<Vec<Value>>>,
}

/// Runs the minimal directional matrix through the production Router.
pub async fn run_matrix() -> Vec<SemanticObservation> {
    let scenarios = [
        build_tool_history("function.result_grounding", Direction::ChatToResponses),
        build_tool_history("function.result_grounding", Direction::ResponsesToChat),
        build_parallel(Direction::ChatToResponses),
        build_parallel(Direction::ResponsesToChat),
        build_structured(Direction::ChatToResponses),
    ];
    let mut observations = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        observations.push(run_scenario(scenario).await);
    }
    observations
}

/// Parses the selected semantic reference trace without copying its task into test code.
pub fn load_reference_events(case_id: &str) -> Vec<SemanticEvent> {
    parse_trace(&load_semantic_json(case_id, "reference-trace.json"))
}

fn build_tool_history(case_id: &'static str, direction: Direction) -> Scenario {
    let semantic_case = load_semantic_json(case_id, "case.json");
    let reference_events = load_reference_events(case_id);
    let tool_values = semantic_case["task"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let request = match direction {
        Direction::ChatToResponses => {
            let call = expect_call(&reference_events, 0);
            let result = expect_result(&reference_events, 1);
            json!({
                "messages": [
                    {"role":"assistant", "content":Value::Null, "tool_calls":[{
                        "id": call_id(call), "type":"function", "function": {
                            "name": call_name(call), "arguments": compact(arguments(call))
                        }
                    }]},
                    {"role":"tool", "tool_call_id": result_id(result), "content": compact(output(result))}
                ],
                "model":"public-model", "stream":false,
                "tools": tool_values.iter().map(chat_tool).collect::<Vec<_>>()
            })
        }
        Direction::ResponsesToChat => {
            let call = expect_call(&reference_events, 0);
            let result = expect_result(&reference_events, 1);
            json!({
                "input": [
                    {"type":"function_call", "id":"function_item", "call_id":call_id(call),
                     "name":call_name(call), "arguments":compact(arguments(call))},
                    {"type":"function_call_output", "call_id":result_id(result),
                     "output":compact(output(result))}
                ],
                "model":"public-model", "stream":false,
                "tools": tool_values
            })
        }
    };
    let upstream_text = expect_message(&reference_events, 2);
    let upstream_body = build_json_response(direction.upstream(), &upstream_text, &[]);
    Scenario {
        id: if direction == Direction::ChatToResponses {
            "chat_to_responses.tool_history"
        } else {
            "responses_to_chat.tool_history"
        },
        direction,
        kind: ScenarioKind::ToolHistory,
        stream: false,
        client_request: json_bytes(request),
        upstream_body,
        semantic_case,
        reference_events,
    }
}

fn build_parallel(direction: Direction) -> Scenario {
    let semantic_case = load_semantic_json("function.parallel_independent", "case.json");
    let reference_events = load_reference_events("function.parallel_independent");
    let tools = semantic_case["task"]["tools"]
        .as_array()
        .expect("parallel semantic tools must be an array");
    let request = match direction {
        Direction::ChatToResponses => json!({
            "messages":[{"role":"user","content":semantic_case["task"]["prompt"]}],
            "model":"public-model", "parallel_tool_calls":true, "stream":true,
            "tools": tools.iter().map(chat_tool).collect::<Vec<_>>()
        }),
        Direction::ResponsesToChat => json!({
            "input":semantic_case["task"]["prompt"], "model":"public-model",
            "parallel_tool_calls":true, "stream":true,
            "tools": tools.to_vec()
        }),
    };
    let upstream_body = build_parallel_stream(direction.upstream(), &reference_events);
    Scenario {
        id: if direction == Direction::ChatToResponses {
            "chat_to_responses.parallel_arguments"
        } else {
            "responses_to_chat.parallel_arguments"
        },
        direction,
        kind: ScenarioKind::ParallelArguments,
        stream: true,
        client_request: json_bytes(request),
        upstream_body,
        semantic_case,
        reference_events,
    }
}

fn build_structured(direction: Direction) -> Scenario {
    let semantic_case = load_semantic_json("structured.strict_nested_json", "case.json");
    let reference_events = load_reference_events("structured.strict_nested_json");
    let format = semantic_case["task"]["response_format"].clone();
    let request = match direction {
        Direction::ChatToResponses => json!({
            "messages":[{"role":"user","content":semantic_case["task"]["prompt"]}],
            "model":"public-model", "stream":false,
            "response_format":{"type":"json_schema", "json_schema":{
                "name":format["name"], "schema":format["schema"], "strict":format["strict"]
            }}
        }),
        Direction::ResponsesToChat => json!({
            "input":semantic_case["task"]["prompt"], "model":"public-model", "stream":false,
            "text":{"format":{"type":"json_schema", "name":format["name"],
                "schema":format["schema"], "strict":format["strict"]}}
        }),
    };
    let upstream_text = expect_message(&reference_events, 0);
    let upstream_body = build_json_response(direction.upstream(), &upstream_text, &[]);
    Scenario {
        id: "chat_to_responses.structured_output",
        direction,
        kind: ScenarioKind::StructuredOutput,
        stream: false,
        client_request: json_bytes(request),
        upstream_body,
        semantic_case,
        reference_events,
    }
}

async fn run_scenario(scenario: Scenario) -> SemanticObservation {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let upstream_sse = scenario.stream;
    let state = MockState {
        response: scenario.upstream_body.clone(),
        content_type: if upstream_sse {
            "text/event-stream"
        } else {
            "application/json"
        },
        requests: requests.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("semantic upstream must bind loopback");
    let upstream_address = listener
        .local_addr()
        .expect("semantic upstream address must exist");
    let upstream_task = process_replay::spawn_server(
        listener,
        Router::new()
            .route("/v1/chat/completions", post(mock_respond))
            .route("/v1/responses", post(mock_respond))
            .with_state(state),
    );
    let mut definition = catalog_replay::replay_definition();
    configure_route(&mut definition, scenario.direction);
    configure_structured_capability(&mut definition);
    let gateway = process_replay::start_gateway_with_definition(upstream_address, definition).await;
    let endpoint = match scenario.direction.downstream() {
        ApiProtocol::ChatCompletions => "/v1/chat/completions",
        ApiProtocol::Responses => "/v1/responses",
    };

    let result = async {
        let response = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .expect("semantic client must build")
            .post(format!("http://{}{}", gateway.address, endpoint))
            .header(CONTENT_TYPE, "application/json")
            .bearer_auth(DOWNSTREAM_TOKEN)
            .body(scenario.client_request.clone())
            .send()
            .await
            .map_err(|error| format!("downstream request failed: {error}"))?;
        let status = response.status();
        let expected_content_type = if scenario.stream {
            "text/event-stream"
        } else {
            "application/json"
        };
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| format!("{} missing response content type", scenario.id))?;
        if content_type != expected_content_type {
            return Err(format!(
                "{} returned content type {content_type:?}, expected {expected_content_type:?}",
                scenario.id
            ));
        }
        let body = collect_bounded(response)
            .await
            .map_err(|error| format!("downstream body failed: {error}"))?;
        if status != StatusCode::OK {
            return Err(format!(
                "{} returned {status}: {}",
                scenario.id,
                String::from_utf8_lossy(&body)
            ));
        }
        let captured = requests.lock().await;
        if captured.len() != 1 {
            return Err(format!("{} expected one upstream attempt", scenario.id));
        }
        let request_value = captured[0].clone();
        let request = project_request(&request_value, scenario.direction.upstream())?;
        let response = normalize_response(&body, scenario.direction.downstream(), scenario.stream)?;
        // Exercise the independent parser before projection: removing the wire terminal must fail.
        if scenario.stream {
            let text = std::str::from_utf8(&body).map_err(|_| "synthetic SSE UTF-8")?;
            let without_terminal = text
                .split("\n\n")
                .filter(|block| !block.contains("[DONE]") && !block.contains("response.completed"))
                .collect::<Vec<_>>()
                .join("\n\n");
            if normalize_response(
                without_terminal.as_bytes(),
                scenario.direction.downstream(),
                true,
            )
            .is_ok_and(|projection| projection.terminal_count == 1)
            {
                return Err("independent parser accepted a missing wire terminal".to_owned());
            }
        }
        Ok::<_, String>((request, response))
    }
    .await;

    gateway.task.abort();
    upstream_task.abort();
    let _ = gateway.task.await;
    let _ = upstream_task.await;
    let (request, response) = result.unwrap_or_else(|error| panic!("{}: {error}", scenario.id));

    let expected_request_events = if scenario.kind == ScenarioKind::ToolHistory {
        scenario.reference_events[..2].to_vec()
    } else {
        Vec::new()
    };
    let expected_response_events = scenario
        .reference_events
        .iter()
        .filter(|event| match scenario.kind {
            ScenarioKind::ToolHistory | ScenarioKind::StructuredOutput => {
                matches!(event, SemanticEvent::AssistantMessage { .. })
            }
            ScenarioKind::ParallelArguments => matches!(event, SemanticEvent::ToolCall { .. }),
        })
        .cloned()
        .collect();
    let expected_tools = semantic_tools(&scenario.semantic_case);
    let expected_schema = (scenario.kind == ScenarioKind::StructuredOutput)
        .then(|| scenario.semantic_case["task"]["response_format"].clone());
    SemanticObservation {
        id: scenario.id,
        direction: scenario.direction,
        kind: scenario.kind,
        stream: scenario.stream,
        request,
        response,
        expected_request_events,
        expected_response_events,
        expected_tools,
        expected_schema,
    }
}

async fn mock_respond(State(state): State<MockState>, headers: HeaderMap, body: Bytes) -> Response {
    if headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return Response::builder()
            .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
            .body(Body::from("synthetic request content type mismatch"))
            .expect("content type response must build");
    }
    if body.len() > MAX_BODY {
        return Response::builder()
            .status(StatusCode::PAYLOAD_TOO_LARGE)
            .body(Body::from("synthetic request exceeded capture bound"))
            .expect("bounded mock response must build");
    }
    let value = serde_json::from_slice(&body).expect("semantic upstream request must be JSON");
    state.requests.lock().await.push(value);
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, state.content_type)
        .body(Body::from(state.response.clone()))
        .expect("semantic upstream response must build")
}

async fn collect_bounded(response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    loop {
        let next = tokio::time::timeout(HTTP_TIMEOUT, stream.next())
            .await
            .map_err(|_| "bounded body timeout".to_owned())?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|error| error.to_string())?;
        if body.len().saturating_add(chunk.len()) > MAX_BODY {
            return Err(format!("body exceeded {MAX_BODY} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn configure_route(definition: &mut openbridge::registry::RegistryConfig, direction: Direction) {
    let downstream = direction.downstream().operation();
    let route = definition.public_models[0]
        .routes
        .iter_mut()
        .find(|route| route.downstream_operation == downstream)
        .expect("semantic route must exist");
    route.upstream_operation = direction.upstream().operation();
}

fn configure_structured_capability(definition: &mut openbridge::registry::RegistryConfig) {
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::StrictSupported,
                ));
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::StrictSupported,
                ));
            }
            UpstreamApiCapabilities::Embeddings(_)
            | UpstreamApiCapabilities::ImagesGenerations(_) => {}
        }
    }
    let parameters =
        &mut crate::support::generation_profile_mut(&mut definition.models[0]).supported_parameters;
    if !parameters.iter().any(|value| value == "response_format") {
        parameters.push("response_format".to_owned());
    }
}

fn project_request(value: &Value, protocol: ApiProtocol) -> Result<RequestProjection, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "upstream request must be an object".to_owned())?;
    let tools = project_tools(object, protocol)?;
    let events = match protocol {
        ApiProtocol::ChatCompletions => project_chat_input(object)?,
        ApiProtocol::Responses => project_responses_input(object)?,
    };
    let structured_schema = match protocol {
        ApiProtocol::ChatCompletions => object
            .get("response_format")
            .map(|format| format["json_schema"].clone())
            .filter(|value| !value.is_null())
            .map(normalize_structured_format)
            .transpose()?,
        ApiProtocol::Responses => object
            .get("text")
            .map(|text| text["format"].clone())
            .filter(|value| !value.is_null())
            .map(normalize_structured_format)
            .transpose()?,
    };
    Ok(RequestProjection {
        protocol,
        tools,
        events,
        structured_schema,
    })
}

// Compare the complete semantic format while ignoring only the protocol wrapper's `type` field.
fn normalize_structured_format(value: Value) -> Result<Value, String> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| "structured format must be an object".to_owned())?;
    if object.get("type").is_some_and(|kind| kind != "json_schema") {
        return Err("unexpected structured format type".to_owned());
    }
    object.remove("type");
    Ok(Value::Object(object))
}

fn normalize_response(
    body: &[u8],
    protocol: ApiProtocol,
    stream: bool,
) -> Result<ResponseProjection, String> {
    if stream {
        normalize_sse(body, protocol)
    } else {
        let value: Value =
            serde_json::from_slice(body).map_err(|error| format!("response JSON: {error}"))?;
        normalize_json(value, protocol)
    }
}

fn normalize_json(value: Value, protocol: ApiProtocol) -> Result<ResponseProjection, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "response must be an object".to_owned())?;
    let events = match protocol {
        ApiProtocol::ChatCompletions => normalize_chat_json(object)?,
        ApiProtocol::Responses => normalize_responses_json(object)?,
    };
    Ok(ResponseProjection {
        protocol,
        events,
        terminal_count: 1,
    })
}

fn normalize_chat_json(object: &Map<String, Value>) -> Result<Vec<SemanticEvent>, String> {
    let choices = object["choices"]
        .as_array()
        .ok_or_else(|| "Chat response choices missing".to_owned())?;
    if choices.len() != 1 || choices[0]["finish_reason"] != "stop" {
        return Err("expected one completed Chat text choice".to_owned());
    }
    let choice = &choices[0];
    let choice_object = choice
        .as_object()
        .ok_or_else(|| "Chat choice must be object".to_owned())?;

    let message = choice_object["message"]
        .as_object()
        .ok_or_else(|| "Chat message missing".to_owned())?;

    let mut events = Vec::new();
    if let Some(calls) = message.get("tool_calls") {
        for call in calls
            .as_array()
            .ok_or_else(|| "Chat tool_calls must be array".to_owned())?
        {
            let call = call
                .as_object()
                .ok_or_else(|| "Chat tool call must be object".to_owned())?;

            let function = call["function"]
                .as_object()
                .ok_or_else(|| "Chat function missing".to_owned())?;

            events.push(SemanticEvent::ToolCall {
                call_id: string_field(call, "id")?,
                name: string_field(function, "name")?,
                arguments: parse_json_string(function, "arguments")?,
            });
        }
    }
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        events.push(SemanticEvent::AssistantMessage {
            text: content.to_owned(),
        });
    }
    Ok(events)
}

fn normalize_responses_json(object: &Map<String, Value>) -> Result<Vec<SemanticEvent>, String> {
    if object.get("status").and_then(Value::as_str) != Some("completed") {
        return Err("Responses JSON did not complete".to_owned());
    }
    let output = object["output"]
        .as_array()
        .ok_or_else(|| "Responses output missing".to_owned())?;
    let mut events = Vec::new();
    for item in output {
        let item = item
            .as_object()
            .ok_or_else(|| "Responses output item must be object".to_owned())?;
        let item_type = string_field(item, "type")?;
        match item_type.as_str() {
            "function_call" => {
                events.push(SemanticEvent::ToolCall {
                    call_id: string_field(item, "call_id")?,
                    name: string_field(item, "name")?,
                    arguments: parse_json_string(item, "arguments")?,
                });
            }
            "message" => {
                let content = item["content"]
                    .as_array()
                    .ok_or_else(|| "Responses message content missing".to_owned())?;
                for part in content {
                    let part = part
                        .as_object()
                        .ok_or_else(|| "Responses content part must be object".to_owned())?;

                    if part["type"] == "output_text" {
                        events.push(SemanticEvent::AssistantMessage {
                            text: string_field(part, "text")?,
                        });
                    }
                }
            }
            other => return Err(format!("unrecognized Responses output type {other}")),
        }
    }
    Ok(events)
}

/// Reads only the tool-only SSE subset emitted by these fixed synthetic scenarios.
fn normalize_sse(body: &[u8], protocol: ApiProtocol) -> Result<ResponseProjection, String> {
    let text = std::str::from_utf8(body).map_err(|error| format!("SSE UTF-8: {error}"))?;
    let mut events = Vec::new();
    let mut terminal_count = 0;
    for block in text.split("\n\n").filter(|block| !block.trim().is_empty()) {
        let mut event_name = None;
        let mut data = None;
        for line in block.lines() {
            if let Some(value) = line.strip_prefix("event: ") {
                event_name = Some(value);
            } else if let Some(value) = line.strip_prefix("data: ") {
                data = Some(value);
            } else if !line.is_empty() {
                return Err(format!("unrecognized SSE line {line}"));
            }
        }
        let data = data.ok_or_else(|| "SSE event missing data".to_owned())?;
        if terminal_count != 0 {
            return Err("event after terminal".to_owned());
        }
        if protocol == ApiProtocol::ChatCompletions && data == "[DONE]" {
            terminal_count += 1;
            continue;
        }
        let value: Value =
            serde_json::from_str(data).map_err(|error| format!("SSE data JSON: {error}"))?;
        match protocol {
            ApiProtocol::ChatCompletions => {
                let choices = value["choices"]
                    .as_array()
                    .ok_or_else(|| "Chat SSE choices missing".to_owned())?;
                let choice = choices
                    .first()
                    .ok_or_else(|| "Chat SSE choice missing".to_owned())?;
                let delta = choice["delta"]
                    .as_object()
                    .ok_or_else(|| "Chat SSE delta missing".to_owned())?;
                if let Some(calls) = delta.get("tool_calls") {
                    for call in calls
                        .as_array()
                        .ok_or_else(|| "Chat SSE tools missing".to_owned())?
                    {
                        let call = call
                            .as_object()
                            .ok_or_else(|| "Chat SSE tool must be object".to_owned())?;
                        let function = call
                            .get("function")
                            .and_then(Value::as_object)
                            .ok_or_else(|| "Chat SSE function missing".to_owned())?;
                        let index = call["index"]
                            .as_u64()
                            .ok_or_else(|| "Chat SSE tool index missing".to_owned())?
                            as usize;
                        if let (Some(id), Some(name)) = (
                            call.get("id").and_then(Value::as_str),
                            function.get("name").and_then(Value::as_str),
                        ) {
                            if index != events.len() {
                                return Err(
                                    "Chat SSE call index does not match its identity".to_owned()
                                );
                            }
                            events.push(SemanticEvent::ToolCall {
                                call_id: id.to_owned(),
                                name: name.to_owned(),
                                arguments: Value::String(
                                    function
                                        .get("arguments")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_owned(),
                                ),
                            });
                        } else if let Some(fragment) =
                            function.get("arguments").and_then(Value::as_str)
                        {
                            append_tool_fragment(&mut events, index, fragment)?;
                        }
                    }
                }
                // The finish marker precedes [DONE]; it is not a second terminal.
            }
            ApiProtocol::Responses => {
                let event =
                    event_name.ok_or_else(|| "Responses SSE event name missing".to_owned())?;
                match event {
                    "response.output_item.added" => {
                        let item = value["item"]
                            .as_object()
                            .ok_or_else(|| "Responses SSE item missing".to_owned())?;
                        if item["type"] != "function_call" {
                            return Err("unrecognized Responses SSE item".to_owned());
                        }
                        let index = value["output_index"]
                            .as_u64()
                            .ok_or_else(|| "Responses SSE output index missing".to_owned())?
                            as usize;
                        if index != events.len() {
                            return Err(format!(
                                "Responses SSE output index {index} is not the next call"
                            ));
                        }
                        events.push(SemanticEvent::ToolCall {
                            call_id: string_field(item, "call_id")?,
                            name: string_field(item, "name")?,
                            arguments: Value::String(String::new()),
                        });
                    }
                    "response.function_call_arguments.delta" => {
                        let index = value["output_index"]
                            .as_u64()
                            .ok_or_else(|| "Responses SSE index missing".to_owned())?
                            as usize;
                        let fragment = string_field(value.as_object().unwrap(), "delta")?;
                        append_tool_fragment(&mut events, index, &fragment)?;
                    }
                    "response.completed" => terminal_count += 1,
                    "response.created"
                    | "response.function_call_arguments.done"
                    | "response.output_item.done" => {}
                    other => return Err(format!("unrecognized Responses SSE event {other}")),
                }
            }
        }
    }
    for event in &mut events {
        if let SemanticEvent::ToolCall { arguments, .. } = event {
            *arguments = serde_json::from_str(
                arguments
                    .as_str()
                    .ok_or("argument fragments must be text")?,
            )
            .map_err(|_| "incomplete argument JSON".to_owned())?;
        }
    }
    Ok(ResponseProjection {
        protocol,
        events,
        terminal_count,
    })
}

fn append_tool_fragment(
    events: &mut [SemanticEvent],
    index: usize,
    fragment: &str,
) -> Result<(), String> {
    match events.get_mut(index) {
        Some(SemanticEvent::ToolCall {
            arguments: Value::String(arguments),
            ..
        }) => {
            arguments.push_str(fragment);
            Ok(())
        }
        Some(SemanticEvent::ToolCall { .. }) => {
            Err(format!("tool index {index} is already complete"))
        }
        Some(_) => Err(format!("tool index {index} is not a call")),
        None => Err(format!("fragment references unknown tool index {index}")),
    }
}

fn project_tools(
    object: &Map<String, Value>,
    protocol: ApiProtocol,
) -> Result<Vec<(String, Value, bool)>, String> {
    let Some(tools) = object.get("tools") else {
        return Ok(Vec::new());
    };
    tools
        .as_array()
        .ok_or_else(|| "upstream tools must be array".to_owned())?
        .iter()
        .map(|tool| {
            let tool = tool
                .as_object()
                .ok_or_else(|| "tool must be object".to_owned())?;
            let function = match protocol {
                ApiProtocol::ChatCompletions => tool
                    .get("function")
                    .and_then(Value::as_object)
                    .ok_or_else(|| "Chat function tool missing".to_owned())?,
                ApiProtocol::Responses => tool,
            };
            let name = string_field(function, "name")?;
            let schema = function
                .get("parameters")
                .cloned()
                .ok_or_else(|| "tool parameters missing".to_owned())?;
            let strict = function
                .get("strict")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok((name, schema, strict))
        })
        .collect()
}

fn project_chat_input(object: &Map<String, Value>) -> Result<Vec<SemanticEvent>, String> {
    let messages = object["messages"]
        .as_array()
        .ok_or_else(|| "Chat messages missing".to_owned())?;
    let mut events = Vec::new();
    for message in messages {
        let message = message
            .as_object()
            .ok_or_else(|| "Chat message must be object".to_owned())?;
        match message.get("role").and_then(Value::as_str) {
            Some("assistant") => {
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        let call = call
                            .as_object()
                            .ok_or_else(|| "Chat input tool call invalid".to_owned())?;
                        let function = call["function"]
                            .as_object()
                            .ok_or_else(|| "Chat input function missing".to_owned())?;
                        events.push(SemanticEvent::ToolCall {
                            call_id: string_field(call, "id")?,
                            name: string_field(function, "name")?,
                            arguments: parse_json_string(function, "arguments")?,
                        });
                    }
                }
            }
            Some("tool") => events.push(SemanticEvent::ToolResult {
                call_id: string_field(message, "tool_call_id")?,
                output: parse_string_or_json(message, "content")?,
            }),
            _ => {}
        }
    }
    Ok(events)
}

fn project_responses_input(object: &Map<String, Value>) -> Result<Vec<SemanticEvent>, String> {
    let Some(input) = object.get("input") else {
        return Ok(Vec::new());
    };
    let items = match input {
        Value::Array(items) => items,
        Value::String(_) => return Ok(Vec::new()),
        _ => return Err("Responses input invalid".to_owned()),
    };
    let mut events = Vec::new();
    for item in items {
        let item = item
            .as_object()
            .ok_or_else(|| "Responses input item invalid".to_owned())?;
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => events.push(SemanticEvent::ToolCall {
                call_id: string_field(item, "call_id")?,
                name: string_field(item, "name")?,
                arguments: parse_json_string(item, "arguments")?,
            }),
            Some("function_call_output") => events.push(SemanticEvent::ToolResult {
                call_id: string_field(item, "call_id")?,
                output: parse_string_or_json(item, "output")?,
            }),
            _ => {}
        }
    }
    Ok(events)
}

fn build_json_response(protocol: ApiProtocol, text: &str, calls: &[SemanticEvent]) -> Bytes {
    let output = if calls.is_empty() {
        json!([{"id":"message_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":text,"annotations":[]}] }])
    } else {
        json!([])
    };
    let value = match protocol {
        ApiProtocol::Responses => {
            json!({"id":"response_1","object":"response","model":"upstream-model","status":"completed","output":output})
        }
        ApiProtocol::ChatCompletions => {
            json!({"id":"chatcmpl_1","object":"chat.completion","model":"upstream-model","created":0,"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":text}}]})
        }
    };
    json_bytes(value)
}

// This is a fixed synthetic fixture for the selected contract, not a general SSE parser.
fn build_parallel_stream(protocol: ApiProtocol, events: &[SemanticEvent]) -> Bytes {
    let calls: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            SemanticEvent::ToolCall {
                call_id,
                name,
                arguments,
            } => Some((call_id.clone(), name.clone(), compact(arguments))),
            _ => None,
        })
        .collect();
    let mut body = String::new();
    match protocol {
        ApiProtocol::Responses => {
            body.push_str("event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"response_parallel\",\"object\":\"response\",\"model\":\"upstream-model\",\"status\":\"in_progress\",\"output\":[]}}\n\n");
            for (index, (call_id, name, _)) in calls.iter().enumerate() {
                body.push_str(&sse(Some("response.output_item.added"), json!({"type":"response.output_item.added","output_index":index,"item":{"id":format!("item_{index}"),"type":"function_call","call_id":call_id,"name":name,"arguments":"","status":"in_progress"}})));
            }
            for (index, (_, _, args)) in calls.iter().enumerate() {
                let fragments = split_arguments(args);
                body.push_str(&sse(Some("response.function_call_arguments.delta"), json!({"type":"response.function_call_arguments.delta","item_id":format!("item_{index}"),"output_index":index,"delta":fragments.0})));
            }
            for (index, (_, _, args)) in calls.iter().enumerate() {
                let fragments = split_arguments(args);
                body.push_str(&sse(Some("response.function_call_arguments.delta"), json!({"type":"response.function_call_arguments.delta","item_id":format!("item_{index}"),"output_index":index,"delta":fragments.1})));
            }
            for (index, (call_id, name, args)) in calls.iter().enumerate() {
                body.push_str(&sse(Some("response.function_call_arguments.done"), json!({"type":"response.function_call_arguments.done","item_id":format!("item_{index}"),"output_index":index,"arguments":args})));
                body.push_str(&sse(Some("response.output_item.done"), json!({"type":"response.output_item.done","output_index":index,"item":{"id":format!("item_{index}"),"type":"function_call","call_id":call_id,"name":name,"arguments":args,"status":"completed"}})));
            }
            let output: Vec<_> = calls.iter().enumerate().map(|(index,(call_id,name,args))| json!({"id":format!("item_{index}"),"type":"function_call","call_id":call_id,"name":name,"arguments":args,"status":"completed"})).collect();
            body.push_str(&sse(Some("response.completed"), json!({"type":"response.completed","response":{"id":"response_parallel","object":"response","model":"upstream-model","status":"completed","output":output}})));
        }
        ApiProtocol::ChatCompletions => {
            let tool_calls: Vec<_> = calls.iter().enumerate().map(|(index,(call_id,name,args))| { let fragments=split_arguments(args); json!({"index":index,"id":call_id,"type":"function","function":{"name":name,"arguments":fragments.0}}) }).collect();
            body.push_str(&sse(None, json!({"id":"chatcmpl_parallel","object":"chat.completion.chunk","model":"upstream-model","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":tool_calls},"finish_reason":null}]})));
            for (index, (_, _, args)) in calls.iter().enumerate() {
                let fragments = split_arguments(args);
                let _ = fragments.0;
                body.push_str(&sse(None, json!({"id":"chatcmpl_parallel","object":"chat.completion.chunk","model":"upstream-model","choices":[{"index":0,"delta":{"tool_calls":[{"index":index,"function":{"arguments":split_arguments(args).1}}]},"finish_reason":null}]})));
            }
            body.push_str(&sse(None, json!({"id":"chatcmpl_parallel","object":"chat.completion.chunk","model":"upstream-model","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]})));
            body.push_str("data: [DONE]\n\n");
        }
    }
    Bytes::from(body)
}

fn sse(event: Option<&str>, value: Value) -> String {
    match event {
        Some(event) => format!("event: {event}\ndata: {}\n\n", value),
        None => format!("data: {}\n\n", value),
    }
}

fn split_arguments(arguments: &str) -> (&str, &str) {
    let split = arguments
        .find(',')
        .map(|index| index + 1)
        .unwrap_or(arguments.len() / 2);
    arguments.split_at(split)
}

fn semantic_tools(case: &Value) -> Vec<(String, Value, bool)> {
    case["task"]["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .map(|tool| {
                    (
                        tool["name"].as_str().expect("tool name").to_owned(),
                        tool["parameters"].clone(),
                        tool["strict"].as_bool().expect("tool strict flag"),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn chat_tool(tool: &Value) -> Value {
    json!({
        "type":"function",
        "function": {
            "name":tool["name"],
            "description":tool["description"],
            "parameters":tool["parameters"],
            "strict":tool["strict"]
        }
    })
}
fn json_bytes(value: Value) -> Bytes {
    Bytes::from(serde_json::to_vec(&value).expect("synthetic JSON must serialize"))
}
fn compact(value: &Value) -> String {
    serde_json::to_string(value).expect("synthetic value must serialize")
}
fn arguments(event: &SemanticEvent) -> &Value {
    match event {
        SemanticEvent::ToolCall { arguments, .. } => arguments,
        _ => panic!("expected tool call"),
    }
}
fn output(event: &SemanticEvent) -> &Value {
    match event {
        SemanticEvent::ToolResult { output, .. } => output,
        _ => panic!("expected tool result"),
    }
}
fn call_id(event: &SemanticEvent) -> &str {
    match event {
        SemanticEvent::ToolCall { call_id, .. } => call_id,
        _ => panic!("expected tool call"),
    }
}
fn result_id(event: &SemanticEvent) -> &str {
    match event {
        SemanticEvent::ToolResult { call_id, .. } => call_id,
        _ => panic!("expected tool result"),
    }
}
fn call_name(event: &SemanticEvent) -> &str {
    match event {
        SemanticEvent::ToolCall { name, .. } => name,
        _ => panic!("expected tool call"),
    }
}
fn expect_call(events: &[SemanticEvent], index: usize) -> &SemanticEvent {
    events.get(index).expect("reference call must exist")
}
fn expect_result(events: &[SemanticEvent], index: usize) -> &SemanticEvent {
    events.get(index).expect("reference result must exist")
}
fn expect_message(events: &[SemanticEvent], index: usize) -> String {
    match events.get(index).expect("reference message must exist") {
        SemanticEvent::AssistantMessage { text } => text.clone(),
        _ => panic!("reference event must be message"),
    }
}

fn load_semantic_json(case_id: &str, file: &str) -> Value {
    let group = case_id.split('.').next().expect("semantic case group");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/semantic-cases")
        .join(group)
        .join(case_id)
        .join(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert!(bytes.len() <= MAX_BODY, "semantic fixture is bounded");
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn parse_trace(value: &Value) -> Vec<SemanticEvent> {
    value["events"]
        .as_array()
        .expect("reference events must be an array")
        .iter()
        .map(|event| {
            let object = event.as_object().expect("reference event must be object");
            match object["type"].as_str().expect("reference event type") {
                "assistant_tool_call" => SemanticEvent::ToolCall {
                    call_id: string_field(object, "call_id").expect("call id"),
                    name: string_field(object, "name").expect("call name"),
                    arguments: object["arguments"].clone(),
                },
                "tool_result" => SemanticEvent::ToolResult {
                    call_id: string_field(object, "call_id").expect("result call id"),
                    output: object["output"].clone(),
                },
                "assistant_message" => SemanticEvent::AssistantMessage {
                    text: string_field(object, "text").expect("message text"),
                },
                other => panic!("unrecognized semantic event {other}"),
            }
        })
        .collect()
}

fn string_field(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} must be a string"))
}
fn parse_json_string(object: &Map<String, Value>, key: &str) -> Result<Value, String> {
    let text = string_field(object, key)?;
    serde_json::from_str(&text).map_err(|error| format!("{key} JSON: {error}"))
}
fn parse_string_or_json(object: &Map<String, Value>, key: &str) -> Result<Value, String> {
    let text = string_field(object, key)?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}
