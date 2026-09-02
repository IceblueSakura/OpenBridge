//! Unit tests for trusted basic-probe egress, fixed protocol requests, and reports.

use std::{
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::body::Body;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::future::BoxFuture;
use http::{
    HeaderMap, HeaderValue, Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use secrecy::SecretString;
use serde_json::{Value, json};

use super::{
    ProbeError, ProbeOptions, ProbeStatus, probe_upstream_target, probe_upstream_target_with_oauth2,
};
use crate::{
    config::parse_bootstrap_config,
    credential::{CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder},
    oauth2_credentials::OAuth2CredentialManagerBuilder,
    provider::{PreparedUpstreamRequest, ProviderKind},
    providers,
    registry::{RuntimeRegistry, UpstreamTarget, build_registry},
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};

const BOOTSTRAP: &str = r#"
schema_version = 3
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
upstream_credentials_file = "config/upstream-credentials.toml"
default_instructions = "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
max_request_body = "1MiB"
max_json_response_body = "16MiB"
max_replay_body = "256KiB"
max_sse_event = "256KiB"
upstream_connect_timeout = "5s"
upstream_pool_idle_timeout = "90s"
upstream_pool_max_idle_per_host = 16
"#;

fn registry() -> RuntimeRegistry {
    let mut definition = providers::compiled_config();
    definition.version = "probe-test".to_owned();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "openai-main")
        .unwrap();
    for upstream_api in &mut target.upstream_apis {
        upstream_api.upstream_model = "test-model".to_owned();
    }
    build_registry(parse_bootstrap_config(BOOTSTRAP).unwrap(), definition).unwrap()
}

fn registry_with_response_limit(max_response_bytes: usize) -> RuntimeRegistry {
    // Override only the probe response budget in the standard bootstrap fixture.
    let bootstrap = BOOTSTRAP.replace(
        "max_json_response_body = \"16MiB\"",
        &format!("max_json_response_body = \"{max_response_bytes}B\""),
    );

    // Compile the ordinary provider catalog with stable model names for probe assertions.
    let mut definition = providers::compiled_config();
    definition.version = "probe-limit-test".to_owned();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "openai-main")
        .unwrap();
    for upstream_api in &mut target.upstream_apis {
        upstream_api.upstream_model = "test-model".to_owned();
    }
    build_registry(parse_bootstrap_config(&bootstrap).unwrap(), definition).unwrap()
}

fn credentials(registry: &RuntimeRegistry) -> CredentialStore {
    credentials_for_target(registry, "openai-main")
}

fn credentials_for_target(registry: &RuntimeRegistry, target_id: &str) -> CredentialStore {
    // Resolve the target's compile-time pool identity and credential kind.
    let target = registry.upstream_target(target_id).unwrap();
    let pool = registry
        .credential_pool(target.credential_pool_id())
        .unwrap();
    let mut credentials = CredentialStoreBuilder::new();

    // Populate two ordered synthetic members through the production builder boundary.
    for (index, secret) in ["test-key", "unused-test-key"].into_iter().enumerate() {
        credentials
            .insert_upstream_member(
                target.kind(),
                pool.id(),
                format!("{}#{}", pool.id(), index + 1),
                SecretString::from(secret),
                CredentialMetadata::upstream(pool.kind(), CredentialSource::Programmatic),
            )
            .unwrap();
    }

    // Freeze the test-only credentials into the immutable runtime snapshot.
    credentials.build()
}

#[derive(Default)]
struct FixtureTransport {
    requests: Mutex<Vec<(Method, String, Value)>>,
    authorizations: Mutex<Vec<String>>,
}

fn fixture_tool_calls(body: &Value) -> Option<Vec<(String, String)>> {
    let tools = body.get("tools")?.as_array()?;
    if body.get("tool_choice").and_then(Value::as_str) == Some("none") {
        return Some(Vec::new());
    }
    let count = if body.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true) {
        2
    } else {
        1
    };
    Some(
        tools
            .iter()
            .take(count)
            .filter_map(|tool| {
                let function = tool.get("function").unwrap_or(tool);
                let name = function.get("name")?.as_str()?.to_owned();
                let value = if name.ends_with("secondary") {
                    "secondary"
                } else {
                    "primary"
                };
                Some((name, json!({"value": value}).to_string()))
            })
            .collect(),
    )
}

fn fixture_tool_json(path: &str, body: &Value) -> Option<Value> {
    let calls = fixture_tool_calls(body)?;
    Some(match path {
        "/v1/chat/completions" => json!({
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "finish_reason": if calls.is_empty() { "stop" } else { "tool_calls" },
                "message": {
                    "role": "assistant",
                    "content": if calls.is_empty() { Value::String("OK".to_owned()) } else { Value::Null },
                    "tool_calls": calls.iter().enumerate().map(|(index, (name, arguments))| json!({
                        "index": index,
                        "id": format!("call_{index}"),
                        "type": "function",
                        "function": {"name": name, "arguments": arguments}
                    })).collect::<Vec<_>>()
                }
            }]
        }),
        "/v1/responses" => json!({
            "object": "response",
            "status": "completed",
            "output": if calls.is_empty() {
                vec![json!({
                    "type": "message",
                    "content": [{"type": "output_text", "text": "OK"}]
                })]
            } else {
                calls.iter().enumerate().map(|(index, (name, arguments))| json!({
                    "type": "function_call",
                    "id": format!("fc_{index}"),
                    "call_id": format!("call_{index}"),
                    "name": name,
                    "arguments": arguments
                })).collect::<Vec<_>>()
            }
        }),
        _ => return None,
    })
}

