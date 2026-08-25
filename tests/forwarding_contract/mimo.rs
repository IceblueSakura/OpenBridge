//! Verifies MiMo Native image and parallel-tool forwarding boundaries.

use super::*;

#[tokio::test]
async fn mimo_native_image_inputs_are_preserved_for_both_protocols() {
    const IMAGE_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Zr4sAAAAASUVORK5CYII=";
    const REMOTE_IMAGE_URL: &str = "https://example.com/image.png";

    // Submit data-URL JSON and remote-URL SSE requests to both production Native interfaces.
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
            Some("chat.completion"),
            None,
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
            Some("response"),
            None,
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "stream": true,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "image_url", "image_url": {"url": REMOTE_IMAGE_URL}},
                        {"type": "text", "text": "Name the colors."}
                    ]
                }]
            }),
            None,
            Some(MIMO_CHAT_IMAGE_STREAM),
        ),
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "stream": true,
                "input": [{
                    "role": "user",
                    "content": [
                        {"type": "input_image", "image_url": REMOTE_IMAGE_URL},
                        {"type": "input_text", "text": "Name the colors."}
                    ]
                }]
            }),
            None,
            Some(MIMO_RESPONSES_IMAGE_STREAM),
        ),
    ];
    for (path, body, expected_object, expected_stream) in &cases {
        let response = app
            .clone()
            .oneshot(
                Request::post(*path)
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
        if let Some(expected_stream) = expected_stream {
            assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
            let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
            assert_eq!(body.as_ref(), *expected_stream);

            // Parse the complete Native SSE lifecycle and require one successful terminal.
            let mut decoder = SseDecoder::new(64 * 1024);
            let mut events = decoder.push(&body).unwrap();
            events.extend(decoder.finish().unwrap());
            if path.ends_with("/responses") {
                let mut state = ResponsesStreamState::new();
                for event in events {
                    state.ingest(&event).unwrap();
                }
                state.finish().unwrap();
                assert_eq!(state.text(), "red and blue");
                assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
            } else {
                let mut state = ChatStreamState::new();
                for event in events {
                    state.ingest(&event).unwrap();
                }
                state.finish().unwrap();
                assert_eq!(state.text(), "red and blue");
                assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
            }
        } else {
            let response: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(response["object"], expected_object.unwrap());
            assert_eq!(response["model"], "mimo-v2.5");
        }
    }

    // Compare every complete prepared body so no role, control field, or nested part can drift.
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        for (request, (expected_path, expected_body, _, _)) in requests.iter().zip(&cases) {
            assert_eq!(request.path, *expected_path);
            let mut expected_body = expected_body.clone();
            if expected_path.ends_with("chat/completions") {
                expected_body["messages"].as_array_mut().unwrap().insert(
                    0,
                    serde_json::json!({
                        "role": "system",
                        "content": "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
                    }),
                );
            } else {
                expected_body["instructions"] = serde_json::json!(
                    "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
                );
                expected_body["store"] = serde_json::json!(false);
            }
            assert_eq!(request.body, expected_body);
        }
    }

    // Expose one typed image contract per Native interface without leaking deployment topology.
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5").await;
    for protocol in ["chat_completions", "responses"] {
        assert_eq!(
            model["interfaces"][protocol]["multimodal_input"]["image"],
            serde_json::json!({
                "sources": ["remote_url", "data_url"],
                "media_types": [
                    "image/jpeg",
                    "image/png",
                    "image/gif",
                    "image/webp",
                    "image/bmp"
                ],
                "detail": {"default": null, "allowed": []},
                "limits": {
                    "max_parts": 64,
                    "max_url_length": 8_192,
                    "max_inline_encoded_bytes": (50 * 1024 * 1024),
                    "max_inline_decoded_bytes": (38 * 1024 * 1024),
                    "max_total_inline_encoded_bytes": (50 * 1024 * 1024),
                    "max_total_inline_decoded_bytes": (38 * 1024 * 1024)
                }
            })
        );
    }

    // Keep the text-only Pro target from inheriting the Provider-wide V2.5 image ceiling.
    let pro = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5-pro").await;
    for protocol in ["chat_completions", "responses"] {
        assert!(pro["interfaces"][protocol]["multimodal_input"]["image"].is_null());
    }
}

