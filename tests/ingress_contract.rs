//! 验证下游 HTTP ingress 的认证、请求边界、响应转发和错误映射契约。

mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
};
use openbridge::{
    ingress::{GatewayState, build_router},
    registry::{RuntimeRegistry, build_registry},
    transport::upstream::UpstreamClient,
};
use tower::ServiceExt;

fn test_app(registry: RuntimeRegistry) -> axum::Router {
    let upstream = UpstreamClient::new(
        registry.http_client().connect_timeout(),
        registry.http_client().pool_idle_timeout(),
        registry.http_client().pool_max_idle_per_host(),
    )
    .unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-test-token-00000000000",
        &registry,
        "upstream-test-token",
    );
    build_router(GatewayState::new(
        Arc::new(registry),
        Arc::new(upstream),
        users,
        credentials,
    ))
}

#[tokio::test]
async fn health_reports_snapshot_version_and_sets_a_request_id() {
    let registry = support::registry("health-test", "code-primary", "test-model");
    let app = test_app(registry);
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
        r#"{"status":"ok","registry_version":"health-test"}"#
    );
}

#[tokio::test]
async fn documentation_endpoints_serve_openapi_and_swagger_ui_without_authentication() {
    let response = test_app(support::registry("docs-test", "code-primary", "test-model"))
        .oneshot(Request::get("/openapi.yaml").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/yaml; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let spec = std::str::from_utf8(&body).unwrap();
    assert!(spec.contains("openapi: 3.0.3"));
    assert!(spec.contains("/healthz:"));
    assert!(spec.contains("/v1/models:"));
    assert!(spec.contains("/v1/chat/completions:"));
    assert!(spec.contains("/v1/responses:"));

    let response = test_app(support::registry("docs-test", "code-primary", "test-model"))
        .oneshot(Request::get("/swagger-ui/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let page = std::str::from_utf8(&body).unwrap();
    assert!(page.contains("SwaggerUIBundle"));
    assert!(page.contains("/openapi.yaml"));
}

#[tokio::test]
async fn requests_over_the_bootstrap_body_limit_are_rejected() {
    let bootstrap_document = support::BOOTSTRAP.replace(
        "max_request_body_bytes = 1048576",
        "max_request_body_bytes = 8",
    );
    let registry = build_registry(
        support::bootstrap(&bootstrap_document),
        support::definition("health-test", "code-primary", "test-model"),
    )
    .unwrap();
    let app = test_app(registry);
    let request = Request::builder()
        .uri("/healthz")
        .header("content-length", "9")
        .body(Body::from("123456789"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