fn fixture_tool_sse(path: &str, body: &Value) -> Option<String> {
    let calls = fixture_tool_calls(body)?;
    match path {
        "/v1/chat/completions" => {
            let first = json!({
                "choices": [{
                    "index": 0,
                    "delta": if calls.is_empty() {
                        json!({"role": "assistant", "content": "OK"})
                    } else {
                        json!({"role": "assistant", "tool_calls": calls.iter().enumerate().map(|(index, (name, arguments))| json!({
                            "index": index,
                            "id": format!("call_{index}"),
                            "type": "function",
                            "function": {"name": name, "arguments": arguments}
                        })).collect::<Vec<_>>()})
                    },
                    "finish_reason": Value::Null
                }]
            });
            let terminal = json!({
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": if calls.is_empty() { "stop" } else { "tool_calls" }
                }]
            });
            Some(format!(
                "data: {first}\n\ndata: {terminal}\n\ndata: [DONE]\n\n"
            ))
        }
        "/v1/responses" => {
            let mut stream = String::new();
            for (index, (name, arguments)) in calls.iter().enumerate() {
                let event = json!({
                    "type": "response.output_item.added",
                    "output_index": index,
                    "item": {
                        "id": format!("fc_{index}"),
                        "type": "function_call",
                        "call_id": format!("call_{index}"),
                        "name": name,
                        "arguments": arguments
                    }
                });
                stream.push_str(&format!(
                    "event: response.output_item.added\ndata: {event}\n\n"
                ));
            }
            let terminal = json!({
                "type": "response.completed",
                "response": {"status": "completed", "output": []}
            });
            stream.push_str(&format!("event: response.completed\ndata: {terminal}\n\n"));
            Some(stream)
        }
        _ => None,
    }
}

impl UpstreamTransport for FixtureTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            let body = if request.body().is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(request.body()).unwrap()
            };
            self.requests.lock().unwrap().push((
                request.method().clone(),
                request.relative_uri().path().to_owned(),
                body.clone(),
            ));
            self.authorizations
                .lock()
                .unwrap()
                .push(headers[AUTHORIZATION].to_str().unwrap().to_owned());
            if body.get("tools").is_some() {
                if body.get("stream").and_then(Value::as_bool) == Some(true) {
                    if let Some(event_stream) =
                        fixture_tool_sse(request.relative_uri().path(), &body)
                    {
                        let mut response_headers = HeaderMap::new();
                        response_headers
                            .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
                        return Ok(UpstreamResponse::new(
                            StatusCode::OK,
                            response_headers,
                            Body::from(event_stream),
                        ));
                    }
                } else if let Some(response) =
                    fixture_tool_json(request.relative_uri().path(), &body)
                {
                    return Ok(UpstreamResponse::new(
                        StatusCode::OK,
                        HeaderMap::new(),
                        Body::from(response.to_string()),
                    ));
                }
            }
            if body.get("stream").and_then(Value::as_bool) == Some(true) {
                let event_stream = match request.relative_uri().path() {
                    "/v1/chat/completions" => Some(concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n",
                        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":4,\"completion_tokens_details\":{\"reasoning_tokens\":2},\"total_tokens\":14}}\n\n",
                        "data: [DONE]\n\n",
                    )),
                    "/v1/responses" => Some(concat!(
                        "event: response.reasoning_text.done\ndata: {\"type\":\"response.reasoning_text.done\"}\n\n",
                        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":10,\"output_tokens\":4,\"output_tokens_details\":{\"reasoning_tokens\":2},\"total_tokens\":14}}}\n\n",
                    )),
                    _ => None,
                };
                if let Some(event_stream) = event_stream {
                    let mut response_headers = HeaderMap::new();
                    response_headers
                        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
                    return Ok(UpstreamResponse::new(
                        StatusCode::OK,
                        response_headers,
                        Body::from(event_stream),
                    ));
                }
            }
            let response = match request.relative_uri().path() {
                "/v1/models" => {
                    json!({"object": "list", "data": [{"id": "test-model"}, {"id": "other-model"}]})
                }
                "/v1/chat/completions" => {
                    json!({
                        "object": "chat.completion",
                        "choices": [{"message": {
                            "role": "assistant",
                            "content": if body
                                .get("response_format")
                                .is_some_and(|format| {
                                    format.get("type").and_then(Value::as_str)
                                        == Some("json_object")
                                }) {
                                r#"{"probe":"ok"}"#
                            } else {
                                "OK"
                            },
                            "reasoning_content": "must-not-enter-report"
                        }}],
                        "usage": {
                            "prompt_tokens": 10,
                            "completion_tokens": 4,
                            "completion_tokens_details": {"reasoning_tokens": 2},
                            "total_tokens": 14
                        }
                    })
                }
                "/v1/responses" => json!({
                    "object": "response",
                    "status": "completed",
                    "output": [
                        {"type": "reasoning", "summary": []},
                        {"type": "message", "content": [{"type": "output_text", "text": if body.pointer("/text/format/type").and_then(Value::as_str) == Some("json_object") { r#"{"probe":"ok"}"# } else { "OK" }}]}
                    ],
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 4,
                        "output_tokens_details": {"reasoning_tokens": 2},
                        "total_tokens": 14
                    }
                }),
                "/v1/embeddings" => json!({
                    "object": "list",
                    "data": [{"object": "embedding", "embedding": [0.0], "index": 0}],
                    "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
                    "usage": {"prompt_tokens": 1, "total_tokens": 1}
                }),
                _ => {
                    return Ok(UpstreamResponse::new(
                        StatusCode::NOT_FOUND,
                        HeaderMap::new(),
                        Body::empty(),
                    ));
                }
            };
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                HeaderMap::new(),
                Body::from(response.to_string()),
            ))
        })
    }
}

static NEXT_SYNTHETIC_AUTH_FILE: AtomicUsize = AtomicUsize::new(1);

struct SyntheticAuthFile {
    path: PathBuf,
}

impl Drop for SyntheticAuthFile {
    fn drop(&mut self) {
        // Remove only the process-unique synthetic file created by this test.
        let _ = fs::remove_file(&self.path);
    }
}

fn synthetic_jwt(payload: Value) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(b"{}"),
        URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes()),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}

