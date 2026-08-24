//! Verifies same-protocol Native forwarding, trusted request preparation, and reasoning mapping.

use super::*;

#[tokio::test]
async fn provider_request_header_hook_overrides_user_agent_for_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    for (path, body) in [
        (
            "/v1/chat/completions",
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
        ),
        (
            "/v1/responses",
            r#"{"model":"public-model","input":"hello"}"#,
        ),
    ] {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .header(USER_AGENT, "openbridge-contract-client/1.0")
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert_eq!(
            request.user_agent.as_deref(),
            Some("openbridge-contract-client/1.0")
        );
        assert_eq!(request.authorization, "Bearer upstream-token");
    }
}

#[tokio::test]
async fn chat_and_responses_are_forwarded_natively_with_safe_response_headers() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_streaming_transport(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
            "application/json",
            b"{\"id\":\"chatcmpl_result\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"hi\"},\"finish_reason\":\"stop\"}]}".as_slice(),
        ),
        (
            "/v1/responses",
            r#"{"model":"public-model","input":"hello","stream":true}"#,
            "text/event-stream",
            concat!(
                "event: response.output_text.delta\n",
                "data: {\"delta\":\"hi\"}\n\n",
                "event: response.completed\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_recording\",\"status\":\"completed\",\"output\":[]}}\n\n",
            )
            .as_bytes(),
        ),
    ];

    for (path, request_body, expected_content_type, expected_body) in cases {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
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

#[tokio::test]
async fn responses_native_preserves_supported_reasoning_summary_values() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    for summary in [serde_json::json!(false), serde_json::json!("auto")] {
        let request = serde_json::json!({
            "model": "deepseek-v4-flash",
            "input": "hello",
            "stream": true,
            "reasoning": {"effort": "high", "summary": summary}
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
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 4096).await.unwrap();
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body["reasoning"]["summary"], false);
    assert_eq!(requests[1].body["reasoning"]["summary"], "auto");
}

#[tokio::test]
async fn deepseek_responses_native_omits_the_encrypted_content_compatibility_hint() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Accept the downstream compatibility hint even though DeepSeek's Native Responses API does not.
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(
                    r#"{"model":"deepseek-v4-flash","input":"hello","stream":true,"include":["reasoning.encrypted_content"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    // Omission is candidate-specific and must be visible on the final post-adapter wire body.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/responses");
    assert!(requests[0].body.get("include").is_none());
}

#[tokio::test]
async fn responses_native_reuses_reasoning_summary_during_credential_rotation() {
    let transport = Arc::new(CredentialRotationTransport::default());
    let (app, metrics) =
        app_with_compiled_registry_and_pool(transport.clone(), &["key-a", "key-b"]);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(
            r#"{"model":"deepseek-v4-flash","input":"hello","stream":true,"reasoning":{"summary":"auto"}}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bodies = transport.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0]["reasoning"]["summary"], "auto");
    assert_eq!(bodies[1], bodies[0]);
    assert_eq!(metrics.snapshot().credential_rotations, 1);
    assert_eq!(metrics.snapshot().upstream_retries, 1);
}

#[tokio::test]
async fn longcat_responses_native_forwards_prompt_cache_key_and_removes_empty_include() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Exercise the checked-in LongCat Native Responses target that passed the real upstream probe.
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(
                    r#"{"model":"LongCat-2.0","input":"hello","stream":true,"include":[],"prompt_cache_key":"cache-test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    // Inspect the post-adapter wire body rather than only the pre-adapter RoutePlan.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/openai/v1/responses");
    assert_eq!(requests[0].body["prompt_cache_key"], "cache-test");
    assert!(requests[0].body.get("include").is_none());
}

#[tokio::test]
async fn deepseek_native_omits_prompt_cache_hint_and_inactive_retention() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    for (path, request) in [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "hello"}],
                "prompt_cache_key": "cache-test",
                "prompt_cache_retention": null
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "hello",
                "stream": true,
                "prompt_cache_key": "cache-test",
                "prompt_cache_retention": null
            }),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
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
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert!(request.body.get("prompt_cache_key").is_none());
        assert!(request.body.get("prompt_cache_retention").is_none());
    }
}

