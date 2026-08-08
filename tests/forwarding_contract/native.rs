//! Verifies same-protocol Native forwarding, trusted request preparation, and reasoning mapping.

use super::*;

#[tokio::test]
async fn provider_request_header_hook_overrides_user_agent_for_upstream() {
    let transport = Arc::new(RecordingTransport::default());
    let app = app_with_transport(transport.clone());
    for path in ["/v1/chat/completions", "/v1/responses"] {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .header(USER_AGENT, "openbridge-contract-client/1.0")
            .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
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
async fn egress_preparation_applies_the_selected_api_reasoning_level_mapping() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.models[0].supported_parameters = vec!["reasoning".to_owned()];
    definition.models[0].reasoning = ReasoningSupport::Supported;
    definition.models[0].reasoning_levels = vec![ReasoningLevel::XHigh];
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
            r#"{"model":"public-model","messages":[],"reasoning_effort":"xhigh"}"#,
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