fn synthetic_chatgpt_auth_file() -> SyntheticAuthFile {
    // Create a non-expired account-bound bundle with no real credential material.
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3_600;
    let access_token = synthetic_jwt(json!({"exp": expires_at}));
    let id_token = synthetic_jwt(json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-account",
            "chatgpt_account_is_fedramp": false
        }
    }));
    let path = std::env::temp_dir().join(format!(
        "openbridge-probe-oauth-{}-{}.json",
        std::process::id(),
        NEXT_SYNTHETIC_AUTH_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let document = json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": id_token,
            "access_token": access_token,
            "refresh_token": "synthetic-refresh-token",
            "account_id": "synthetic-account"
        },
        "last_refresh": "2026-08-07T00:00:00Z"
    });
    fs::write(&path, document.to_string()).unwrap();
    SyntheticAuthFile { path }
}

#[derive(Default)]
struct ChatGptModelListTransport {
    requests: Mutex<Vec<String>>,
    authorizations: Mutex<Vec<String>>,
    accounts: Mutex<Vec<String>>,
}

impl UpstreamTransport for ChatGptModelListTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            // Record only the fixed request shape and redacted test-only header observations.
            let relative_uri = request.relative_uri().to_string();
            self.requests.lock().unwrap().push(relative_uri.clone());
            self.authorizations.lock().unwrap().push(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned(),
            );
            self.accounts.lock().unwrap().push(
                headers
                    .get(http::header::HeaderName::from_static("chatgpt-account-id"))
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned(),
            );

            // Return the ChatGPT manifest envelope only for the registered fixed endpoint.
            if request.method() != Method::GET || relative_uri != "/models?client_version=0.146.0" {
                return Ok(UpstreamResponse::new(
                    StatusCode::NOT_FOUND,
                    HeaderMap::new(),
                    Body::empty(),
                ));
            }
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                HeaderMap::new(),
                Body::from(
                    json!({
                        "models": [{"slug": "gpt-5.6-sol"}]
                    })
                    .to_string(),
                ),
            ))
        })
    }
}

#[derive(Default)]
struct ChatGptResponsesTransport {
    requests: Mutex<Vec<(String, Value)>>,
}

impl UpstreamTransport for ChatGptResponsesTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            // Record the fixed streaming request without retaining any credential header.
            let body: Value = serde_json::from_slice(request.body()).unwrap();
            self.requests
                .lock()
                .unwrap()
                .push((request.relative_uri().to_string(), body));

            // Return one ChatGPT-style data-discriminated terminal event.
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                headers,
                Body::from("data: {\"type\":\"response.completed\"}\n\n"),
            ))
        })
    }
}

struct StaticTransport {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
    fails: bool,
    requests: AtomicUsize,
}

impl StaticTransport {
    fn response(status: StatusCode, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: body.into(),
            fails: false,
            requests: AtomicUsize::new(0),
        }
    }

    fn event_stream(body: impl Into<Vec<u8>>) -> Self {
        let mut transport = Self::response(StatusCode::OK, body);
        transport
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        transport
    }

    fn failure() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            headers: HeaderMap::new(),
            body: Vec::new(),
            fails: true,
            requests: AtomicUsize::new(0),
        }
    }
}

impl UpstreamTransport for StaticTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            // Record every attempted exchange before applying the scripted outcome.
            self.requests.fetch_add(1, Ordering::Relaxed);
            if self.fails {
                return Err(TransportError::Timeout);
            }

            // Return the fixed status and body without consulting a network endpoint.
            Ok(UpstreamResponse::new(
                self.status,
                self.headers.clone(),
                Body::from(self.body.clone()),
            ))
        })
    }
}

#[tokio::test]
async fn probe_discovers_models_and_smokes_one_generation_case_without_tool_payloads() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            embeddings: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    let serialized_report = serde_json::to_value(&report).unwrap();
    let list_models = report.list_models.as_ref().unwrap();
    assert_eq!(list_models.outcome.state, ProbeStatus::Accepted);
    assert_eq!(list_models.configured_model_listed, Some(true));
    assert_eq!(list_models.model_ids, ["test-model", "other-model"]);
    assert_eq!(
        report.generation.as_ref().unwrap().outcome.state,
        ProbeStatus::Accepted
    );
    assert_eq!(
        report.embeddings.as_ref().unwrap().state,
        ProbeStatus::Unsupported
    );

    // Keep the serialized report limited to discovery and basic operation observations.
    assert!(serialized_report.get("chat_function_calling").is_none());
    assert!(
        serialized_report
            .get("responses_function_calling")
            .is_none()
    );

    let requests = transport.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|(method, path, _)| method == Method::GET && path == "/v1/models")
    );
    assert!(
        requests
            .iter()
            .filter_map(|(_, path, body)| (path != "/v1/models").then_some(body))
            .all(|body| body.get("model").and_then(Value::as_str) == Some("test-model"))
    );
    let chat = requests
        .iter()
        .find(|(_, path, _)| path == "/v1/chat/completions")
        .map(|(_, _, body)| body)
        .unwrap();
    assert_eq!(
        chat["messages"][0],
        serde_json::json!({
            "role": "system",
            "content": "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
        })
    );

    assert!(requests.iter().all(|(_, _, body)| {
        body.get("tools").is_none()
            && body.get("tool_choice").is_none()
            && !body.to_string().contains("function_call_output")
            && !body.to_string().contains("\"role\":\"tool\"")
    }));
    assert!(
        transport
            .authorizations
            .lock()
            .unwrap()
            .iter()
            .all(|authorization| authorization == "Bearer test-key")
    );
}

