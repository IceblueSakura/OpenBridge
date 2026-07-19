use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use openbridge::{
    config::{ConfigManager, load_registry},
    ingress::build_router,
};
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
config_version = "health-test"

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
upstream_model = "test-model"
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
name = "code-primary"
candidates = ["openai-main"]
"#;

#[tokio::test]
async fn health_reports_snapshot_version_and_sets_a_request_id() {
    let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
    let app = build_router(Arc::new(ConfigManager::new(snapshot)));
    let request = Request::builder()
        .uri("/healthz")
        .header(AUTHORIZATION, "Bearer must-not-be-reflected")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    assert!(!response.headers().contains_key(AUTHORIZATION));
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        std::str::from_utf8(&body).unwrap(),
        r#"{"status":"ok","config_version":"health-test"}"#
    );
}

#[tokio::test]
async fn requests_over_the_bootstrap_body_limit_are_rejected() {
    let bootstrap = BOOTSTRAP.replace(
        "max_request_body_bytes = 1048576",
        "max_request_body_bytes = 8",
    );
    let snapshot = load_registry(&bootstrap, ROUTES).unwrap();
    let app = build_router(Arc::new(ConfigManager::new(snapshot)));
    let request = Request::builder()
        .uri("/healthz")
        .header("content-length", "9")
        .body(Body::from("123456789"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
