//! Unit tests for trusted capability-probe egress, protocol requests, and reports.

use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::body::Body;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Method, StatusCode, header::AUTHORIZATION};
use secrecy::SecretString;
use serde_json::{Value, json};

use super::{ProbeError, ProbeOptions, SupportStatus, probe_upstream_target};
use crate::{
    config::parse_bootstrap_config,
    credential::{CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder},
    provider::PreparedUpstreamRequest,
    providers,
    registry::{RuntimeRegistry, UpstreamTarget, build_registry},
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};

const BOOTSTRAP: &str = r#"
schema_version = 2
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
upstream_credentials_file = "config/upstream-credentials.toml"
max_request_body_bytes = 1048576
max_json_response_body_bytes = 16777216
max_replay_body_bytes = 262144
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

fn registry() -> RuntimeRegistry {
    let mut definition = providers::compiled_config();
    definition.version = "probe-test".to_owned();
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
        upstream_api.upstream_model = "test-model".to_owned();
    }
    build_registry(parse_bootstrap_config(BOOTSTRAP).unwrap(), definition).unwrap()
}

fn registry_with_response_limit(max_response_bytes: usize) -> RuntimeRegistry {
    // Override only the probe response budget in the standard bootstrap fixture.
    let bootstrap = BOOTSTRAP.replace(
        "max_json_response_body_bytes = 16777216",
        &format!("max_json_response_body_bytes = {max_response_bytes}"),
    );

    // Compile the ordinary provider catalog with stable model names for probe assertions.
    let mut definition = providers::compiled_config();
    definition.version = "probe-limit-test".to_owned();
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
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
            let response = match request.relative_uri().path() {
                "/v1/models" => {
                    json!({"object": "list", "data": [{"id": "test-model"}, {"id": "other-model"}]})
                }
                "/v1/chat/completions"
                if body.get("tools").is_some() && !has_tool_result(&body) =>
                    {
                        json!({
                        "object": "chat.completion",
                        "choices": [{"message": {"role": "assistant", "content": null, "tool_calls": [{
                            "id": "call_chat", "type": "function", "function": {"name": "openbridge_probe", "arguments": "{}"}
                        }]}}]
                    })
                    }
                "/v1/chat/completions" => {
                    json!({"object": "chat.completion", "choices": [{"message": {"role": "assistant", "content": "OK"}}]})
                }
                "/v1/responses" if body.get("tools").is_some() && !has_tool_result(&body) => {
                    json!({
                        "object": "response", "output": [{"type": "function_call", "call_id": "call_response", "name": "openbridge_probe", "arguments": "{}"}]
                    })
                }
                "/v1/responses" => json!({"object": "response", "output": []}),
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

struct StaticTransport {
    status: StatusCode,
    body: Vec<u8>,
    fails: bool,
    requests: AtomicUsize,
}

impl StaticTransport {
    fn response(status: StatusCode, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
            fails: false,
            requests: AtomicUsize::new(0),
        }
    }

    fn failure() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
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
                HeaderMap::new(),
                Body::from(self.body.clone()),
            ))
        })
    }
}

struct SequenceTransport {
    responses: Mutex<VecDeque<(StatusCode, Vec<u8>)>>,
}

impl SequenceTransport {
    fn new(responses: impl IntoIterator<Item=(StatusCode, Vec<u8>)>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

impl UpstreamTransport for SequenceTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move {
            // Consume exactly one scripted response for each ordered probe exchange.
            let (status, body) = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("the probe must not exceed its scripted exchanges");

            // Return the scripted wire outcome through the production transport contract.
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from(body),
            ))
        })
    }
}

fn has_tool_result(body: &Value) -> bool {
    body.get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages
                .iter()
                .any(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
        })
        || body
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
            })
        })
}