#[tokio::test]
async fn deepseek_native_omits_parallel_tool_control_without_executable_tools() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "hello"}],
                "parallel_tool_calls": true
            }),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "hello"}],
                "parallel_tool_calls": false
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "hello",
                "stream": true,
                "parallel_tool_calls": null
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "hello",
                "stream": true,
                "tools": [{
                    "type": "function",
                    "name": "lookup",
                    "parameters": {"type": "object", "properties": {}}
                }],
                "tool_choice": "none",
                "parallel_tool_calls": true
            }),
        ),
    ];

    for (path, request) in cases {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
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
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for request in requests.iter() {
        assert!(request.body.get("parallel_tool_calls").is_none());
    }
}

#[tokio::test]
async fn verified_serial_only_api_omits_active_false_parallel_tool_control() {
    let mut definition =
        support::definition("serial-tool-control", "public-model", "upstream-model");
    definition.upstream_targets[0].upstream_apis[1]
        .model_rules
        .serial_tool_calls_only = true;
    let UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("second synthetic API must be Responses");
    };
    capabilities.function_tools = Some(FunctionToolCapabilities {
        choice_modes: ALL_TOOL_CHOICE_MODES,
        parallel_calls: false,
        strict_schema: false,
    });
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    serde_json::json!({
                        "model": "public-model",
                        "input": "call the tool",
                        "tools": [{
                            "type": "function",
                            "name": "lookup",
                            "parameters": {"type": "object", "properties": {}}
                        }],
                        "parallel_tool_calls": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 64 * 1024).await.unwrap();

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].body.get("parallel_tool_calls").is_none());
}

#[tokio::test]
async fn deepseek_v4_flash_chat_native_exposes_plain_text_reasoning_content() {
    // Build the actual compiled DeepSeek route and an explicit reasoning request.
    let transport = Arc::new(DeepSeekReasoningStreamTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let request_body = r#"{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"请回答"}],"stream":true,"reasoning_effort":"high"}"#;

    // Submit a Chat Native request and confirm that the gateway preserves the original SSE body.
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    assert_eq!(response.headers()["openai-request-id"], "deepseek-id");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(body.as_ref(), DEEPSEEK_CHAT_REASONING_STREAM);

    // Use the Chat state machine to confirm that reasoning_content is a separate PlainText channel, not visible text.
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    let mut state = ChatStreamState::new();
    for event in events {
        state.ingest(&event).unwrap();
    }
    state.finish().unwrap();
    assert_eq!(state.reasoning_text(), "先分析后得出结论");
    assert_eq!(state.text(), "答案");
    assert_eq!(state.terminal(), Some(StreamTerminal::Completed));

    // Confirm the request reaches the DeepSeek Chat endpoint and preserves the canonical model and reasoning level.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/chat/completions");
    assert_eq!(requests[0].authorization, "Bearer upstream-token");
    assert_eq!(requests[0].body["model"], "deepseek-v4-flash");
    assert_eq!(requests[0].body["reasoning_effort"], "high");
}

#[tokio::test]
async fn deepseek_stream_usage_option_and_usage_tail_are_byte_transparent() {
    // Exercise the checked-in first DeepSeek Flash Chat candidate with the exact Hermes option.
    let transport = Arc::new(DeepSeekUsageStreamTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let request_body = r#"{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"hello"}],"stream":true,"stream_options":{"include_usage":true}}"#;
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    // Preserve every Provider-specific usage detail and the terminal without local normalization.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(body.as_ref(), DEEPSEEK_CHAT_USAGE_STREAM);

    // Preserve the exact nested request value on the post-adapter Native wire.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/chat/completions");
    assert_eq!(requests[0].body["model"], "deepseek-v4-flash");
    assert_eq!(
        requests[0].body["stream_options"],
        serde_json::json!({"include_usage": true})
    );
}

#[tokio::test]
async fn deepseek_v4_flash_responses_native_preserves_typed_reasoning_stream() {
    // Build the production registry and select the first Responses candidate for DeepSeek V4 Flash.
    let transport = Arc::new(DeepSeekResponsesStreamTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let request_body = r#"{"model":"deepseek-v4-flash","input":"\u8bf7\u56de\u7b54","stream":true,"reasoning":{"effort":"high"}}"#;

    // Submit the downstream Responses request and preserve the typed semantic event stream byte for byte.
    let response = app
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    assert_eq!(response.headers()["openai-request-id"], "deepseek-id");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(body.as_ref(), DEEPSEEK_RESPONSES_REASONING_STREAM);

    // Reconstruct the stream to confirm reasoning, visible text, and the explicit terminal event.
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    let mut state = ResponsesStreamState::new();
    for event in events {
        state.ingest(&event).unwrap();
    }
    state.finish().unwrap();
    assert_eq!(state.reasoning_text(), "先分析");
    assert_eq!(state.text(), "答案");
    assert_eq!(state.terminal(), Some(StreamTerminal::Completed));

    // Confirm direct DeepSeek egress and preserve the public model plus standard reasoning configuration.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/responses");
    assert_eq!(requests[0].authorization, "Bearer upstream-token");
    assert_eq!(requests[0].body["model"], "deepseek-v4-flash");
    assert_eq!(requests[0].body["reasoning"]["effort"], "high");
}

#[tokio::test]
async fn deepseek_json_object_is_preserved_by_native_egress() {
    // Exercise both public models across their Native Chat and Responses request shapes.
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "messages": [{"role": "user", "content": "Return json like {\"result\":\"ok\"}."}],
                "response_format": {"type": "json_object"}
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "deepseek-v4-pro",
                "input": "Return json like {\"result\":\"ok\"}.",
                "text": {"format": {"type": "json_object"}}
            }),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "Return json like {\"result\":\"ok\"}."}],
                "response_format": {"type": "json_object"}
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "deepseek-v4-flash",
                "input": "Return json like {\"result\":\"ok\"}.",
                "stream": true,
                "text": {"format": {"type": "json_object"}}
            }),
        ),
    ];

    // Submit each request through the production registry and consume its complete downstream body.
    for (path, body) in cases {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path} {body}");
        let _ = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }

    // Verify both V4 models keep Chat response_format and send Responses text.format to Native egress.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].path, "/chat/completions");
    assert_eq!(requests[1].path, "/responses");
    assert_eq!(requests[2].path, "/chat/completions");
    assert_eq!(requests[3].path, "/responses");
    for request in [&requests[0], &requests[2]] {
        assert_eq!(
            request.body["response_format"],
            serde_json::json!({"type": "json_object"})
        );
    }
    for request in [&requests[1], &requests[3]] {
        assert_eq!(
            request.body["text"]["format"],
            serde_json::json!({"type": "json_object"})
        );
    }
}