#[tokio::test]
async fn model_list_report_caps_ids_without_losing_candidate_correlation() {
    let registry = registry();
    let credential_store = credentials(&registry);
    let mut models = (0..1_025)
        .map(|index| json!({"id": format!("model-{index}")}))
        .collect::<Vec<_>>();
    models.push(json!({"id": "candidate-outside-sample"}));
    let transport = StaticTransport::response(
        StatusCode::OK,
        serde_json::to_vec(&json!({"object": "list", "data": models})).unwrap(),
    );

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credential_store,
        ProbeOptions {
            list_models: true,
            upstream_model: Some("candidate-outside-sample".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();
    let models = report.list_models.unwrap();

    assert_eq!(models.model_id_count, Some(1_026));
    assert_eq!(models.model_ids.len(), 1_024);
    assert!(models.model_ids_truncated);
    assert_eq!(models.requested_model_listed, Some(true));
    assert!(
        !models
            .model_ids
            .contains(&"candidate-outside-sample".to_owned())
    );
}

#[tokio::test]
async fn probe_executes_exactly_one_custom_model_generation_case() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    // Select one explicit Responses streaming reasoning case.
    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::Responses,
                mode: super::ProbeGenerationMode::Streaming,
                case: super::ProbeGenerationCase::ReasoningHigh,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    // Report only the selected unit and preserve bounded evidence.
    assert_eq!(report.requested_model.as_deref(), Some("candidate-model"));
    let generation = report.generation.as_ref().unwrap();
    assert_eq!(generation.case, super::ProbeGenerationCase::ReasoningHigh);
    assert_eq!(
        generation.upstream_model.as_deref(),
        Some("candidate-model")
    );
    assert_eq!(generation.outcome.state, super::ProbeStatus::Accepted);
    assert!(
        generation
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.usage.as_ref())
            .is_some_and(|usage| usage.reasoning_tokens == Some(2))
    );
    let model_list = report.list_models.as_ref().unwrap();
    assert_eq!(model_list.requested_model_listed, Some(false));
    let serialized_report = serde_json::to_string(&report).unwrap();
    assert!(!serialized_report.contains("test-key"));
    assert!(!serialized_report.contains("Reply with exactly OK."));
    assert!(!serialized_report.contains("must-not-enter-report"));
    assert!(serialized_report.contains("\"elapsed_ms\":"));

    // The one case retains the fixed Provider path while overriding only the model field.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let (_, path, body) = requests
        .iter()
        .find(|(_, path, _)| path != "/v1/models")
        .unwrap();
    assert_eq!(path, "/v1/responses");
    assert_eq!(
        body.get("model").and_then(Value::as_str),
        Some("candidate-model")
    );
    assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
    assert_eq!(
        body.get("max_output_tokens").and_then(Value::as_u64),
        Some(4_096)
    );
    assert_eq!(
        body.pointer("/reasoning/effort").and_then(Value::as_str),
        Some("high")
    );
}

#[tokio::test]
async fn inline_png_case_sends_one_fixed_image_request_without_retaining_image_content() {
    let registry = registry();
    let credentials = credentials(&registry);

    for (protocol, path, image_pointer) in [
        (
            super::ProbeProtocol::ChatCompletions,
            "/v1/chat/completions",
            "/messages/1/content/1/image_url/url",
        ),
        (
            super::ProbeProtocol::Responses,
            "/v1/responses",
            "/input/0/content/1/image_url",
        ),
    ] {
        let transport = FixtureTransport::default();
        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions {
                generation: Some(super::ProbeGenerationSelection {
                    protocol,
                    mode: super::ProbeGenerationMode::NonStreaming,
                    case: super::ProbeGenerationCase::ImageInputInlinePng,
                    custom_prompt: None,
                    custom_schema: None,
                    custom_schema_name: None,
                }),
                upstream_model: Some("candidate-model".to_owned()),
                ..ProbeOptions::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(
            report.generation.as_ref().unwrap().case,
            super::ProbeGenerationCase::ImageInputInlinePng
        );
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("data:image/png"));
        assert!(!serialized.contains("OPENBRIDGE 7"));

        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let (_, actual_path, body) = &requests[0];
        assert_eq!(actual_path, path);
        assert!(
            body.pointer(image_pointer)
                .and_then(Value::as_str)
                .is_some_and(|url| url.starts_with("data:image/png;base64,iVBORw0KGgo"))
        );
    }
}

#[tokio::test]
async fn structured_capability_cases_report_supported_not_honored_or_inconclusive() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    for protocol in [
        super::ProbeProtocol::ChatCompletions,
        super::ProbeProtocol::Responses,
    ] {
        for selected_case in [
            super::ProbeGenerationCase::Text,
            super::ProbeGenerationCase::JsonObject,
            super::ProbeGenerationCase::JsonSchema,
            super::ProbeGenerationCase::JsonSchemaStrict,
        ] {
            let report = probe_upstream_target(
                &registry,
                "openai-main",
                &transport,
                &credentials,
                ProbeOptions {
                    generation: Some(super::ProbeGenerationSelection {
                        protocol,
                        mode: super::ProbeGenerationMode::NonStreaming,
                        case: selected_case,
                        custom_prompt: None,
                        custom_schema: None,
                        custom_schema_name: None,
                    }),
                    upstream_model: Some("candidate-model".to_owned()),
                    ..ProbeOptions::default()
                },
            )
            .await
            .unwrap();

            let result = report.generation.as_ref().unwrap();
            let evidence = result
                .capability_evidence
                .as_ref()
                .expect("accepted cases always carry capability evidence");
            match selected_case {
                super::ProbeGenerationCase::Text => {
                    assert!(evidence.valid_json_object.is_none());
                }
                super::ProbeGenerationCase::JsonObject => {
                    assert_eq!(evidence.verdict, super::ProbeCapabilityVerdict::Supported);
                    assert_eq!(evidence.valid_json_object, Some(true));
                }
                super::ProbeGenerationCase::JsonSchema
                | super::ProbeGenerationCase::JsonSchemaStrict => {
                    assert_eq!(evidence.verdict, super::ProbeCapabilityVerdict::NotHonored);
                    assert_eq!(evidence.fixed_schema_match, Some(false));
                }
                _ => unreachable!(),
            }

            // Generated text, schemas, and prompts never enter the serialized report.
            let serialized = serde_json::to_string(&report).unwrap();
            assert!(!serialized.contains("probe\\\":\\\"ok"));
            assert!(!serialized.contains("Reply with exactly"));
            assert!(!serialized.contains("\"schema\""));
            assert!(!serialized.contains("\"response_format\""));
            assert!(!serialized.contains("text.format"));
            assert!(serialized.contains("\"case\":"));
        }
    }
}

