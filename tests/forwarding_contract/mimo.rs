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
            assert_eq!(&request.body, expected_body);
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
async fn mimo_asr_language_syntax_and_profile_rejections_remain_distinct() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let cases = [
        (serde_json::json!(42), "invalid_request_error"),
        (serde_json::json!("fr"), "unsupported_model_capability"),
    ];
    let transport = Arc::new(MimoAudioTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Keep malformed wire values separate from well-formed values outside the executable profile.
    for (language, expected_code) in cases {
        let body = serde_json::json!({
            "model": "mimo-v2.5-asr",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "input_audio",
                    "input_audio": {"data": WAV_DATA_URL, "format": "wav"}
                }]
            }],
            "asr_options": {"language": language}
        });
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
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(error["error"]["code"], expected_code);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mimo_text_models_forward_documented_json_object_for_both_protocols() {
    let cases = [
        (
            "/v1/chat/completions",
            "mimo-v2.5",
            serde_json::json!({"model":"mimo-v2.5","messages":[{"role":"system","content":"Return only JSON matching {\"result\": string}."},{"role":"user","content":"Set result to ok."}],"response_format":{"type":"json_object"}}),
        ),
        (
            "/v1/responses",
            "mimo-v2.5",
            serde_json::json!({"model":"mimo-v2.5","input":"Return only JSON matching {\"result\": string}; set result to ok.","text":{"format":{"type":"json_object"}}}),
        ),
        (
            "/v1/chat/completions",
            "mimo-v2.5-pro",
            serde_json::json!({"model":"mimo-v2.5-pro","messages":[{"role":"system","content":"Return only JSON matching {\"result\": string}."},{"role":"user","content":"Set result to ok."}],"response_format":{"type":"json_object"}}),
        ),
        (
            "/v1/responses",
            "mimo-v2.5-pro",
            serde_json::json!({"model":"mimo-v2.5-pro","input":"Return only JSON matching {\"result\": string}; set result to ok.","text":{"format":{"type":"json_object"}}}),
        ),
    ];
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Forward the documented JSON-object shape for both text models and Native protocols.
    for (path, model, body) in cases {
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
        let status = response.status();
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path} {model}: {}",
            String::from_utf8_lossy(&response_body)
        );
    }

    // Preserve the protocol-specific format parameter without gateway prompt rewriting.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].body["response_format"],
        serde_json::json!({"type":"json_object"})
    );
    assert_eq!(
        requests[1].body["text"]["format"],
        serde_json::json!({"type":"json_object"})
    );
    assert_eq!(
        requests[2].body["response_format"],
        serde_json::json!({"type":"json_object"})
    );
    assert_eq!(
        requests[3].body["text"]["format"],
        serde_json::json!({"type":"json_object"})
    );
}

#[tokio::test]
async fn mimo_text_models_forward_documented_auto_and_strict_function_tools() {
    let chat_tool = serde_json::json!({
        "type": "function",
        "function": {
            "name": "report_result",
            "parameters": {
                "type": "object",
                "properties": {"result": {"type": "string"}},
                "required": ["result"],
                "additionalProperties": false
            },
            "strict": true
        }
    });
    let responses_tool = serde_json::json!({
        "type": "function",
        "name": "report_result",
        "parameters": {
            "type": "object",
            "properties": {"result": {"type": "string"}},
            "required": ["result"],
            "additionalProperties": false
        },
        "strict": true
    });
    let mut cases = Vec::new();
    for model in ["mimo-v2.5", "mimo-v2.5-pro"] {
        cases.push((
            "/v1/chat/completions",
            serde_json::json!({"model":model,"messages":[{"role":"user","content":"Use report_result with result ok."}],"tools":[chat_tool.clone()],"tool_choice":"auto"}),
        ));
        cases.push((
            "/v1/responses",
            serde_json::json!({"model":model,"input":"Use report_result with result ok.","tools":[responses_tool.clone()],"tool_choice":"auto"}),
        ));
    }
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Forward only MiMo's documented auto choice with strict function schemas intact.
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
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }

    // Preserve the protocol-specific strict field and omit any parallel-call control.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for request in requests.iter() {
        assert_eq!(request.body["tool_choice"], "auto");
        if request.path.ends_with("/responses") {
            assert_eq!(request.body["tools"][0]["strict"], true);
        } else {
            assert_eq!(request.body["tools"][0]["function"]["strict"], true);
        }
        assert!(request.body.get("parallel_tool_calls").is_none());
    }
}