#[tokio::test]
async fn egress_preparation_applies_the_selected_api_reasoning_level_mapping() {
    let mut definition = streaming_definition("forward-test", "public-model", "upstream-model");
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::XHigh]);
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
        upstream_api.model_rules.reasoning_level_mappings = vec![ReasoningLevelMapping {
            downstream: ReasoningLevel::XHigh,
            upstream: "max".to_owned(),
        }];
    }
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Send the canonical Chat level through the selected Chat Upstream API mapping.
    let chat = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"xhigh"}"#,
        ))
        .unwrap();
    let response = app.clone().oneshot(chat).await.unwrap();
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    // Send the canonical Responses level through the selected Responses Upstream API mapping.
    let responses = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-0000000000000000",
        )
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true,"reasoning":{"effort":"xhigh"}}"#,
        ))
        .unwrap();
    let response = app.oneshot(responses).await.unwrap();
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    // Verify that each protocol rewrites only its standard reasoning field at egress.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body["reasoning_effort"], "max");
    assert_eq!(requests[1].body["reasoning"]["effort"], "max");
}

#[tokio::test]
async fn routed_egress_drops_only_configured_ordinary_generation_parameters() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    support::generation_profile_mut(&mut definition.models[0]).supported_parameters =
        ["logprobs", "n", "seed", "temperature", "top_logprobs"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    definition.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Submit one ignored hint beside output-shaping fields that must remain transparent.
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"logprobs":true,"n":2,"seed":7,"temperature":0.2,"top_logprobs":3}"#,
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 4096).await.unwrap();

    // Verify only the selected API's configured keys disappear from the final egress body.
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].body.get("temperature").is_none());
        assert_eq!(requests[0].body["logprobs"], true);
        assert_eq!(requests[0].body["n"], 2);
        assert_eq!(requests[0].body["seed"], 7);
        assert_eq!(requests[0].body["top_logprobs"], 3);
    }

    // Keep ignored parameters visible as downstream-accepted interface parameters.
    let model = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    let parameters = model["interfaces"]["chat_completions"]["supported_parameters"]
        .as_array()
        .unwrap();
    for parameter in ["logprobs", "n", "seed", "temperature", "top_logprobs"] {
        assert!(parameters.iter().any(|value| value == parameter));
    }
}

