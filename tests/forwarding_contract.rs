use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, SET_COOKIE},
    },
};
use bytes::Bytes;
use futures_util::{future::BoxFuture, stream};
use http::{HeaderMap, HeaderValue};
use openbridge::{
    config::{ConfigManager, ResolvedDeployment, load_registry},
    ingress::{AppState, StaticBearerCredential, build_router},
    provider::{CredentialSource, UpstreamRequestParts},
    transport::upstream::{UpstreamError, UpstreamResponse, UpstreamTransport},
};
use secrecy::SecretString;
use serde_json::Value;
use tower::ServiceExt;

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
config_version = "forward-test"
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

#[derive(Debug)]
struct RecordedRequest {
    path: String,
    authorization: String,
    body: Value,
}

#[derive(Default)]
struct RecordingTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

struct TimeoutTransport;

struct InvalidSseTransport;

#[derive(Default)]
struct FailoverTransport {
    attempted_models: Mutex<Vec<String>>,
}

impl UpstreamTransport for TimeoutTransport {
    fn send<'a>(
        &'a self,
        _deployment: &'a ResolvedDeployment,
        _request: UpstreamRequestParts,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
        Box::pin(async { Err(UpstreamError::Timeout) })
    }
}

impl UpstreamTransport for InvalidSseTransport {
    fn send<'a>(
        &'a self,
        _deployment: &'a ResolvedDeployment,
        _request: UpstreamRequestParts,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(vec![Ok::<_, Infallible>(Bytes::from_static(
                    b"data: \xff\n\n",
                ))])),
            ))
        })
    }
}

impl UpstreamTransport for RecordingTransport {
    fn send<'a>(
        &'a self,
        _deployment: &'a ResolvedDeployment,
        request: UpstreamRequestParts,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path: path.clone(),
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            body: serde_json::from_slice(request.body()).unwrap(),
        });
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(if path.ends_with("responses") {
                    "text/event-stream"
                } else {
                    "application/json"
                }),
            );
            response_headers.insert("openai-request-id", HeaderValue::from_static("upstream-id"));
            response_headers.insert(SET_COOKIE, HeaderValue::from_static("must-not-pass=true"));
            let chunks = if path.ends_with("responses") {
                vec![
                    Ok::<_, Infallible>(Bytes::from_static(b"event: response.output_text.delta\n")),
                    Ok(Bytes::from_static(b"data: {\"delta\":\"hi\"}\n\n")),
                ]
            } else {
                vec![Ok(Bytes::from_static(b"{\"id\":\"chat-result\"}"))]
            };
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(chunks)),
            ))
        })
    }
}

impl UpstreamTransport for FailoverTransport {
    fn send<'a>(
        &'a self,
        _deployment: &'a ResolvedDeployment,
        request: UpstreamRequestParts,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
        let model = serde_json::from_slice::<Value>(request.body()).unwrap()["model"]
            .as_str()
            .unwrap()
            .to_owned();
        self.attempted_models.lock().unwrap().push(model.clone());
        Box::pin(async move {
            let status = if model == "fallback-model" {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from("{}"),
            ))
        })
    }
}

fn app_with_transport(transport: Arc<dyn UpstreamTransport>) -> axum::Router {
    app_with_transport_and_routes(transport, ROUTES)
}

fn app_with_transport_and_routes(
    transport: Arc<dyn UpstreamTransport>,
    routes: &str,
) -> axum::Router {
    let snapshot = load_registry(BOOTSTRAP, routes).unwrap();
    let state = AppState::new(
        Arc::new(ConfigManager::new(snapshot)),
        transport,
        StaticBearerCredential::new(SecretString::from("downstream-token".to_owned())),
        CredentialSource::fixed(
            "OPENAI_API_KEY",
            SecretString::from("upstream-token".to_owned()),
        ),
    );
    build_router(state)
}

#[tokio::test]
async fn business_endpoints_reject_unauthenticated_requests_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(transport.requests.lock().unwrap().is_empty());
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_api_key");
}

