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
    ProbeError, ProbeOptions, SupportStatus, probe_upstream_target,
    probe_upstream_target_with_oauth2,
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
                "/v1/chat/completions" => {
                    json!({"object": "chat.completion", "choices": [{"message": {"role": "assistant", "content": "OK"}}]})
                }
                "/v1/responses" => json!({"object": "response", "output": []}),
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
async fn probe_discovers_models_and_smokes_basic_generation_apis_without_tool_payloads() {
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

    let serialized_report = serde_json::to_value(&report).unwrap();
    let list_models = report.list_models.as_ref().unwrap();
    assert_eq!(list_models.outcome.state, SupportStatus::Supported);
    assert_eq!(list_models.configured_model_listed, Some(true));
    assert_eq!(list_models.model_ids, ["test-model", "other-model"]);
    assert_eq!(
        report.chat.as_ref().unwrap().state,
        SupportStatus::Supported
    );
    assert_eq!(
        report.responses.as_ref().unwrap().state,
        SupportStatus::Supported
    );
    assert_eq!(
        report.embeddings.as_ref().unwrap().state,
        SupportStatus::Unsupported
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
    let responses = requests
        .iter()
        .find(|(_, path, _)| path == "/v1/responses")
        .map(|(_, _, body)| body)
        .unwrap();
    assert_eq!(
        responses["instructions"],
        "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
    );
    assert_eq!(responses["store"], false);
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

    assert_eq!(report.embeddings.unwrap().state, SupportStatus::Supported);
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
        "chatgpt-gpt-5-6-sol",
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
    assert_eq!(list_models.outcome.state, SupportStatus::Supported);
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

    // Observe only the registered streaming Responses API through the selected OAuth2 lease.
    let report = probe_upstream_target_with_oauth2(
        &registry,
        "chatgpt-gpt-5-6-sol",
        &transport,
        &oauth2_credentials,
        ProbeOptions {
            responses: true,
            ..ProbeOptions::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(report.responses.unwrap().state, SupportStatus::Supported);
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
async fn probe_rejects_disabled_target_before_credentials_or_egress() {
    // Disable one compiled ChatGPT target and remove only its production Public Model publication.
    let mut definition = providers::compiled_config();
    let target = definition
        .upstream_targets
        .iter_mut()
        .find(|target| target.id == "chatgpt-gpt-5-6-sol")
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
        "chatgpt-gpt-5-6-sol",
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
        "configured upstream target 'chatgpt-gpt-5-6-sol' is disabled"
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
    assert_eq!(outcome.state, SupportStatus::Unknown);
    assert_eq!(outcome.http_status, Some(StatusCode::OK.as_u16()));
}
