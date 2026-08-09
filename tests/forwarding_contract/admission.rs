//! Verifies authenticated generation admission and capability rejection before egress.

use super::*;

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
        let mut request = Request::post("/v1/chat/completions")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000");
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
async fn unknown_generation_parameters_fail_consistently_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());

    // Reject the same unknown field for Native Chat and Native Responses with exact attribution.
    for (path, request) in [
        (
            "/v1/chat/completions",
            serde_json::json!({"model": "public-model", "messages": [], "future_parameter": null}),
        ),
        (
            "/v1/responses",
            serde_json::json!({"model": "public-model", "input": "hello", "future_parameter": 1}),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["type"], "invalid_request_error");
        assert_eq!(error["error"]["code"], "unknown_parameter");
        assert_eq!(error["error"]["param"], "future_parameter");
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_public_model_capability_fails_before_any_upstream_attempt() {
    // Build a preferred Route with weaker tool capability and a later Route with stronger capability.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = None;
    }
    let mut stronger = definition.upstream_targets[0].clone();
    stronger.id = "openai-stronger".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut stronger.upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(openbridge::core::FunctionToolCapabilities {
            choice_modes: openbridge::core::ALL_TOOL_CHOICE_MODES,
            parallel_calls: false,
            strict_schema: false,
        });
    }
    definition.upstream_targets.push(stronger);
    definition.routes.push(RouteConfig {
        id: "stronger-chat".to_owned(),
        upstream_target: "openai-stronger".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec!["public-chat".to_owned(), "stronger-chat".to_owned()];
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // The fixed Public Model contract rejects tool requests before egress and cannot select the stronger Route.
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","messages":[],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "unsupported_model_capability");
    assert!(transport.requests.lock().unwrap().is_empty());

    // The extended endpoint reports the same intersection rather than extra capability from a later Route.
    let detail = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    assert_eq!(
        detail["interfaces"]["chat_completions"]["tools"]["support"],
        "unsupported"
    );
}