#[tokio::test]
async fn stateless_tool_cases_probe_one_first_turn_each_across_json_and_sse() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);
    for protocol in [
        super::ProbeProtocol::ChatCompletions,
        super::ProbeProtocol::Responses,
    ] {
        for mode in [
            super::ProbeGenerationMode::NonStreaming,
            super::ProbeGenerationMode::Streaming,
        ] {
            for selected_case in [
                super::ProbeGenerationCase::ToolAuto,
                super::ProbeGenerationCase::ToolNone,
                super::ProbeGenerationCase::ToolRequired,
                super::ProbeGenerationCase::ToolNamed,
                super::ProbeGenerationCase::ToolStrict,
                super::ProbeGenerationCase::ToolParallelDisabled,
                super::ProbeGenerationCase::ToolParallelEnabled,
            ] {
                let report = probe_upstream_target(
                    &registry,
                    "openai-main",
                    &transport,
                    &credentials,
                    ProbeOptions {
                        generation: Some(super::ProbeGenerationSelection {
                            protocol,
                            mode,
                            case: selected_case,
                            custom_prompt: None,
                            custom_schema: None,
                            custom_schema_name: None,
                        }),
                        upstream_model: Some("candidate-model".to_owned()),
                        ..ProbeOptions::default()
                    },
                )
                .await
                .unwrap();
                assert!(
                    report
                        .generation
                        .as_ref()
                        .unwrap()
                        .capability_evidence
                        .as_ref()
                        .is_some_and(
                            |evidence| evidence.verdict == super::ProbeCapabilityVerdict::Supported
                        )
                );
                let serialized = serde_json::to_string(&report).unwrap();
                assert!(!serialized.contains("openbridge_probe_primary"));
                assert!(!serialized.contains("openbridge_probe_secondary"));
                assert!(!serialized.contains("call_private"));
                assert!(!serialized.contains("\"value\""));
            }
        }
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 28);
    for (_, _, body) in requests.iter() {
        assert!(body.get("tools").is_some());
        assert!(body.get("previous_response_id").is_none());
        assert!(body.get("background").is_none());
        assert!(body.get("conversation").is_none());
        assert!(body.get("tool_outputs").is_none());
        assert!(!body.to_string().contains("function_call_output"));
    }
}

