//! Verifies MiMo Native image and parallel-tool forwarding boundaries.

use super::*;

#[tokio::test]
async fn mimo_native_image_inputs_are_preserved_for_both_protocols() {
    const IMAGE_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Zr4sAAAAASUVORK5CYII=";

    // Use the production MiMo registry and submit one protocol-native image request to each endpoint.
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let cases = [
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "image_url", "image_url": {"url": IMAGE_DATA_URL}},
                        {"type": "text", "text": "Name the colors."}
                    ]
                }]
            }),
            "chat.completion",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{
                    "role": "user",
                    "content": [
                        {"type": "input_image", "image_url": IMAGE_DATA_URL},
                        {"type": "input_text", "text": "Name the colors."}
                    ]
                }]
            }),
            "response",
        ),
    ];
    for (path, body, expected_object) in cases {
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
        assert_eq!(response.status(), StatusCode::OK);
        let response: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(response["object"], expected_object);
        assert_eq!(response["model"], "mimo-v2.5");
    }

    // Verify that model binding changes no content part, order, source, or protocol-specific nesting.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(requests[0].body["model"], "mimo-v2.5");
    assert_eq!(
        requests[0].body["messages"][0]["content"],
        serde_json::json!([
            {"type": "image_url", "image_url": {"url": IMAGE_DATA_URL}},
            {"type": "text", "text": "Name the colors."}
        ])
    );
    assert_eq!(requests[1].path, "/v1/responses");
    assert_eq!(requests[1].body["model"], "mimo-v2.5");
    assert_eq!(
        requests[1].body["input"][0]["content"],
        serde_json::json!([
            {"type": "input_image", "image_url": IMAGE_DATA_URL},
            {"type": "input_text", "text": "Name the colors."}
        ])
    );

    // Expose one typed image contract per Native interface without leaking deployment topology.
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5").await;
    for protocol in ["chat_completions", "responses"] {
        assert_eq!(
            model["interfaces"][protocol]["multimodal_input"]["image"]["sources"],
            serde_json::json!(["remote_url", "data_url"])
        );
        assert_eq!(
            model["interfaces"][protocol]["multimodal_input"]["image"]["media_types"],
            serde_json::json!([
                "image/jpeg",
                "image/png",
                "image/gif",
                "image/webp",
                "image/bmp"
            ])
        );
    }

    // Keep the text-only Pro target from inheriting the Provider-wide V2.5 image ceiling.
    let pro = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5-pro").await;
    for protocol in ["chat_completions", "responses"] {
        assert!(pro["interfaces"][protocol]["multimodal_input"]["image"].is_null());
    }
}

