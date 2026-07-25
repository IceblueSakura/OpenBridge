mod support;

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use openbridge::{
    ingress::{AppState, StaticBearerCredential, build_router},
    provider::CredentialSource,
    registry::{RegistrySnapshot, build_registry},
    transport::upstream::UpstreamClient,
};
use secrecy::SecretString;
use tower::ServiceExt;

fn test_app(snapshot: RegistrySnapshot) -> axum::Router {
    let upstream = UpstreamClient::new(
        snapshot.upstream_policy().connect_timeout(),
        snapshot.upstream_policy().pool_idle_timeout(),
        snapshot.upstream_policy().pool_max_idle_per_host(),
    )
    .unwrap();
    build_router(AppState::new(
        Arc::new(snapshot),
        Arc::new(upstream),
        StaticBearerCredential::new(SecretString::from("downstream-test-token".to_owned())),
        CredentialSource::fixed(
            "OPENAI_API_KEY",
            SecretString::from("upstream-test-token".to_owned()),
        ),
    ))
}

#[tokio::test]
async fn health_reports_snapshot_version_and_sets_a_request_id() {
    let snapshot = support::snapshot("health-test", "code-primary", "test-model");
    let app = test_app(snapshot);
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
    let snapshot = build_registry(
        support::bootstrap(&bootstrap_document),
        support::definition("health-test", "code-primary", "test-model"),
    )
    .unwrap();
    let app = test_app(snapshot);
    let request = Request::builder()
        .uri("/healthz")
        .header("content-length", "9")
        .body(Body::from("123456789"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