#[tokio::test]
async fn mimo_v25_chat_audio_understanding_preserves_bounded_wav_data_url() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let cases = [
        serde_json::json!({
            "model": "mimo-v2.5",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}},
                    {"type": "text", "text": "Describe the audio."}
                ]
            }]
        }),
        serde_json::json!({
            "model": "mimo-v2.5",
            "stream": true,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}},
                    {"type": "text", "text": "Describe the audio."}
                ]
            }]
        }),
    ];
    let transport = Arc::new(MimoAudioUnderstandingTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Exercise the same fixed Chat Native interface through JSON and SSE delivery.
    for body in &cases {
        let streaming = body.get("stream").and_then(Value::as_bool) == Some(true);
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(serde_json::to_vec(body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{body}");
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        if streaming {
            assert_eq!(response_body.as_ref(), MIMO_CHAT_AUDIO_UNDERSTANDING_STREAM);
            let mut decoder = SseDecoder::new(64 * 1024);
            let mut events = decoder.push(&response_body).unwrap();
            events.extend(decoder.finish().unwrap());
            let mut state = ChatStreamState::new();
            for event in events {
                state.ingest(&event).unwrap();
            }
            state.finish().unwrap();
            assert_eq!(state.text(), "understood audio");
            assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
        } else {
            let response: Value = serde_json::from_slice(&response_body).unwrap();
            assert_eq!(response["object"], "chat.completion");
            assert_eq!(response["model"], "mimo-v2.5");
            assert_eq!(
                response["choices"][0]["message"]["content"],
                "understood audio"
            );
        }
    }

    // Preserve the complete mixed-part body while applying the one general-generation instruction policy.
    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), cases.len());
        for (request, expected) in requests.iter().zip(&cases) {
            assert_eq!(request.path, "/v1/chat/completions");
            let mut expected = expected.clone();
            expected["messages"].as_array_mut().unwrap().insert(
                0,
                serde_json::json!({
                    "role": "system",
                    "content": "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
                }),
            );
            assert_eq!(request.body, expected);
        }
    }

    // Project the exact understanding profile only on Chat; Responses remains audio-closed.
    let model = compiled_authenticated_get(&app, "/openbridge/v1/models/mimo-v2.5").await;
    assert_eq!(
        model["interfaces"]["chat_completions"]["audio_task"],
        "content_understanding"
    );
    assert_eq!(
        model["interfaces"]["chat_completions"]["multimodal_input"]["audio"],
        serde_json::json!({
            "sources": ["data_url"],
            "formats": ["wav"],
            "limits": {
                "max_parts": 1,
                "max_url_length": 0,
                "max_inline_encoded_bytes": (10 * 1024 * 1024),
                "max_inline_decoded_bytes": (8 * 1024 * 1024),
                "max_total_inline_encoded_bytes": (10 * 1024 * 1024),
                "max_total_inline_decoded_bytes": (8 * 1024 * 1024)
            }
        })
    );
    assert!(model["interfaces"]["responses"]["audio_task"].is_null());
    assert!(model["interfaces"]["responses"]["multimodal_input"]["audio"].is_null());
}