#[tokio::test]
async fn mimo_audio_models_are_chat_native_and_keep_task_specific_wire() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let cases = [
        (
            "mimo-v2.5-asr",
            serde_json::json!({
                "model": "mimo-v2.5-asr",
                "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}}]}],
                "asr_options": {"language": "zh"}
            }),
        ),
        (
            "mimo-v2.5-tts",
            serde_json::json!({
                "model": "mimo-v2.5-tts",
                "messages": [{"role": "user", "content": "calm"}, {"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav", "voice": "mimo_default"}
            }),
        ),
        (
            "mimo-v2.5-tts-voicedesign",
            serde_json::json!({
                "model": "mimo-v2.5-tts-voicedesign",
                "messages": [{"role": "user", "content": "a warm low voice"}, {"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav"}
            }),
        ),
        (
            "mimo-v2.5-tts-voiceclone",
            serde_json::json!({
                "model": "mimo-v2.5-tts-voiceclone",
                "messages": [{"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav", "voice": WAV_DATA_URL}
            }),
        ),
    ];
    let transport = Arc::new(MimoAudioTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    for (model, body) in cases.iter() {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
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
        assert_eq!(response.status(), StatusCode::OK, "{model}");
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), cases.len());
    for ((model, body), request) in cases.iter().zip(requests.iter()) {
        assert_eq!(request.path, "/v1/chat/completions");
        assert_eq!(request.body["model"], *model);
        assert_eq!(request.body, *body);
    }
    drop(requests);

    // Expose only task-specific Chat interfaces; no Responses or Bridge surface is created.
    for model in [
        "mimo-v2.5-asr",
        "mimo-v2.5-tts",
        "mimo-v2.5-tts-voicedesign",
        "mimo-v2.5-tts-voiceclone",
    ] {
        let info =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model}")).await;
        assert!(info["interfaces"]["chat_completions"].is_object());
        assert!(info["interfaces"]["responses"].is_null());
    }
    let asr = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5-asr").await;
    assert_eq!(asr["interfaces"]["chat_completions"]["audio_task"], "asr");
    assert_eq!(
        asr["interfaces"]["chat_completions"]["multimodal_input"]["audio"]["formats"],
        serde_json::json!(["wav"])
    );
    let tts = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5-tts").await;
    assert_eq!(tts["interfaces"]["chat_completions"]["audio_task"], "tts");
    assert_eq!(
        tts["interfaces"]["chat_completions"]["multimodal_output"]["audio"]["streaming_formats"],
        serde_json::json!(["pcm16"])
    );
    let clone =
        compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5-tts-voiceclone").await;
    assert_eq!(
        clone["interfaces"]["chat_completions"]["audio_task"],
        "voice_clone"
    );
    assert_eq!(
        clone["interfaces"]["chat_completions"]["multimodal_input"]["voice_conditioning"]["sources"],
        serde_json::json!(["data_url"])
    );

    // The streaming profile accepts PCM16 only and preserves Chat SSE framing.
    let stream_body = serde_json::json!({
        "model": "mimo-v2.5-tts",
        "stream": true,
        "messages": [{"role": "assistant", "content": "hello"}],
        "modalities": ["text", "audio"],
        "audio": {"format": "pcm16", "voice": "mimo_default"}
    });
    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(serde_json::to_vec(&stream_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("UklGRg=="));
}

#[tokio::test]
async fn mimo_audio_task_mismatches_fail_before_egress() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let cases = [
        (
            "/v1/responses",
            serde_json::json!({"model":"mimo-v2.5-asr","input":"audio"}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-asr","messages":[{"role":"user","content":[{"type":"input_audio","input_audio":{"data":WAV_DATA_URL,"format":"wav"}},{"type":"text","text":"also answer"}]}]}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts","messages":[{"role":"user","content":"style"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":"mimo_default"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voiceclone","messages":[{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5","messages":[{"role":"user","content":"hello"}],"asr_options":{"language":"zh"}}),
        ),
    ];
    let transport = Arc::new(MimoAudioTransport::default());
    let app = app_with_compiled_registry(transport.clone());
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
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path} {body}");
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mimo_invalid_unsupported_and_oversized_images_fail_before_egress() {
    const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgo=";

    // Build malformed, unsupported, and locally oversized cases for both Native protocols.
    let too_many_images = (0..65)
        .map(|_| serde_json::json!({"type": "image_url", "image_url": {"url": PNG_DATA_URL}}))
        .collect::<Vec<_>>();
    let cases = vec![
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "system", "content": [{"type": "image_url", "image_url": {"url": PNG_DATA_URL}}]}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"type": "input_image", "image_url": PNG_DATA_URL}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{"type": "input_image", "file_id": "file_synthetic"}]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": "http://example.invalid/image.png"}}]}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": "https://127.0.0.1/image.png"}}]}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{"type": "input_image", "image_url": "data:image/png;base64,not-base64"}]}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{"type": "input_image", "image_url": "data:image/png;base64,ZE=="}]}]
            }),
            "invalid_request_error",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{"type": "input_image", "image_url": "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="}]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": PNG_DATA_URL, "detail": "auto"}}]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": too_many_images}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{
                    "type": "input_image",
                    "image_url": format!("https://example.invalid/{}", "a".repeat(8_192))
                }]}]
            }),
            "unsupported_model_capability",
        ),
    ];

    // Submit every case through the production Router and verify its stable public classification.
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    for (path, body, expected_code) in cases {
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
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], expected_code, "{body}");
    }

    // No rejected image body may reach the trusted MiMo transport boundary.
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mimo_responses_native_preserves_parallel_tool_stream() {
    // Build the actual compiled MiMo Route and a parallel tool request supported by both Native and Bridge.
    let transport = Arc::new(MimoResponsesToolStreamTransport::default());
    let app = app_with_compiled_registry(transport.clone());
    let request_body = r#"{
        "model":"mimo-v2.5",
        "input":"查天气和时间",
        "stream":true,
        "parallel_tool_calls":true,
        "tools":[
            {"type":"function","name":"lookup_weather","parameters":{"type":"object","properties":{"city":{"type":"string"}}}},
            {"type":"function","name":"lookup_time","parameters":{"type":"object","properties":{"tz":{"type":"string"}}}}
        ]
    }"#;

    // Submit a streaming Responses request and read the complete body returned by the gateway.
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
    assert_eq!(response.headers()["openai-request-id"], "mimo-id");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(body.as_ref(), MIMO_RESPONSES_PARALLEL_TOOL_STREAM);

    // Use the Responses state machine to verify that interleaved arguments still reconstruct two independent function calls.
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    let mut state = ResponsesStreamState::new();
    for event in events {
        state.ingest(&event).unwrap();
    }
    state.finish().unwrap();
    assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
    let tool_calls = state.tool_calls();
    assert_eq!(tool_calls.len(), 2);
    assert_eq!(tool_calls[0].call_id(), "call_0");
    assert_eq!(tool_calls[0].name(), "lookup_weather");
    assert_eq!(tool_calls[0].arguments(), r#"{"city":"Shanghai"}"#);
    assert_eq!(tool_calls[1].call_id(), "call_1");
    assert_eq!(tool_calls[1].name(), "lookup_time");
    assert_eq!(tool_calls[1].arguments(), r#"{"tz":"Asia/Shanghai"}"#);

    // Confirm that the request still uses the MiMo Responses endpoint and preserves shared capabilities and model name.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/responses");
    assert_eq!(requests[0].authorization, "Bearer upstream-token");
    assert_eq!(requests[0].body["model"], "mimo-v2.5");
    assert_eq!(requests[0].body["stream"], true);
    assert_eq!(requests[0].body["parallel_tool_calls"], true);
    assert_eq!(requests[0].body["input"], "查天气和时间");
    assert_eq!(requests[0].body["tools"].as_array().unwrap().len(), 2);
}