#[tokio::test]
async fn probe_discovers_models_and_verifies_both_tool_loops_without_rewriting_configuration() {
    let registry = registry();
    let transport = FixtureTransport::default();
    let credentials = credentials(&registry);

    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions::all(),
    )
        .await
        .unwrap();

    let list_models = report.list_models.unwrap();
    assert_eq!(list_models.outcome.state, SupportStatus::Supported);
    assert_eq!(list_models.configured_model_listed, Some(true));
    assert_eq!(list_models.model_ids, ["test-model", "other-model"]);
    assert_eq!(report.chat.unwrap().state, SupportStatus::Supported);
    assert_eq!(report.responses.unwrap().state, SupportStatus::Supported);
    assert_eq!(
        report
            .chat_function_calling
            .unwrap()
            .result_replay
            .unwrap()
            .state,
        SupportStatus::Supported
    );
    assert_eq!(
        report
            .responses_function_calling
            .unwrap()
            .result_replay
            .unwrap()
            .state,
        SupportStatus::Supported
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

    // Convert a transport failure to unknown without inventing an HTTP status.
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
    assert_eq!(outcome.state, SupportStatus::Unknown);
    assert_eq!(outcome.http_status, None);

    // Reserve unsupported only for explicit endpoint statuses and keep rate limits unknown.
    for (status, expected) in [
        (StatusCode::NOT_FOUND, SupportStatus::Unsupported),
        (StatusCode::TOO_MANY_REQUESTS, SupportStatus::Unknown),
    ] {
        let transport = StaticTransport::response(status, Vec::new());
        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions {
                chat: true,
                ..ProbeOptions::default()
            },
        )
            .await
            .unwrap();
        let outcome = report.chat.unwrap();
        assert_eq!(outcome.state, expected);
        assert_eq!(outcome.http_status, Some(status.as_u16()));
    }

    // Treat successful non-JSON and structurally invalid model lists as unknown evidence.
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
        assert_eq!(result.outcome.state, SupportStatus::Unknown);
        assert_eq!(result.outcome.http_status, Some(StatusCode::OK.as_u16()));
        assert_eq!(result.configured_model_listed, None);
        assert!(result.model_ids.is_empty());
    }
}

#[tokio::test]
async fn probe_rejects_oversized_bodies_and_unusable_tool_call_shapes() {
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
    assert_eq!(outcome.state, SupportStatus::Unknown);
    assert_eq!(outcome.http_status, Some(StatusCode::OK.as_u16()));

    // Require a replayable tool call before reporting initial function-calling support.
    let registry = registry();
    let credentials = credentials(&registry);
    let unusable = StaticTransport::response(
        StatusCode::OK,
        br#"{"object":"chat.completion","choices":[{"message":{"role":"assistant","content":"plain text"}}]}"#
            .to_vec(),
    );
    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &unusable,
        &credentials,
        ProbeOptions {
            function_calling: true,
            ..ProbeOptions::default()
        },
    )
        .await
        .unwrap();
    let chat = report.chat_function_calling.unwrap();
    assert_eq!(chat.initial_call.state, SupportStatus::Unknown);
    assert!(chat.result_replay.is_none());
    let responses = report.responses_function_calling.unwrap();
    assert_eq!(responses.initial_call.state, SupportStatus::Unknown);
    assert!(responses.result_replay.is_none());
}

#[tokio::test]
async fn probe_reports_an_unconfigured_protocol_without_egress() {
    // Select a compiled Chat-only target while requesting only the Responses probe.
    let registry = registry();
    let credentials = credentials_for_target(&registry, "deepseek-v4-pro");
    let transport = StaticTransport::response(StatusCode::OK, b"{}".to_vec());

    // Verify the absent protocol is reported locally without issuing a request.
    let report = probe_upstream_target(
        &registry,
        "deepseek-v4-pro",
        &transport,
        &credentials,
        ProbeOptions {
            responses: true,
            ..ProbeOptions::default()
        },
    )
        .await
        .unwrap();

    let outcome = report.responses.unwrap();
    assert_eq!(outcome.state, SupportStatus::Unsupported);
    assert_eq!(outcome.http_status, None);
    assert_eq!(transport.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn probe_keeps_initial_tool_support_when_result_replay_is_invalid() {
    // Script valid tool calls followed by unusable replay responses for both protocols.
    let registry = registry();
    let credentials = credentials(&registry);
    let transport = SequenceTransport::new([
        (
            StatusCode::OK,
            br#"{"object":"chat.completion","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_chat","type":"function","function":{"name":"openbridge_probe","arguments":"{}"}}]}}]}"#
                .to_vec(),
        ),
        (StatusCode::OK, b"{}".to_vec()),
        (
            StatusCode::OK,
            br#"{"object":"response","output":[{"type":"function_call","call_id":"call_response","name":"openbridge_probe","arguments":"{}"}]}"#
                .to_vec(),
        ),
        (StatusCode::BAD_GATEWAY, b"{}".to_vec()),
    ]);

    // Run both ordered tool loops through the same trusted probe session.
    let report = probe_upstream_target(
        &registry,
        "openai-main",
        &transport,
        &credentials,
        ProbeOptions {
            function_calling: true,
            ..ProbeOptions::default()
        },
    )
        .await
        .unwrap();

    // Preserve initial support while classifying each failed replay conservatively.
    let chat = report.chat_function_calling.unwrap();
    assert_eq!(chat.initial_call.state, SupportStatus::Supported);
    assert_eq!(chat.result_replay.unwrap().state, SupportStatus::Unknown);
    let responses = report.responses_function_calling.unwrap();
    assert_eq!(responses.initial_call.state, SupportStatus::Supported);
    assert_eq!(
        responses.result_replay.unwrap().state,
        SupportStatus::Unknown
    );
    assert!(transport.responses.lock().unwrap().is_empty());
}