#[tokio::test]
async fn mimo_v25_audio_understanding_rejections_fail_before_egress() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    const MP3_DATA_URL: &str = "data:audio/mpeg;base64,UklGRg==";
    let valid_parts = serde_json::json!([
        {"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}},
        {"type": "text", "text": "Describe the audio."}
    ]);
    let cases = [
        (
            "/v1/responses",
            serde_json::json!({
                "model": "mimo-v2.5",
                "input": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}}]}]
            }),
            "unimplemented_request",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model": "mimo-v2.5-pro", "messages": [{"role": "user", "content": valid_parts.clone()}]}),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [
                    {"type": "input_audio", "input_audio": {"data": "https://example.com/audio.wav", "format": "wav"}},
                    {"type": "text", "text": "Describe the audio."}
                ]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [
                    {"type": "input_audio", "input_audio": {"data": "UklGRg==", "format": "wav"}},
                    {"type": "text", "text": "Describe the audio."}
                ]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [
                    {"type": "input_audio", "input_audio": {"data": MP3_DATA_URL, "format": "mp3"}},
                    {"type": "text", "text": "Describe the audio."}
                ]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": [
                    {"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}},
                    {"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}},
                    {"type": "text", "text": "Describe the audio."}
                ]}]
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "user", "content": valid_parts.clone()}],
                "asr_options": {"language": "zh"}
            }),
            "unsupported_model_capability",
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "mimo-v2.5",
                "messages": [{"role": "assistant", "content": "Speak this text."}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav", "voice": "mimo_default"}
            }),
            "unsupported_model_capability",
        ),
    ];
    let transport = Arc::new(MimoAudioUnderstandingTransport::default());
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
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path} {body}");
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["code"], expected_code, "{path} {body}");
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mimo_audio_models_are_chat_native_and_keep_task_specific_wire() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    const ASR_JSON: &str = r#"{"id":"chat_asr","object":"chat.completion","model":"audio-model","choices":[{"index":0,"message":{"role":"assistant","content":"transcript"},"finish_reason":"stop"}]}"#;
    const GENERATED_JSON: &str = r#"{"id":"chat_audio","object":"chat.completion","model":"audio-model","choices":[{"index":0,"message":{"role":"assistant","content":null,"audio":{"data":"UklGRg=="}},"finish_reason":"stop"}]}"#;
    const ASR_SSE: &str = "data: {\"id\":\"chat_asr\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"transcript\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_asr\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    const GENERATED_SSE: &str = "data: {\"id\":\"chat_audio\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"audio\":{\"data\":\"UklGRg==\"}},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_audio\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
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
            "mimo-v2.5-asr",
            serde_json::json!({
                "model": "mimo-v2.5-asr",
                "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}}]}]
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
            "mimo-v2.5-tts",
            serde_json::json!({
                "model": "mimo-v2.5-tts",
                "messages": [{"role": "assistant", "content": "hello without a preset voice"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav"}
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
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let expected = if *model == "mimo-v2.5-asr" {
            ASR_JSON
        } else {
            GENERATED_JSON
        };
        assert_eq!(response_body.as_ref(), expected.as_bytes(), "{model}");
    }

    {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), cases.len());
        for ((model, body), request) in cases.iter().zip(requests.iter()) {
            assert_eq!(request.path, "/v1/chat/completions");
            assert_eq!(request.body["model"], *model);
            assert_eq!(request.body, *body);
        }
    }

    // Expose only task-specific Chat interfaces; no Responses or Bridge surface is created.
    for (model, expected_task) in [
        ("mimo-v2.5-asr", "speech_recognition"),
        ("mimo-v2.5-tts", "speech_synthesis"),
        ("mimo-v2.5-tts-voicedesign", "voice_design"),
        ("mimo-v2.5-tts-voiceclone", "voice_clone"),
    ] {
        let info =
            compiled_authenticated_get(&app, &format!("/openbridge/v1/models/{model}")).await;
        assert_eq!(
            info["capabilities"]["tasks"],
            serde_json::json!([expected_task]),
            "{model} must expose only its canonical task"
        );
        assert!(info["interfaces"]["chat_completions"].is_object());
        assert!(info["interfaces"]["responses"].is_null());
        assert!(
            info["interfaces"]["chat_completions"]
                .as_object()
                .is_some_and(|interface| !interface.contains_key("audio")),
            "{model} must not serialize the private audio profile union"
        );
        assert_eq!(
            info["interfaces"]["chat_completions"]["tools"]["support"],
            "unsupported"
        );
        let supported_parameters = info["interfaces"]["chat_completions"]["supported_parameters"]
            .as_array()
            .unwrap();
        for parameter in ["tools", "tool_choice", "parallel_tool_calls"] {
            assert!(
                supported_parameters
                    .iter()
                    .all(|value| value.as_str() != Some(parameter)),
                "{model} must not expose {parameter}"
            );
        }
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

    // Preserve exact task-specific request bodies and one terminal Chat SSE lifecycle.
    let stream_cases = [
        (
            serde_json::json!({
                "model": "mimo-v2.5-asr",
                "stream": true,
                "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": WAV_DATA_URL, "format": "wav"}}]}],
                "asr_options": {"language": "zh"}
            }),
            ASR_SSE,
        ),
        (
            serde_json::json!({
                "model": "mimo-v2.5-tts",
                "stream": true,
                "messages": [{"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "pcm16", "voice": "mimo_default"}
            }),
            GENERATED_SSE,
        ),
        (
            serde_json::json!({
                "model": "mimo-v2.5-tts-voicedesign",
                "stream": true,
                "messages": [{"role": "user", "content": "a warm low voice"}, {"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "pcm16"}
            }),
            GENERATED_SSE,
        ),
        (
            serde_json::json!({
                "model": "mimo-v2.5-tts-voiceclone",
                "stream": true,
                "messages": [{"role": "assistant", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "pcm16", "voice": WAV_DATA_URL}
            }),
            GENERATED_SSE,
        ),
    ];
    for (stream_body, expected_response) in &stream_cases {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/chat/completions")
                    .header(CONTENT_TYPE, "application/json")
                    .header(
                        AUTHORIZATION,
                        "Bearer downstream-token-00000000000000000000000000000000",
                    )
                    .body(Body::from(serde_json::to_vec(stream_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        assert_eq!(response_body.as_ref(), expected_response.as_bytes());
    }

    // Confirm the transport received every SSE request unchanged and in configured order.
    let requests = transport.requests.lock().unwrap();
    let stream_requests = &requests[cases.len()..];
    assert_eq!(stream_requests.len(), stream_cases.len());
    for ((expected, _), request) in stream_cases.iter().zip(stream_requests) {
        assert_eq!(request.path, "/v1/chat/completions");
        assert_eq!(&request.body, expected);
    }
}

#[tokio::test]
async fn mimo_audio_task_mismatches_fail_before_egress() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let function_tool = serde_json::json!({
        "type": "function",
        "function": {
            "name": "report_result",
            "parameters": {"type": "object"}
        }
    });
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
        // Reject an extra empty or wrong-role message for every specialist task.
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-asr","messages":[{"role":"user","content":[{"type":"input_audio","input_audio":{"data":WAV_DATA_URL,"format":"wav"}}]},{"role":"assistant","content":null}]}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts","messages":[{"role":"assistant","content":"hello"},{"role":"system","content":null}],"modalities":["text","audio"],"audio":{"format":"wav","voice":"mimo_default"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voicedesign","messages":[{"role":"user","content":"a warm voice"},{"role":"assistant","content":"hello"},{"role":"assistant","content":null}],"modalities":["text","audio"],"audio":{"format":"wav"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voiceclone","messages":[{"role":"system","content":null},{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":WAV_DATA_URL}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5","messages":[{"role":"user","content":"hello"}],"asr_options":{"language":"zh"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-asr","messages":[{"role":"user","content":[{"type":"input_audio","input_audio":{"data":WAV_DATA_URL,"format":"wav"}}]}],"asr_options":{"language":"en"},"tools":[function_tool.clone()],"tool_choice":"required"}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts","messages":[{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":"mimo_default"},"tools":[function_tool.clone()],"tool_choice":"required"}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voicedesign","messages":[{"role":"user","content":"a warm voice"},{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav"},"tools":[function_tool.clone()],"tool_choice":"required"}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voiceclone","messages":[{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":WAV_DATA_URL},"tools":[function_tool.clone()],"tool_choice":"required"}),
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
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(
            error["error"]["code"], "unsupported_model_capability",
            "{path} {body}"
        );
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
