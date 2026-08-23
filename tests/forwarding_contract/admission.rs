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
async fn invalid_reasoning_summary_requests_fail_before_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    for reasoning in [
        serde_json::json!({"effort": "high", "summary": "detailed"}),
        serde_json::json!({"effort": "high", "summary": true}),
        serde_json::json!({"effort": "high", "summary": null}),
        serde_json::json!({"effort": "high", "summary": {"mode": "auto"}}),
        serde_json::json!({"effort": "none", "summary": "auto"}),
    ] {
        let request = serde_json::json!({
            "model": "deepseek-v4-flash",
            "input": "hello",
            "reasoning": reasoning
        });
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["type"], "invalid_request_error");
        assert_eq!(error["error"]["code"], "invalid_request_error");
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn encrypted_content_hint_does_not_hide_an_unsupported_mixed_include() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(
                    r#"{"model":"deepseek-v4-flash","input":"hello","include":["reasoning.encrypted_content","file_search_call.results"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["error"]["code"], "unsupported_model_capability");
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn deepseek_capability_error_locates_parallel_tool_calls_after_include_compatibility() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let request = serde_json::json!({
        "model": "deepseek-v4-pro",
        "input": "hello",
        "include": ["reasoning.encrypted_content"],
        "tools": [{
            "type": "function",
            "name": "lookup",
            "description": "Synthetic function",
            "parameters": {"type": "object", "properties": {}}
        }],
        "parallel_tool_calls": true
    });

    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(error["error"]["type"], "invalid_request_error");
    assert_eq!(error["error"]["code"], "unsupported_model_capability");
    assert_eq!(error["error"]["param"], "parallel_tool_calls");
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn generation_capability_families_return_deterministic_top_level_params() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let cases = [
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","stream":true}),
            "stream",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}),
            "tools",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","text":{"format":{"type":"json_object"}}}),
            "text",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","previous_response_id":"resp_previous"}),
            "previous_response_id",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","background":true}),
            "background",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","include":["file_search_call.results"]}),
            "include",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":[{"type":"message","role":"user","content":[{"type":"input_image","image_url":"https://example.invalid/image.png"}]}]}),
            "input",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","max_output_tokens":8193}),
            "max_output_tokens",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","reasoning":{"effort":"high"}}),
            "reasoning",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","temperature":0.5}),
            "temperature",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"max_completion_tokens":9000,"max_tokens":10000}),
            "max_tokens",
        ),
    ];

    for (path, request, expected_param) in cases {
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
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{expected_param}"
        );
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], "unsupported_model_capability");
        assert_eq!(error["error"]["param"], expected_param);
        assert!(error["error"].get("reason").is_none());
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn generation_capability_first_error_ignores_json_key_order() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    for body in [
        r#"{"model":"public-model","input":"hello","stream":true,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],"include":["file_search_call.results"]}"#,
        r#"{"include":["file_search_call.results"],"tools":[{"parameters":{"type":"object"},"name":"lookup","type":"function"}],"stream":true,"input":"hello","model":"public-model"}"#,
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["param"], "stream");
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_generation_shape_precedes_model_capability_order() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","input":"hello","stream":true,"reasoning":{"summary":true}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "invalid_request_error");
    assert_eq!(error["error"]["param"], "reasoning");
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn chat_stream_usage_admission_keeps_noops_but_rejects_invalid_or_unsupported_requests() {
    // Build a streaming Chat model whose fixed Native API cannot guarantee the effective usage tail.
    let mut definition =
        support::definition("stream-usage-admission", "public-model", "upstream-model");
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.streaming = true;
        capabilities.stream_usage = false;
    }
    let transport = Arc::new(DeepSeekUsageStreamTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Malformed Chat shapes and the Responses-only protocol mismatch fail before transport.
    for (path, request, expected_code) in [
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream_options":{"include_usage":true}}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":false,"stream_options":{"include_usage":true}}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":"true"}}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true,"future":false}}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_obfuscation":false}}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":"usage"}),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":null}),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"public-model","input":"hello","stream":true,"stream_options":{"include_usage":true}}),
            "unknown_parameter",
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
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{path} {request}"
        );
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], expected_code, "{path} {request}");
        assert!(transport.requests.lock().unwrap().is_empty());
    }

    // A valid effective request needs the missing capability and therefore also fails with zero egress.
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "unsupported_model_capability");
    assert_eq!(error["error"]["param"], "stream_options");
    assert!(transport.requests.lock().unwrap().is_empty());

    // Empty and explicit-false objects are omitted-equivalent and cross the same unsupported API.
    for stream_options in [
        serde_json::json!({}),
        serde_json::json!({"include_usage": false}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                    .body(Body::from(
                        serde_json::json!({
                            "model": "public-model",
                            "messages": [{"role": "user", "content": "hello"}],
                            "stream": true,
                            "stream_options": stream_options
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.body.get("stream_options").is_none())
    );
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
                    r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
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

#[tokio::test]
async fn disjoint_structured_output_routes_fail_with_zero_egress() {
    // Build two independently valid Native candidates whose Structured Output modes are disjoint.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
            }
            UpstreamApiCapabilities::Embeddings(_)
            | UpstreamApiCapabilities::ImagesGenerations(_) => {}
        }
    }
    let mut schema_target = definition.upstream_targets[0].clone();
    schema_target.id = "openai-schema-only".to_owned();
    for api in &mut schema_target.upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::NonStrictOnly,
                ));
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::NonStrictOnly,
                ));
            }
            UpstreamApiCapabilities::Embeddings(_)
            | UpstreamApiCapabilities::ImagesGenerations(_) => {}
        }
    }
    definition.upstream_targets.push(schema_target);
    definition.routes.extend([
        RouteConfig {
            id: "schema-only-chat".to_owned(),
            upstream_target: "openai-schema-only".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "schema-only-responses".to_owned(),
            upstream_target: "openai-schema-only".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::Responses,
            mode: RouteMode::Native,
        },
    ]);
    definition.public_models[0].routes.extend([
        "schema-only-chat".to_owned(),
        "schema-only-responses".to_owned(),
    ]);
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Reject both disjoint modes for both protocols through the public HTTP boundary.
    for (path, request) in [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "response_format": {"type": "json_object"}
            }),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "response_format": {"type": "json_schema", "json_schema": {"name": "answer"}}
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_object"}}
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_schema", "name": "answer"}}
            }),
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
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{path} {request}"
        );
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(
            error["error"]["code"], "unsupported_model_capability",
            "{path} {request}"
        );
    }

    // Prove no rejected request crossed the trusted transport boundary.
    assert!(transport.requests.lock().unwrap().is_empty());
}