#[tokio::test]
async fn probe_smokes_the_registered_embeddings_create_api() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials_for_target(&registry, "openai-text-embedding-3-small");

    // Run one bounded Embeddings request through its dedicated Target and adapter path.
    let report = probe_upstream_target(
        &registry,
        "openai-text-embedding-3-small",
        &transport,
        &credentials,
        ProbeOptions {
            embeddings: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(report.embeddings.unwrap().state, ProbeStatus::Accepted);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let (method, path, body) = &requests[0];
    assert_eq!(*method, Method::POST);
    assert_eq!(path, "/v1/embeddings");
    assert_eq!(
        body.get("model").and_then(Value::as_str),
        Some("text-embedding-3-small")
    );
    assert_eq!(
        body.get("input").and_then(Value::as_str),
        Some("OpenBridge probe")
    );
    assert!(body.get("tools").is_none());
}

#[tokio::test]
async fn candidate_generation_cannot_borrow_a_non_generation_target() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials_for_target(&registry, "openai-text-embedding-3-small");

    let report = probe_upstream_target(
        &registry,
        "openai-text-embedding-3-small",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        report.generation.as_ref().unwrap().outcome.state,
        ProbeStatus::Unsupported
    );
    assert_eq!(
        report.generation.as_ref().unwrap().outcome.failure,
        Some(super::ProbeFailure::OperationUnavailable)
    );
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn candidate_model_can_probe_an_unregistered_protocol_within_generation() {
    let registry = registry();
    let transport = StaticTransport::response(
        StatusCode::OK,
        json!({"object": "response", "output": []}).to_string(),
    );
    let credentials = credentials_for_target(&registry, "bailian/deepseek-v4-flash");

    let report = probe_upstream_target(
        &registry,
        "bailian/deepseek-v4-flash",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::Responses,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        report.generation.as_ref().unwrap().outcome.state,
        ProbeStatus::Accepted
    );
    assert_eq!(transport.requests.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn chatgpt_probe_uses_oauth2_lease_for_model_manifest() {
    let registry = registry();
    let auth_file = synthetic_chatgpt_auth_file();
    let mut builder = OAuth2CredentialManagerBuilder::new();
    builder
        .load_auth_json_file(
            ProviderKind::ChatGpt,
            "chatgpt-codex",
            auth_file.path.clone(),
        )
        .unwrap();
    let oauth2_credentials = builder.build();
    let transport = ChatGptModelListTransport::default();

    // Run only the fixed model-list observation through the ChatGPT OAuth2 probe boundary.
    let report = probe_upstream_target_with_oauth2(
        &registry,
        "chatgpt/gpt-5-6-sol",
        &transport,
        &oauth2_credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    // Confirm the ChatGPT-specific manifest parser and configured model correlation.
    let list_models = report.list_models.unwrap();
    assert_eq!(list_models.outcome.state, ProbeStatus::Accepted);
    assert_eq!(list_models.configured_model_listed, Some(true));
    assert_eq!(list_models.model_ids, ["gpt-5.6-sol"]);
    assert_eq!(
        transport.requests.lock().unwrap().as_slice(),
        ["/models?client_version=0.146.0"]
    );
    let authorizations = transport.authorizations.lock().unwrap();
    assert_eq!(authorizations.len(), 1);
    assert!(authorizations[0].starts_with("Bearer "));
    assert_ne!(authorizations[0], "Bearer ");
    assert_eq!(
        transport.accounts.lock().unwrap().as_slice(),
        ["synthetic-account"]
    );
}

#[tokio::test]
async fn chatgpt_probe_smokes_the_fixed_streaming_responses_api() {
    let registry = registry();
    let auth_file = synthetic_chatgpt_auth_file();
    let mut builder = OAuth2CredentialManagerBuilder::new();
    builder
        .load_auth_json_file(
            ProviderKind::ChatGpt,
            "chatgpt-codex",
            auth_file.path.clone(),
        )
        .unwrap();
    let oauth2_credentials = builder.build();
    let transport = ChatGptResponsesTransport::default();

    // Keep the known unbounded backend at zero egress until the risk is explicitly selected.
    let bounded = probe_upstream_target_with_oauth2(
        &registry,
        "chatgpt/gpt-5-6-sol",
        &transport,
        &oauth2_credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::Responses,
                mode: super::ProbeGenerationMode::Streaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();
    let bounded_streaming = bounded.generation.as_ref().unwrap();
    assert_eq!(bounded_streaming.outcome.state, ProbeStatus::Inconclusive);
    assert_eq!(
        bounded_streaming.outcome.failure,
        Some(super::ProbeFailure::RequestPreparation)
    );
    assert!(transport.requests.lock().unwrap().is_empty());

    // Observe only the registered streaming Responses API through the selected OAuth2 lease.
    let report = probe_upstream_target_with_oauth2(
        &registry,
        "chatgpt/gpt-5-6-sol",
        &transport,
        &oauth2_credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::Responses,
                mode: super::ProbeGenerationMode::Streaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            allow_unbounded_streaming_output: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    assert!(report.allow_unbounded_streaming_output);
    let streaming = report.generation.as_ref().unwrap();
    assert_eq!(streaming.outcome.state, ProbeStatus::Accepted);
    assert_eq!(
        streaming.evidence.as_ref().unwrap().terminal,
        Some(super::ProbeTerminal::ResponsesCompleted)
    );

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/responses");
    assert_eq!(
        requests[0].1.get("stream").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        requests[0].1.get("store").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        requests[0].1.get("instructions").and_then(Value::as_str),
        Some(
            "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
        )
    );
    assert!(requests[0].1.get("max_output_tokens").is_none());
    assert!(requests[0].1.get("tools").is_none());
}

#[tokio::test]
async fn probe_rejects_invalid_selection_before_credentials_or_egress() {
    let registry = registry();
    let transport = StaticTransport::failure();
    let credentials = CredentialStoreBuilder::new().build();

    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            upstream_model: Some("invalid\nmodel".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        ProbeError::InvalidSelection(super::ProbeSelectionError::InvalidUpstreamModel)
    ));

    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            embeddings: true,
            upstream_model: Some("unused-generation-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        ProbeError::InvalidSelection(super::ProbeSelectionError::UnusedUpstreamModel)
    ));

    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            allow_unbounded_streaming_output: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        ProbeError::InvalidSelection(super::ProbeSelectionError::UnusedUnboundedStreamingOutput)
    ));
    assert_eq!(transport.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn probe_rejects_unknown_target_before_any_egress() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = CredentialStoreBuilder::new().build();

    let error = probe_upstream_target(
        &registry,
        "missing",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        super::ProbeError::UnknownUpstreamTarget { .. }
    ));
}

#[test]
fn provider_target_resolution_stays_within_enabled_generation_targets() {
    let registry = registry();

    // One deployment resolves without an explicit target; another provider's target is rejected.
    let resolved = super::resolve_generation_probe_target(&registry, ProviderKind::OpenAi, None)
        .expect("single-deployment providers must resolve without --target");
    assert_eq!(resolved, "openai-gpt-5-5");
    assert_eq!(
        super::resolve_generation_probe_target(
            &registry,
            ProviderKind::OpenAi,
            Some("openai-main"),
        )
        .unwrap(),
        "openai-main"
    );
    let error = super::resolve_generation_probe_target(
        &registry,
        ProviderKind::Bailian,
        Some("openai-main"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        super::ProbeError::ProviderTargetMismatch { .. }
    ));
    let error = super::resolve_generation_probe_target(&registry, ProviderKind::KimiCn, None)
        .expect("kimi-cn keeps one trusted deployment");
    assert_eq!(error, "kimi-cn-kimi-k3");
}

#[tokio::test]
async fn probe_rejects_disabled_target_before_credentials_or_egress() {
    // Disable one compiled ChatGPT target and remove only its production Public Model publication.
    let mut definition = providers::compiled_config();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "chatgpt/gpt-5-6-sol")
        .unwrap();
    target.enabled = false;
    definition
        .public_models
        .retain(|model| model.id != "gpt-5.6-sol");
    let registry = build_registry(parse_bootstrap_config(BOOTSTRAP).unwrap(), definition).unwrap();
    let transport = StaticTransport::response(StatusCode::OK, b"{}".to_vec());
    let credentials = CredentialStoreBuilder::new().build();

    // Reject the target through the generic enabled boundary before credential lookup or egress.
    let error = probe_upstream_target(
        &registry,
        "chatgpt/gpt-5-6-sol",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "configured upstream target 'chatgpt/gpt-5-6-sol' is disabled"
    );
    assert_eq!(transport.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn probe_rejects_missing_credentials_before_any_egress() {
    let registry = registry();
    let transport = StaticTransport::response(StatusCode::OK, b"{}".to_vec());
    let credentials = CredentialStoreBuilder::new().build();

    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ProbeError::CredentialUnavailable));
    assert_eq!(transport.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn probe_rejects_unusable_authentication_material_before_egress() {
    // Build a matching pool member whose secret cannot become a valid HTTP header.
    let registry = registry();
    let target = registry.upstream_target("openai-main").unwrap();
    let pool = registry
        .credential_pool(target.credential_pool_id())
        .unwrap();
    let mut credentials = CredentialStoreBuilder::new();
    credentials
        .insert_upstream_member(
            target.kind(),
            pool.id(),
            format!("{}#1", pool.id()),
            SecretString::from("invalid\nheader-value"),
            CredentialMetadata::upstream(pool.kind(), CredentialSource::Programmatic),
        )
        .unwrap();
    let credentials = credentials.build();
    let transport = StaticTransport::response(StatusCode::OK, b"{}".to_vec());

    // Verify authentication preparation fails before the transport receives a request.
    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ProbeError::AuthenticationPreparation));
    assert_eq!(transport.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn probe_classifies_transport_http_and_json_failures_conservatively() {
    let registry = registry();
    let credentials = credentials(&registry);

    // Convert a transport failure to inconclusive without inventing an HTTP status.
    let failed = StaticTransport::failure();
    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &failed,
        &credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();
    let outcome = report.list_models.unwrap().outcome;
    assert_eq!(outcome.state, ProbeStatus::Inconclusive);
    assert_eq!(outcome.http_status, None);

    // Keep every real HTTP non-success request-scoped; a candidate-model 404 is not endpoint proof.
    for (status, expected) in [
        (StatusCode::NOT_FOUND, ProbeStatus::Rejected),
        (StatusCode::TOO_MANY_REQUESTS, ProbeStatus::Rejected),
    ] {
        let transport = StaticTransport::response(status, Vec::new());
        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions {
                generation: Some(super::ProbeGenerationSelection {
                    protocol: super::ProbeProtocol::ChatCompletions,
                    mode: super::ProbeGenerationMode::NonStreaming,
                    case: super::ProbeGenerationCase::Text,
                    custom_prompt: None,
                    custom_schema: None,
                    custom_schema_name: None,
                }),
                ..ProbeOptions::default()
            },
        )
        .await
        .unwrap();
        let outcome = &report.generation.as_ref().unwrap().outcome;
        assert_eq!(outcome.state, expected);
        assert_eq!(outcome.http_status, Some(status.as_u16()));
    }

    // Treat successful non-JSON and structurally invalid model lists as inconclusive evidence.
    for body in [b"not-json".to_vec(), b"{}".to_vec()] {
        let transport = StaticTransport::response(StatusCode::OK, body);
        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions {
                list_models: true,
                ..ProbeOptions::default()
            },
        )
        .await
        .unwrap();
        let result = report.list_models.unwrap();
        assert_eq!(result.outcome.state, ProbeStatus::Inconclusive);
        assert_eq!(result.outcome.http_status, Some(StatusCode::OK.as_u16()));
        assert_eq!(result.configured_model_listed, None);
        assert!(result.model_ids.is_empty());
    }
}

#[tokio::test]
async fn probe_classifies_streaming_terminals_and_limits() {
    let registry = registry();
    let credential_store = credentials(&registry);
    let options = ProbeOptions {
        generation: Some(super::ProbeGenerationSelection {
            protocol: super::ProbeProtocol::Responses,
            mode: super::ProbeGenerationMode::Streaming,
            case: super::ProbeGenerationCase::Text,
            custom_prompt: None,
            custom_schema: None,
            custom_schema_name: None,
        }),
        ..ProbeOptions::default()
    };

    // Distinguish accepted incomplete responses, explicit failures, and missing terminals.
    for (body, state, terminal, failure) in [
        (
            "event: response.incomplete\ndata: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\"}}\n\n",
            ProbeStatus::Accepted,
            Some(super::ProbeTerminal::ResponsesIncomplete),
            None,
        ),
        (
            "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n",
            ProbeStatus::Rejected,
            Some(super::ProbeTerminal::ResponsesFailed),
            Some(super::ProbeFailure::UpstreamTerminalFailure),
        ),
        (
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"OK\"}\n\n",
            ProbeStatus::Inconclusive,
            None,
            Some(super::ProbeFailure::MissingTerminal),
        ),
    ] {
        let transport = StaticTransport::event_stream(body);
        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credential_store,
            options.clone(),
        )
        .await
        .unwrap();
        let case = report.generation.as_ref().unwrap();
        assert_eq!(case.outcome.state, state);
        assert_eq!(case.outcome.failure, failure);
        assert_eq!(
            case.evidence
                .as_ref()
                .and_then(|evidence| evidence.terminal),
            terminal
        );
    }

    // Enforce the total response budget before accepting an unterminated stream.
    let limited_registry = registry_with_response_limit(1_000_000);
    let limited_credentials = credentials(&limited_registry);
    let oversized = StaticTransport::event_stream(vec![b'x'; 1_000_001]);
    let report = probe_upstream_target(
        &limited_registry,
        "openai-main",
        &oversized,
        &limited_credentials,
        options.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        report.generation.as_ref().unwrap().outcome.failure,
        Some(super::ProbeFailure::ResponseLimit)
    );

    // Enforce the single-event framing budget independently of the larger total body budget.
    let oversized_event = format!("data: {}\n\n", "x".repeat(262_145));
    let transport = StaticTransport::event_stream(oversized_event);
    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credential_store,
        options,
    )
    .await
    .unwrap();
    assert_eq!(
        report.generation.as_ref().unwrap().outcome.failure,
        Some(super::ProbeFailure::InvalidSse)
    );
}

