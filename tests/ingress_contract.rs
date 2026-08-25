//! Verifies downstream HTTP ingress authentication, request boundaries, response forwarding, and error mapping.

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
    registry::{RuntimeRegistry, UpstreamApiCapabilities, build_registry},
    transport::upstream::UpstreamClient,
};
use serde_json::Value;
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
async fn documentation_resources_are_public_and_cross_linked() {
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
    assert!(spec.starts_with("openapi: 3.0.3\n"));
    assert!(spec.lines().any(|line| line == "paths:"));
    assert!(spec.lines().any(|line| line == "components:"));

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
    let bootstrap_document = support::BOOTSTRAP
        .replace("max_request_body = \"1MiB\"", "max_request_body = \"8B\"")
        .replace("max_replay_body = \"256KiB\"", "max_replay_body = \"8B\"");
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

#[tokio::test]
async fn protected_endpoints_reject_malformed_bearer_schemes() {
    let app = test_app(support::registry(
        "auth-boundary-test",
        "code-primary",
        "test-model",
    ));

    for authorization in [
        "Bearer ",
        "bearer downstream-test-token-00000000000",
        "Basic value",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/v1/models")
                    .header(AUTHORIZATION, authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()["www-authenticate"], "Bearer");
    }
}

#[tokio::test]
async fn chat_admission_maps_invalid_documents_and_fixed_streaming_rejections() {
    // Map malformed JSON and missing models to the stable downstream request error.
    let app = test_app(support::registry(
        "admission-test",
        "code-primary",
        "test-model",
    ));
    for body in ["{".to_owned(), r#"{"messages":[]}"#.to_owned()] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json; charset=utf-8")
                    .header(AUTHORIZATION, "Bearer downstream-test-token-00000000000")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["error"]["code"], "invalid_request_error");
    }

    // Map a fixed interface streaming rejection before any real upstream request can occur.
    let mut definition = support::definition("streaming-test", "code-primary", "test-model");
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.streaming = false;
    }
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();
    let response = test_app(registry)
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "Application/JSON")
                .header(AUTHORIZATION, "Bearer downstream-test-token-00000000000")
                .body(Body::from(
                    r#"{"model":"code-primary","messages":[{"role":"user","content":"hello"}],"stream":true}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "unsupported_model_capability");
}

#[tokio::test]
async fn instruction_and_stateless_boundaries_return_typed_bad_requests_before_egress() {
    let app = test_app(support::registry(
        "instruction-admission-test",
        "code-primary",
        "test-model",
    ));
    for (body, param) in [
        (
            serde_json::json!({"model": "code-primary", "input": "hello", "instructions": null}),
            "instructions",
        ),
        (
            serde_json::json!({"model": "code-primary", "input": "hello", "instructions": "   "}),
            "instructions",
        ),
        (
            serde_json::json!({"model": "code-primary", "input": "hello", "store": true}),
            "store",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-test-token-00000000000")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], "invalid_request_error");
        assert_eq!(error["error"]["param"], param);
    }

    for messages in [
        serde_json::json!([]),
        serde_json::json!([{"role": "future", "content": "hello"}]),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-test-token-00000000000")
                    .body(Body::from(
                        serde_json::json!({"model": "code-primary", "messages": messages})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