#[tokio::test]
async fn kimi_k3_drops_documented_fixed_sampling_parameters_before_egress() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Exercise the ignored hints through both Chat Native and Responses-to-Chat Bridge.
    for (path, request) in [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "kimi-k3",
                "messages": [{"role": "user", "content": "hello"}],
                "frequency_penalty": 0.5,
                "presence_penalty": 0.5,
                "seed": 7,
                "temperature": 0.2,
                "top_p": 0.9,
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "kimi-k3",
                "input": "hello",
                "frequency_penalty": 0.5,
                "presence_penalty": 0.5,
                "temperature": 0.2,
                "top_p": 0.9,
            }),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
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
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 4096).await.unwrap();
    }

    // Remove all fixed Kimi sampling fields while preserving an independently accepted ordinary field.
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            for parameter in [
                "frequency_penalty",
                "presence_penalty",
                "temperature",
                "top_p",
            ] {
                assert!(
                    request.body.get(parameter).is_none(),
                    "unexpected egress parameter {parameter}"
                );
            }
        }
        assert_eq!(requests[0].body["seed"], 7);
        assert!(requests[1].body.get("seed").is_none());
    }

    // Advertise ignored hints but exclude output-shaping parameters rejected by this API.
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/kimi-k3").await;
    for interface in ["chat_completions", "responses"] {
        let parameters = model["interfaces"][interface]["supported_parameters"]
            .as_array()
            .unwrap();
        for parameter in [
            "frequency_penalty",
            "presence_penalty",
            "temperature",
            "top_p",
        ] {
            assert!(parameters.iter().any(|value| value == parameter));
        }
        for parameter in ["logprobs", "n", "top_logprobs"] {
            assert!(!parameters.iter().any(|value| value == parameter));
        }
        assert_eq!(
            parameters.iter().any(|value| value == "seed"),
            interface == "chat_completions"
        );
    }
}

