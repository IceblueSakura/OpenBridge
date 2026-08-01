mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
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