#[tokio::test]
async fn probe_rejects_oversized_response_bodies() {
    // Enforce the configured JSON body limit before parsing model-list evidence.
    let limited_registry = registry_with_response_limit(1_000_000);
    let limited_credentials = credentials(&limited_registry);
    let oversized = StaticTransport::response(StatusCode::OK, vec![b'x'; 1_000_001]);
    let report = probe_upstream_target(
        &limited_registry,
        "openai-main",
        &oversized,
        &limited_credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();
    let outcome = report.list_models.unwrap().outcome;
    assert_eq!(outcome.state, ProbeStatus::Inconclusive);
    assert_eq!(outcome.http_status, Some(StatusCode::OK.as_u16()));

    // Preserve a real HTTP rejection without reading or reclassifying its oversized error body.
    let rejected = StaticTransport::response(StatusCode::BAD_REQUEST, vec![b'x'; 1_000_001]);
    let report = probe_upstream_target(
        &limited_registry,
        "openai-main",
        &rejected,
        &limited_credentials,
        ProbeOptions {
            list_models: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();
    let outcome = report.list_models.unwrap().outcome;
    assert_eq!(outcome.state, ProbeStatus::Rejected);
    assert_eq!(outcome.http_status, Some(StatusCode::BAD_REQUEST.as_u16()));
    assert_eq!(outcome.failure, None);
}

#[tokio::test]
async fn custom_prompt_override_replaces_fixed_prompt_and_fingerprints_without_retaining_text() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);
    let custom_prompt = "ADMIN AUTHORED PROMPT MARKER 12345";

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::Text,
                custom_prompt: Some(custom_prompt.to_owned()),
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    let result = report.generation.as_ref().unwrap();
    assert_eq!(result.outcome.state, super::ProbeStatus::Accepted);
    let fingerprint = result
        .custom_prompt_fingerprint
        .as_ref()
        .expect("prompt override must be fingerprinted");
    assert_eq!(fingerprint.len(), 16);
    assert!(result.custom_schema_fingerprint.is_none());

    // The override text must not enter the serialized report.
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("ADMIN AUTHORED PROMPT MARKER"));

    // The sent wire request carries only the override prompt.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let (_, _, body) = &requests[0];
    // The override replaces the user message; the trusted system instruction stays first.
    assert_eq!(
        body.pointer("/messages/1/content"),
        Some(&serde_json::json!(custom_prompt))
    );
    assert_eq!(
        body.pointer("/messages/0/role"),
        Some(&serde_json::json!("system"))
    );
}

