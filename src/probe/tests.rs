//! Unit tests for trusted capability-probe egress, protocol requests, and reports.

use std::sync::Mutex;

use axum::body::Body;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Method, StatusCode, header::AUTHORIZATION};
use secrecy::SecretString;
use serde_json::{Value, json};

use super::{ProbeOptions, SupportStatus, probe_upstream_target};
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

fn credentials(registry: &RuntimeRegistry) -> CredentialStore {
    let target = registry.upstream_target("openai-main").unwrap();
    let pool = registry
        .credential_pool(target.credential_pool_id())
        .unwrap();
    let mut credentials = CredentialStoreBuilder::new();
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