#[tokio::test]
async fn kimi_k3_rejects_output_shaping_parameters_before_egress() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Reject fields whose omission would change the number or shape of visible outputs.
    for (path, parameter, value) in [
        ("/v1/chat/completions", "logprobs", serde_json::json!(true)),
        ("/v1/chat/completions", "n", serde_json::json!(2)),
        ("/v1/chat/completions", "top_logprobs", serde_json::json!(3)),
        ("/v1/responses", "logprobs", serde_json::json!(true)),
        ("/v1/responses", "n", serde_json::json!(2)),
        ("/v1/responses", "top_logprobs", serde_json::json!(3)),
        ("/v1/chat/completions", "n", serde_json::Value::Null),
        ("/v1/responses", "n", serde_json::Value::Null),
    ] {
        let mut request = if path.ends_with("responses") {
            serde_json::json!({"model": "kimi-k3", "input": "hello"})
        } else {
            serde_json::json!({
                "model": "kimi-k3",
                "messages": [{"role": "user", "content": "hello"}]
            })
        };
        request[parameter] = value;
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
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
        assert_eq!(error["error"]["code"], "unsupported_model_capability");
        assert_eq!(error["error"]["param"], parameter);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mimo_text_responses_rejects_top_logprobs_before_egress() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Exercise both text targets because they share one model-specific registration helper.
    for model in ["mimo-v2.5", "mimo-v2.5-pro"] {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(
                AUTHORIZATION,
                "Bearer downstream-token-00000000000000000000000000000000",
            )
            .body(Body::from(
                serde_json::json!({
                    "model": model,
                    "input": "hello",
                    "seed": 7,
                    "stream": true,
                    "top_logprobs": 2,
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], "unsupported_model_capability");
        assert_eq!(error["error"]["param"], "top_logprobs");
    }

    // Keep Chat support visible while narrowing the Responses interface and avoiding all egress.
    assert!(transport.requests.lock().unwrap().is_empty());
    for model in ["mimo-v2.5", "mimo-v2.5-pro"] {
        let detail =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model}")).await;
        let chat = detail["interfaces"]["chat_completions"]["supported_parameters"]
            .as_array()
            .unwrap();
        let responses = detail["interfaces"]["responses"]["supported_parameters"]
            .as_array()
            .unwrap();
        assert!(chat.iter().any(|value| value == "top_logprobs"));
        assert!(!responses.iter().any(|value| value == "top_logprobs"));
    }
}

#[tokio::test]
async fn bailian_image_models_expose_only_probed_native_media_profiles() {
    const REMOTE_IMAGE_URL: &str = "https://example.com/image.jpeg";
    const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAF0lEQVR4nGP4z8BAEiJN9aiGUQ1DSgMAkPn/Afnh+ngAAAAASUVORK5CYII=";
    const JPEG_DATA_URL: &str = "data:image/jpeg;base64,/9j/2Q==";
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Admit one remote image through every image-capable Native interface.
    let cases = [
        ("qwen3.7-plus", "/v1/chat/completions"),
        ("qwen3.7-plus", "/v1/responses"),
        ("qwen3.8-max", "/v1/chat/completions"),
        ("qwen3.8-max", "/v1/responses"),
        ("qwen3.8-27b", "/v1/chat/completions"),
        ("qwen3.8-27b", "/v1/responses"),
        ("kimi-k3", "/v1/chat/completions"),
    ];
    for (model, path) in cases {
        let body = if path.ends_with("chat/completions") {
            serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": REMOTE_IMAGE_URL}},
                    {"type": "text", "text": "Describe the image."}
                ]}]
            })
        } else {
            serde_json::json!({
                "model": model,
                "input": [{"role": "user", "content": [
                    {"type": "input_image", "image_url": REMOTE_IMAGE_URL},
                    {"type": "input_text", "text": "Describe the image."}
                ]}]
            })
        };
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{model} {path}");
        let _ = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    }
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), cases.len());
        for (request, (_, path)) in requests.iter().zip(cases) {
            let forwarded = if path.ends_with("chat/completions") {
                &request.body["messages"][1]["content"][0]["image_url"]["url"]
            } else {
                &request.body["input"][0]["content"][0]["image_url"]
            };
            assert_eq!(forwarded, REMOTE_IMAGE_URL, "{path}");
        }
    }

    // Preserve both admitted inline MIME shapes through their native protocol wire.
    for (path, body) in [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "qwen3.7-plus",
                "messages": [{"role": "user", "content": [
                    {"type": "image_url", "image_url": {"url": PNG_DATA_URL}},
                    {"type": "text", "text": "Describe the image."}
                ]}]
            }),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "qwen3.8-27b",
                "input": [{"role": "user", "content": [
                    {"type": "input_image", "image_url": JPEG_DATA_URL},
                    {"type": "input_text", "text": "Describe the image."}
                ]}]
            }),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let _ = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    }
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(
            requests[cases.len()].body["messages"][1]["content"][0]["image_url"]["url"],
            PNG_DATA_URL
        );
        assert_eq!(
            requests[cases.len() + 1].body["input"][0]["content"][0]["image_url"],
            JPEG_DATA_URL
        );
    }

    // Expose only the single-image source and MIME shapes proven across the live matrix.
    let expected_image = serde_json::json!({
        "sources": ["remote_url", "data_url"],
        "media_types": ["image/jpeg", "image/png"],
        "detail": {"default": null, "allowed": []},
        "limits": {
            "max_parts": 1,
            "max_url_length": 8_192,
            "max_inline_encoded_bytes": 1024 * 1024,
            "max_inline_decoded_bytes": 768 * 1024,
            "max_total_inline_encoded_bytes": 1024 * 1024,
            "max_total_inline_decoded_bytes": 768 * 1024
        }
    });
    for model in ["qwen3.7-plus", "qwen3.8-max", "qwen3.8-27b"] {
        let detail =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model}")).await;
        for protocol in ["chat_completions", "responses"] {
            assert_eq!(
                detail["interfaces"][protocol]["multimodal_input"]["image"],
                expected_image
            );
        }
    }
    let kimi = compiled_authenticated_get(&app, "/openbridge/v1/models/kimi-k3").await;
    assert_eq!(
        kimi["interfaces"]["chat_completions"]["multimodal_input"]["image"],
        expected_image
    );
    assert!(kimi["interfaces"]["responses"]["multimodal_input"]["image"].is_null());

    // Keep the text-only canonical Qwen3.7 Max from inheriting the Provider image ceiling.
    let text_only = compiled_authenticated_get(&app, "/openbridge/v1/models/qwen3.7-max").await;
    for protocol in ["chat_completions", "responses"] {
        assert!(text_only["interfaces"][protocol]["multimodal_input"]["image"].is_null());
    }
}