#[tokio::test]
async fn mimo_unreliable_tool_and_structured_output_combinations_fail_before_egress() {
    const WAV_DATA_URL: &str = "data:audio/wav;base64,UklGRg==";
    let chat_tool = serde_json::json!({
        "type": "function",
        "function": {"name": "report_result", "parameters": {"type": "object"}}
    });
    let responses_tool = serde_json::json!({
        "type": "function",
        "name": "report_result",
        "parameters": {"type": "object"}
    });
    let mut cases = Vec::new();

    // Reject every tool-choice value that MiMo documents as stripped back to auto.
    let tool_choices = [
        (serde_json::json!("none"), serde_json::json!("none")),
        (serde_json::json!("required"), serde_json::json!("required")),
        (
            serde_json::json!({"type":"function","function":{"name":"report_result"}}),
            serde_json::json!({"type":"function","name":"report_result"}),
        ),
    ];
    for model in ["mimo-v2.5", "mimo-v2.5-pro"] {
        for (chat_choice, responses_choice) in &tool_choices {
            cases.push((
                "/v1/chat/completions",
                serde_json::json!({"model":model,"messages":[{"role":"user","content":"call the tool"}],"tools":[chat_tool.clone()],"tool_choice":chat_choice}),
            ));
            cases.push((
                "/v1/responses",
                serde_json::json!({"model":model,"input":"call the tool","tools":[responses_tool.clone()],"tool_choice":responses_choice}),
            ));
        }

        // Keep the control rejected on MiMo Pro, whose exact target has not been verified.
        if model == "mimo-v2.5-pro" {
            cases.push((
                "/v1/chat/completions",
                serde_json::json!({"model":model,"messages":[{"role":"user","content":"call the tool"}],"tools":[chat_tool.clone()],"tool_choice":"auto","parallel_tool_calls":true}),
            ));
            cases.push((
                "/v1/responses",
                serde_json::json!({"model":model,"input":"call the tool","tools":[responses_tool.clone()],"tool_choice":"auto","parallel_tool_calls":true}),
            ));
        }
    }

    // Reject unsupported structured modes and text constraints on dedicated audio tasks.
    cases.extend([
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5","messages":[{"role":"user","content":"return JSON"}],"response_format":{"type":"json_schema","json_schema":{"name":"result","schema":{"type":"object"}}}}),
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"mimo-v2.5","input":"return JSON","text":{"format":{"type":"json_schema","name":"result","schema":{"type":"object"}}}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-pro","messages":[{"role":"user","content":"return JSON"}],"response_format":{"type":"json_schema","json_schema":{"name":"result","schema":{"type":"object"}}}}),
        ),
        (
            "/v1/responses",
            serde_json::json!({"model":"mimo-v2.5-pro","input":"return JSON","text":{"format":{"type":"json_schema","name":"result","strict":true,"schema":{"type":"object"}}}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-asr","messages":[{"role":"user","content":[{"type":"input_audio","input_audio":{"data":WAV_DATA_URL,"format":"wav"}}]}],"asr_options":{"language":"zh"},"response_format":{"type":"json_object"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts","messages":[{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":"mimo_default"},"response_format":{"type":"json_object"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voicedesign","messages":[{"role":"user","content":"a warm voice"},{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav"},"response_format":{"type":"json_object"}}),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({"model":"mimo-v2.5-tts-voiceclone","messages":[{"role":"assistant","content":"hello"}],"modalities":["text","audio"],"audio":{"format":"wav","voice":WAV_DATA_URL},"response_format":{"type":"json_object"}}),
        ),
    ]);
    let transport = Arc::new(MimoAudioTransport::default());
    let app = app_with_compiled_registry(transport.clone());

    // Reject every overclaimed combination through the public Router before trusted transport.
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

#[tokio::test]
async fn mimo_responses_native_preserves_parallel_tool_control_and_multi_tool_stream() {
    // Build the actual compiled MiMo Route and a multi-tool request using the verified parallel control.
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

    // Verify that the returned multi-call stream remains valid independently of the accepted control.
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

    // Confirm that the request preserves the explicitly requested parallel-call switch.
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