#[tokio::test]
async fn business_endpoints_require_json_content_type_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());

    for content_type in [None, Some("text/plain")] {
        let mut request =
            Request::post("/v1/chat/completions").header(AUTHORIZATION, "Bearer downstream-token");
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn models_lists_only_public_aliases_after_authentication() {
    let app = app_with_transport(Arc::new(RecordingTransport::default()));
    let request = Request::get("/v1/models")
        .header(AUTHORIZATION, "Bearer downstream-token")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let models: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(models["object"], "list");
    assert_eq!(
        models["data"],
        serde_json::json!([
            {"id": "public-model", "object": "model", "owned_by": "openbridge"}
        ])
    );
    assert!(!std::str::from_utf8(&body).unwrap().contains("openai-main"));
}

#[tokio::test]
async fn streaming_requests_fail_over_to_the_next_compatible_deployment_before_output() {
    let routes = ROUTES
        .replace(
            "[[aliases]]",
            r#"[[deployments]]
id = "openai-fallback"
provider = "openai"
upstream_model = "fallback-model"
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

[[aliases]]"#,
        )
        .replace(
            "candidates = [\"openai-main\"]",
            "candidates = [\"openai-main\", \"openai-fallback\"]",
        );
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_routes(transport.clone(), &routes);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let attempted_models = transport.attempted_models.lock().unwrap();
    assert_eq!(attempted_models.last(), Some(&"fallback-model".to_owned()));
    assert!(
        attempted_models
            .iter()
            .any(|model| model == "upstream-model")
    );
}

#[tokio::test]
async fn provider_bound_streams_do_not_fall_back_to_another_deployment() {
    let routes = ROUTES
        .replace(
            "previous_response_id = false",
            "previous_response_id = true",
        )
        .replace(
            "[[aliases]]",
            r#"[[deployments]]
id = "openai-fallback"
provider = "openai"
upstream_model = "fallback-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities]
chat = true
responses = true
streaming = true
function_tools = true
structured_output = false
previous_response_id = true
background = false
response_store = false

[[aliases]]"#,
        )
        .replace(
            "candidates = [\"openai-main\"]",
            "candidates = [\"openai-main\", \"openai-fallback\"]",
        );
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_routes(transport.clone(), &routes);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true,"previous_response_id":"resp_123"}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        transport.attempted_models.lock().unwrap().as_slice(),
        ["upstream-model", "upstream-model"]
    );
}

#[tokio::test]
async fn invalid_upstream_sse_closes_the_stream_after_output_starts() {
    let app = app_with_transport(Arc::new(InvalidSseTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await.is_err());
}

#[tokio::test]
async fn upstream_timeouts_return_a_safe_gateway_timeout() {
    let app = app_with_transport(Arc::new(TimeoutTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "upstream_timeout");
    assert!(!std::str::from_utf8(&body).unwrap().contains("reqwest"));
}

#[tokio::test]
async fn chat_and_responses_are_forwarded_natively_with_safe_response_headers() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            r#"{"model":"public-model","messages":[]}"#,
            "application/json",
            b"{\"id\":\"chat-result\"}".as_slice(),
        ),
        (
            "/v1/responses",
            r#"{"model":"public-model","input":"hello","stream":true}"#,
            "text/event-stream",
            b"event: response.output_text.delta\ndata: {\"delta\":\"hi\"}\n\n".as_slice(),
        ),
    ];

    for (path, request_body, expected_content_type, expected_body) in cases {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token")
            .body(Body::from(request_body))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], expected_content_type);
        assert_eq!(response.headers()["openai-request-id"], "upstream-id");
        assert!(response.headers().contains_key("x-request-id"));
        assert!(!response.headers().contains_key(SET_COOKIE));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected_body
        );
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(requests[1].path, "/v1/responses");
    for request in requests.iter() {
        assert_eq!(request.authorization, "Bearer upstream-token");
        assert_eq!(request.body["model"], "upstream-model");
    }
}