#[tokio::test]
async fn custom_schema_override_replaces_response_format_and_forces_inconclusive_verdict() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);
    let custom_schema = r#"{"type":"object","properties":{"answer":{"type":"string"}}"#;
    // Close the JSON object to keep the fixture valid.
    let custom_schema = format!("{custom_schema}}}");

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::JsonSchema,
                custom_prompt: None,
                custom_schema: Some(custom_schema.clone()),
                custom_schema_name: Some("admin_probe_schema".to_owned()),
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    let result = report.generation.as_ref().unwrap();
    assert_eq!(result.outcome.state, super::ProbeStatus::Accepted);

    // An arbitrary schema removes the fixed oracle: verdict stays inconclusive.
    let evidence = result.capability_evidence.as_ref().unwrap();
    assert_eq!(
        evidence.verdict,
        super::ProbeCapabilityVerdict::Inconclusive
    );
    assert_eq!(evidence.fixed_schema_match, None);
    // The fixture echoes non-JSON text for json_schema requests; the oracle must not be fooled.
    assert_eq!(evidence.valid_json_object, Some(false));

    assert_eq!(
        result.custom_schema_name.as_deref(),
        Some("admin_probe_schema")
    );
    assert!(result.custom_schema_fingerprint.is_some());
    assert!(result.custom_prompt_fingerprint.is_none());

    // The override schema text must not enter the serialized report.
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("\"answer\""));

    let requests = transport.requests.lock().unwrap();
    let (_, _, body) = &requests[0];
    assert_eq!(
        body.pointer("/response_format/json_schema/name"),
        Some(&serde_json::json!("admin_probe_schema"))
    );
    assert_eq!(
        body.pointer("/response_format/json_schema/schema/properties/answer/type"),
        Some(&serde_json::json!("string"))
    );
    // The case keeps its non-strict wire flag.
    assert_eq!(
        body.pointer("/response_format/json_schema/strict"),
        Some(&serde_json::json!(false))
    );
}

#[tokio::test]
async fn invalid_or_inapplicable_overrides_fail_before_credentials_or_egress() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    // A prompt override is rejected for tool cases.
    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::ToolAuto,
                custom_prompt: Some("override".to_owned()),
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        super::ProbeError::InvalidSelection(super::ProbeSelectionError::UnsupportedPromptOverride)
    ));

    // A schema override is rejected outside json-schema cases.
    let error = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::JsonObject,
                custom_prompt: None,
                custom_schema: Some("{}".to_owned()),
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        super::ProbeError::InvalidSelection(super::ProbeSelectionError::UnsupportedSchemaOverride)
    ));

    // A non-object or oversized schema is rejected.
    for bad_schema in ["[1,2]", "\"text\"", "null"] {
        let error = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions {
                generation: Some(super::ProbeGenerationSelection {
                    protocol: super::ProbeProtocol::ChatCompletions,
                    mode: super::ProbeGenerationMode::NonStreaming,
                    case: super::ProbeGenerationCase::JsonSchema,
                    custom_prompt: None,
                    custom_schema: Some(bad_schema.to_owned()),
                    custom_schema_name: None,
                }),
                upstream_model: Some("candidate-model".to_owned()),
                ..ProbeOptions::default()
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            super::ProbeError::InvalidSelection(super::ProbeSelectionError::InvalidCustomSchema)
        ));
    }

    // No request must have been sent.
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn canonical_cases_remain_unaffected_without_overrides() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            generation: Some(super::ProbeGenerationSelection {
                protocol: super::ProbeProtocol::ChatCompletions,
                mode: super::ProbeGenerationMode::NonStreaming,
                case: super::ProbeGenerationCase::JsonSchemaStrict,
                custom_prompt: None,
                custom_schema: None,
                custom_schema_name: None,
            }),
            upstream_model: Some("candidate-model".to_owned()),
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    let result = report.generation.as_ref().unwrap();
    assert!(result.custom_prompt_fingerprint.is_none());
    assert!(result.custom_schema_fingerprint.is_none());
    assert!(result.custom_schema_name.is_none());

    let requests = transport.requests.lock().unwrap();
    let (_, _, body) = &requests[0];
    assert_eq!(
        body.pointer("/response_format/json_schema/name"),
        Some(&serde_json::json!("openbridge_probe"))
    );
    assert_eq!(
        body.pointer("/response_format/json_schema/schema/properties/probe/const"),
        Some(&serde_json::json!("ok"))
    );
}
