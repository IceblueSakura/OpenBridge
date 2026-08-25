//! Verifies ChatGPT OAuth2 generation routes, Bridge admission, and bounded 401 recovery.

use super::*;

#[tokio::test]
async fn chatgpt_oauth_routes_forward_five_models_with_account_bound_headers() {
    let directory = SyntheticAuthDirectory::new();
    let (document, access_token) = synthetic_chatgpt_document(1);
    fs::write(directory.auth_file(), document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {access_token}"),
        second_authorization: "Bearer unused-synthetic-token".to_owned(),
        replacement: Mutex::new(None),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    // Send one minimal streaming Responses request through each fixed ChatGPT Public Model.
    for public_model in [
        "gpt-5.3-codex-spark",
        "gpt-5.5",
        "gpt-5.6-luna",
        "gpt-5.6-terra",
        "gpt-5.6-sol",
    ] {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(
                AUTHORIZATION,
                "Bearer downstream-token-00000000000000000000000000000000",
            )
            .body(Body::from(
                serde_json::json!({
                    "model": public_model,
                    "input": "hello",
                    "stream": true,
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        assert!(
            body.windows(b"response.completed".len())
                .any(|window| { window == b"response.completed" })
        );
    }

    // Verify fixed endpoint/model rewriting and the complete non-FedRAMP OAuth request identity.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    for (request, upstream_model) in requests.iter().zip([
        "gpt-5.3-codex-spark",
        "gpt-5.5",
        "gpt-5.6-luna",
        "gpt-5.6-terra",
        "gpt-5.6-sol",
    ]) {
        assert_eq!(request.path, "/responses");
        assert_eq!(request.model, upstream_model);
        assert_eq!(
            request.instructions.as_deref(),
            Some(
                "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
            )
        );
        assert!(request.input_is_array);
        assert!(request.store_is_false);
        assert!(!request.output_limit_present);
        assert!(request.stream_is_true);
        assert_eq!(request.token_generation, SyntheticTokenGeneration::First);
        assert!(request.account_matches);
        assert!(request.originator_matches);
        assert!(request.user_agent_matches);
        assert!(request.accepts_sse);
        assert!(!request.fedramp_header_present);
    }
}

#[tokio::test]
async fn chatgpt_preserves_client_instructions_across_native_and_chat_bridge_paths() {
    let directory = SyntheticAuthDirectory::new();
    let (document, access_token) = synthetic_chatgpt_document(1);
    fs::write(directory.auth_file(), document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {access_token}"),
        second_authorization: "Bearer unused-synthetic-token".to_owned(),
        replacement: Mutex::new(None),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    for (path, body) in [
        (
            "/v1/responses",
            serde_json::json!({
                "model": "gpt-5.6-sol",
                "instructions": "  native client instruction  ",
                "input": "hello",
                "stream": true
            }),
        ),
        (
            "/v1/chat/completions",
            serde_json::json!({
                "model": "gpt-5.6-sol",
                "messages": [
                    {"role": "developer", "content": "  bridge client instruction  "},
                    {"role": "user", "content": "hello"}
                ],
                "stream": true
            }),
        ),
    ] {
        let request = Request::post(path)
            .header(CONTENT_TYPE, "application/json")
            .header(
                AUTHORIZATION,
                "Bearer downstream-token-00000000000000000000000000000000",
            )
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    }

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].instructions.as_deref(),
        Some("  native client instruction  ")
    );
    assert_eq!(
        requests[1].instructions.as_deref(),
        Some("  bridge client instruction  ")
    );
    assert!(requests.iter().all(|request| request.store_is_false));
    assert!(requests.iter().all(|request| request.input_is_array));
}

#[tokio::test]
async fn chatgpt_chat_requests_use_the_automatic_responses_to_chat_bridge() {
    let directory = SyntheticAuthDirectory::new();
    let (document, access_token) = synthetic_chatgpt_document(1);
    fs::write(directory.auth_file(), document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {access_token}"),
        second_authorization: "Bearer unused-synthetic-token".to_owned(),
        replacement: Mutex::new(None),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    // Send a streaming Chat request through the Responses-native ChatGPT target.
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(
            serde_json::json!({
                "model": "gpt-5.6-luna",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": true,
            })
            .to_string(),
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert!(
        body.windows(b"chat.completion.chunk".len())
            .any(|window| { window == b"chat.completion.chunk" })
    );
    assert!(
        body.windows(b"hello".len())
            .any(|window| window == b"hello")
    );
    assert!(
        body.windows(b"[DONE]".len())
            .any(|window| window == b"[DONE]")
    );

    // Verify the bridge kept the fixed Responses endpoint and ChatGPT request envelope.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.path, "/responses");
    assert_eq!(request.model, "gpt-5.6-luna");
    assert_eq!(
        request.instructions.as_deref(),
        Some(
            "You are a coding agent. Follow the user's instructions carefully and use the provided tools when needed."
        )
    );
    assert!(request.input_is_array);
    assert!(request.store_is_false);
    assert!(!request.output_limit_present);
    assert!(request.stream_is_true);
    assert_eq!(request.token_generation, SyntheticTokenGeneration::First);
    assert!(request.account_matches);
    assert!(request.originator_matches);
    assert!(request.user_agent_matches);
    assert!(request.accepts_sse);
    assert!(!request.fedramp_header_present);
}

#[tokio::test]
async fn chatgpt_chat_bridge_fulfills_hermes_stream_usage_contract() {
    let directory = SyntheticAuthDirectory::new();
    let (document, access_token) = synthetic_chatgpt_document(1);
    fs::write(directory.auth_file(), document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {access_token}"),
        second_authorization: "Bearer unused-synthetic-token".to_owned(),
        replacement: Mutex::new(None),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::from(
                    serde_json::json!({
                        "model": "gpt-5.3-codex-spark",
                        "messages": [{"role": "user", "content": "hello"}],
                        "stream": true,
                        "stream_options": {"include_usage": true}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(&body).unwrap();
    events.extend(decoder.finish().unwrap());
    assert_eq!(events.last().map(|event| event.data()), Some("[DONE]"));
    let chunks = events[..events.len() - 1]
        .iter()
        .map(|event| serde_json::from_str::<Value>(event.data()).unwrap())
        .collect::<Vec<_>>();
    assert!(
        chunks[..chunks.len() - 1]
            .iter()
            .all(|chunk| chunk["usage"].is_null())
    );
    assert_eq!(chunks.last().unwrap()["choices"], serde_json::json!([]));
    assert_eq!(
        chunks.last().unwrap()["usage"],
        serde_json::json!({"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2})
    );

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/responses");
    assert_eq!(requests[0].model, "gpt-5.3-codex-spark");
    assert!(requests[0].stream_is_true);
    assert!(!requests[0].stream_options_present);
}

#[tokio::test]
async fn chatgpt_buffers_streaming_responses_for_non_streaming_responses_and_chat() {
    let directory = SyntheticAuthDirectory::new();
    let (document, access_token) = synthetic_chatgpt_document(1);
    fs::write(directory.auth_file(), document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {access_token}"),
        second_authorization: "Bearer unused-synthetic-token".to_owned(),
        replacement: Mutex::new(None),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    // Convert the streaming-only upstream response into one Native Responses JSON document.
    let responses_request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(r#"{"model":"gpt-5.6-luna","input":"hello"}"#))
        .unwrap();
    let responses_response = app.clone().oneshot(responses_request).await.unwrap();
    assert_eq!(responses_response.status(), StatusCode::OK);
    assert_eq!(
        responses_response.headers()[CONTENT_TYPE],
        "application/json"
    );
    let responses_body = to_bytes(responses_response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let responses: Value = serde_json::from_slice(&responses_body).unwrap();
    assert_eq!(responses["object"], "response");
    assert_eq!(responses["model"], "gpt-5.6-luna");
    assert_eq!(responses["status"], "completed");
    assert_eq!(responses["output"][0]["type"], "reasoning");
    assert_eq!(responses["output"][1]["content"][0]["text"], "hello");
    assert_eq!(responses["usage"]["total_tokens"], 2);

    // Reuse the same bounded terminal snapshot through the existing non-streaming Chat Bridge.
    let chat_request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(
            r#"{"model":"gpt-5.6-luna","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .unwrap();
    let chat_response = app.oneshot(chat_request).await.unwrap();
    assert_eq!(chat_response.status(), StatusCode::OK);
    assert_eq!(chat_response.headers()[CONTENT_TYPE], "application/json");
    let chat_body = to_bytes(chat_response.into_body(), 64 * 1024)
        .await
        .unwrap();
    let chat: Value = serde_json::from_slice(&chat_body).unwrap();
    assert_eq!(chat["object"], "chat.completion");
    assert_eq!(chat["choices"][0]["message"]["content"], "hello");

    // Require both downstream non-streaming calls to use the upstream streaming envelope.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.stream_is_true));
}

#[tokio::test]
async fn chatgpt_first_401_reloads_changed_bundle_and_replays_once() {
    let directory = SyntheticAuthDirectory::new();
    let (first_document, first_access_token) = synthetic_chatgpt_document(1);
    let (second_document, second_access_token) = synthetic_chatgpt_document(2);
    fs::write(directory.auth_file(), first_document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {first_access_token}"),
        second_authorization: format!("Bearer {second_access_token}"),
        replacement: Mutex::new(Some((directory.auth_file(), second_document))),
        reject_after_replacement: false,
        requests: Mutex::new(Vec::new()),
    });
    let (app, manager) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    // Trigger one pre-output 401, guarded file reload, and one replay on the same fixed Route.
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(
            r#"{"model":"gpt-5.6-sol","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert!(
        body.windows(b"response.completed".len())
            .any(|window| { window == b"response.completed" })
    );

    // Confirm exactly two attempts and publication of the externally rotated generation.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].token_generation,
        SyntheticTokenGeneration::First
    );
    assert_eq!(
        requests[1].token_generation,
        SyntheticTokenGeneration::Second
    );
    let snapshot = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .expect("ChatGPT OAuth2 credential should remain configured");
    assert_eq!(snapshot.metadata().generation(), 2);
    assert_eq!(
        snapshot.status(),
        openbridge::oauth2_credentials::OAuth2CredentialStatus::Active
    );
}

#[tokio::test]
async fn chatgpt_second_401_stops_replay_and_requires_explicit_login() {
    let directory = SyntheticAuthDirectory::new();
    let (first_document, first_access_token) = synthetic_chatgpt_document(1);
    let (second_document, second_access_token) = synthetic_chatgpt_document(2);
    fs::write(directory.auth_file(), first_document).unwrap();
    let transport = Arc::new(ChatGptOAuthTransport {
        first_authorization: format!("Bearer {first_access_token}"),
        second_authorization: format!("Bearer {second_access_token}"),
        replacement: Mutex::new(Some((directory.auth_file(), second_document))),
        reject_after_replacement: true,
        requests: Mutex::new(Vec::new()),
    });
    let (app, manager) = app_with_chatgpt_oauth(transport.clone(), &directory.auth_file());

    // Reject both the original and reloaded generations through one downstream request.
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(
            AUTHORIZATION,
            "Bearer downstream-token-00000000000000000000000000000000",
        )
        .body(Body::from(
            r#"{"model":"gpt-5.6-sol","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "upstream_authentication_error");

    // Prove the request stopped after one replay and terminalized only the replayed generation.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].token_generation,
        SyntheticTokenGeneration::First
    );
    assert_eq!(
        requests[1].token_generation,
        SyntheticTokenGeneration::Second
    );
    let snapshot = manager
        .credential_for_provider(ProviderKind::ChatGpt)
        .expect("ChatGPT OAuth2 credential should remain configured");
    assert_eq!(snapshot.metadata().generation(), 2);
    assert_eq!(
        snapshot.status(),
        openbridge::oauth2_credentials::OAuth2CredentialStatus::ReauthRequired
    );
}
