//! One real HTTP Router smoke and a bootstrap process gate; resource/failure
//! mechanisms live at their owners.
#[allow(dead_code)]
#[path = "support/chat_profile.rs"]
mod chat_wire;
#[allow(dead_code)]
#[path = "support/responses_profile.rs"]
mod responses_wire;
#[path = "support/gateway.rs"]
mod support;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use openbridge::{
    adapter::{Adapter, Dialect},
    gateway::Limits,
    lowering::generation::GenerationRepresentationContract,
    protocol::openai::{DecodedResponse, Profile, sse::ResponsesSseDecoder},
    semantic::{
        task::generation::{Continuation, Outcome},
        value::ReplayOrigin,
    },
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot};

#[derive(Clone, Default)]
struct Upstream(Arc<Mutex<Vec<Value>>>);
struct Guard(tokio::task::JoinHandle<()>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn answer(
    State(state): State<Upstream>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Value>,
) -> Response {
    assert_eq!(
        headers["authorization"],
        "Bearer synthetic-upstream-credential-0001"
    );
    assert!(!headers.contains_key("x-never-forward"));
    assert_eq!(request["model"], "private-model");
    let chat = request.get("messages").is_some();
    let cap = if chat {
        "max_completion_tokens"
    } else {
        "max_output_tokens"
    };
    assert_eq!(
        request[cap], 32,
        "trusted default must pass through IR into upstream wire"
    );
    let probability_case = request["metadata"]["case"] == "probabilities";
    if probability_case {
        assert!(chat);
        assert_eq!(request["service_tier"], "fast");
        assert_eq!(request["logprobs"], true);
        assert_eq!(request["top_logprobs"], 1);
    }
    let probability = |token: &str| json!({"token":token,"logprob":-0.5,"bytes":token.as_bytes(),"top_logprobs":[]});
    let tool_image_case = request["metadata"]["case"] == "tool-images";
    if tool_image_case {
        assert!(!chat);
        assert_eq!(
            request["input"],
            json!([
                {"type":"function_call","call_id":"media-call","name":"lookup","arguments":"{}"},
                {"type":"function_call_output","call_id":"media-call","output":[
                    {"type":"input_text","text":"caption"},
                    {"type":"input_image","image_url":"data:image/png;base64,AQID"},
                    {"type":"input_image","image_url":"https://example.test/tool.png","detail":"low"}
                ]}
            ])
        );
    }
    let image_case = !tool_image_case && request.to_string().contains("data:image/png;base64,AQID");
    if image_case {
        let expected = if chat {
            json!([
                {"type":"text","text":"first"},
                {"type":"image_url","image_url":{"url":"data:image/png;base64,AQID"}},
                {"type":"text","text":"second"},
                {"type":"image_url","image_url":{"url":"https://example.test/synthetic.png","detail":"low"}}
            ])
        } else {
            json!([
                {"type":"input_text","text":"first"},
                {"type":"input_image","image_url":"data:image/png;base64,AQID"},
                {"type":"input_text","text":"second"},
                {"type":"input_image","image_url":"https://example.test/synthetic.png","detail":"low"}
            ])
        };
        assert_eq!(
            request[if chat { "messages" } else { "input" }][0]["content"],
            expected
        );
    }
    let turn = if probability_case
        || image_case
        || request.to_string().contains("tool_call_id")
        || request.to_string().contains("function_call_output")
    {
        2
    } else {
        1
    };
    if !chat && !tool_image_case && request.to_string().contains("function_call_output") {
        let history = request["input"].as_array().unwrap();
        for expected in [
            json!({"type":"function_call_output","call_id":"c_lookup","output":"{\"n\":1}"}),
            json!({"type":"custom_tool_call_output","call_id":"c_sql","output":"1"}),
        ] {
            assert!(history.contains(&expected));
        }
        assert!(
            history
                .iter()
                .any(|i| i["type"] == "reasoning"
                    && i["encrypted_content"] == "synthetic-final-token")
        );
        assert!(history.iter().any(|i| i["type"] == "function_call"
            && i["call_id"] == "c_lookup"
            && i["arguments"] == "{\"n\":1}"));
    }
    state.0.lock().unwrap().push(request.clone());
    let stream = request["stream"] == true;
    let mut output = if chat {
        chat_wire::response(turn)
    } else {
        responses_wire::response(turn)
    };
    output["model"] = json!("private-model");
    if probability_case {
        output["choices"][0]["logprobs"] =
            json!({"content":[probability("old "),probability("🧪")]});
        output["service_tier"] = json!("default");
        output["metadata"] = json!({"provider":"reported"});
    }
    if !chat {
        output["created_at"] = json!(1);
        output["completed_at"] = json!(2);
    }
    let bytes = if stream {
        let mut frames = if chat {
            chat_wire::events(turn)
        } else {
            responses_wire::events(turn)
        };
        let mut bytes = Vec::new();
        for frame in &mut frames {
            if chat {
                frame["model"] = json!("private-model");
                if probability_case {
                    frame["service_tier"] = json!("default");
                    if let Some(fragment) = frame
                        .pointer("/choices/0/delta/content")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        frame["choices"][0]["logprobs"] =
                            json!({"content":[probability(fragment)]});
                    }
                }
            } else if let Some(snapshot) = frame.get_mut("response") {
                snapshot["model"] = json!("private-model");
                snapshot["created_at"] = json!(1);
                if !snapshot["completed_at"].is_null() {
                    snapshot["completed_at"] = json!(2);
                }
            }
            if chat {
                bytes.extend_from_slice(format!("data: {frame}\n\n").as_bytes());
            } else {
                bytes.extend_from_slice(
                    &openbridge::protocol::openai::sse::encode_frame(frame, 1 << 20).unwrap(),
                );
            }
        }
        if chat {
            bytes.extend_from_slice(b"data: [DONE]\n\n");
        }
        bytes
    } else {
        serde_json::to_vec(&output).unwrap()
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(
            "content-type",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .body(Body::from(bytes))
        .unwrap()
}
fn responses_delivery(body: &[u8], streaming: bool) -> DecodedResponse {
    let scope = Some(ReplayOrigin::new("fixture").unwrap());
    if !streaming {
        return Adapter::new(Profile::Responses, Dialect::Standard, scope)
            .decode_response(body)
            .unwrap();
    }
    let mut decoder =
        ResponsesSseDecoder::new(200, "text/event-stream", Default::default(), scope).unwrap();
    for fragment in body.chunks(13) {
        let mut rest = fragment;
        while !rest.is_empty() {
            let (used, _) = decoder.consume(rest).unwrap();
            assert!(used > 0);
            rest = &rest[used..];
        }
    }
    decoder.finish().unwrap();
    decoder.materialize().unwrap()
}

#[tokio::test]
async fn real_router_uses_provider_http_for_json_sse_tools_and_cross_profile() {
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let observed = Upstream::default();
    let app = Router::new()
        .route("/chat/completions", post(answer))
        .route("/responses", post(answer))
        .with_state(observed.clone());
    let upstream_guard = Guard(tokio::spawn(async move {
        axum::serve(upstream, app).await.unwrap();
    }));
    let gateway = support::gateway(
        &origin,
        Limits {
            default_output_tokens: 32,
            ..Limits::default()
        },
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, stopped) = oneshot::channel();
    let serving = tokio::spawn(gateway.serve(listener, async {
        let _ = stopped.await;
    }));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    for profile in [Profile::Chat, Profile::Responses] {
        for stream in [false, true] {
            let path = if profile == Profile::Chat {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            let request = if profile == Profile::Chat {
                json!({"model":"public-model","messages":[{"role":"user","content":"lookup"}],"stream":stream,"stream_options":if stream {json!({"include_usage":true,"include_obfuscation":false})}else{Value::Null}})
            } else {
                json!({"model":"public-model","input":"lookup","stream":stream})
            };
            let response = client
                .post(format!("{url}{path}"))
                .bearer_auth(support::CLIENT_KEY)
                .header("x-never-forward", "private-input")
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let body = response.bytes().await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("private-model"));
            if !stream {
                let value: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(value["model"], "public-model");
                if profile == Profile::Chat {
                    assert_eq!(
                        value["choices"][0]["message"]["tool_calls"][0]["id"],
                        "call-local"
                    );
                } else {
                    assert!(
                        value["output"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|i| i["call_id"] == "c_lookup")
                    );
                }
            } else {
                assert!(
                    String::from_utf8_lossy(&body).contains(if profile == Profile::Chat {
                        "[DONE]"
                    } else {
                        "response.completed"
                    })
                );
            }
            if profile == Profile::Responses {
                let decoded = responses_delivery(&body, stream);
                assert_eq!(decoded.semantic.outcome(), Outcome::Completed);
                let Continuation::ToolResults(calls) = decoded.semantic.continuation() else {
                    panic!("completed tool response still needs results")
                };
                assert_eq!(
                    calls.iter().map(|c| c.call_id).collect::<Vec<_>>(),
                    ["c_sql", "c_lookup"]
                );
                let contract = GenerationRepresentationContract {
                    replay_origin: Some(ReplayOrigin::new("fixture").unwrap()),
                    ..GenerationRepresentationContract::full()
                };
                let projected = Adapter::new(Profile::Responses, Dialect::Standard, None)
                    .encode_response(&decoded, &contract)
                    .unwrap();
                let mut history = vec![json!({"role":"user","content":"lookup"})];
                history.extend(projected["output"].as_array().unwrap().iter().cloned());
                for (index, call) in calls.into_iter().enumerate() {
                    // Synthetic results only; call arguments never execute code.
                    history.push(match call.call_id {
                        "c_sql" => json!({"type":"custom_tool_call_output","call_id":call.call_id,"output":"1"}),
                        "c_lookup" => json!({"type":"function_call_output","call_id":call.call_id,"output":"{\"n\":1}"}),
                        _ => panic!("unexpected call identity"),
                    });
                    // Derive facts from the actual delivered/replayed history,
                    // not from the earlier response's unchanged pending view.
                    let replay = Adapter::new(Profile::Responses, Dialect::Standard, None)
                        .decode_request(
                            &serde_json::to_vec(&json!({"model":"public-model","input":history}))
                                .unwrap(),
                        )
                        .unwrap();
                    if index == 0 {
                        let Continuation::ToolResults(pending) =
                            replay.task.semantic.continuation()
                        else {
                            panic!("one result must not resolve both calls")
                        };
                        assert_eq!(
                            pending.iter().map(|c| c.call_id).collect::<Vec<_>>(),
                            ["c_lookup"]
                        );
                    } else {
                        assert_eq!(
                            replay.task.semantic.continuation(),
                            Continuation::Unreported
                        );
                    }
                }
                let followup = client
                    .post(format!("{url}{path}"))
                    .bearer_auth(support::CLIENT_KEY)
                    .json(&json!({"model":"public-model","input":history,"stream":stream}))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(followup.status(), 200);
                let finished = responses_delivery(&followup.bytes().await.unwrap(), stream);
                assert_eq!(finished.semantic.outcome(), Outcome::Completed);
                assert_eq!(finished.semantic.continuation(), Continuation::Unreported);
                assert_eq!(finished.semantic.usage().unwrap().total_tokens, Some(8));

                let before = observed.0.lock().unwrap().len();
                history.last_mut().unwrap()["call_id"] = json!("unmatched-call");
                let rejected = client
                    .post(format!("{url}{path}"))
                    .bearer_auth(support::CLIENT_KEY)
                    .json(&json!({"model":"public-model","input":history,"stream":stream}))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(rejected.status(), 400);
                assert_eq!(observed.0.lock().unwrap().len(), before);
            }
        }
    }
    // New controls and reported probabilities traverse the actual HTTP chain;
    // metadata is a static reported fact, never a request echo or a chunk field.
    for stream in [false, true] {
        let response = client.post(format!("{url}/v1/chat/completions"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&json!({"model":"public-model","messages":[{"role":"user","content":"hi"}],"stream":stream,"metadata":{"case":"probabilities"},"service_tier":"fast","logprobs":true,"top_logprobs":1}))
            .send().await.unwrap();
        assert_eq!(response.status(), 200);
        let bytes = response.bytes().await.unwrap();
        let decoded = if stream {
            let mut decoder = openbridge::protocol::openai::chat_sse::ChatSseDecoder::new(
                200,
                "text/event-stream",
                Default::default(),
            )
            .unwrap();
            let mut rest = bytes.as_ref();
            while !rest.is_empty() {
                let (used, _) = decoder.consume(rest).unwrap();
                assert!(used > 0);
                rest = &rest[used..];
            }
            decoder.finish().unwrap();
            decoder.materialize().unwrap()
        } else {
            let wire: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(wire["metadata"], json!({"provider":"reported"}));
            openbridge::adapter::Adapter::new(
                Profile::Chat,
                openbridge::adapter::Dialect::Standard,
                None,
            )
            .decode_response(&bytes)
            .unwrap()
        };
        let openbridge::semantic::task::generation::Item::Message(message) =
            &decoded.semantic.items()[0].1
        else {
            panic!("message")
        };
        let openbridge::semantic::task::generation::ContentPart::Text(text) =
            &message.parts[0].content
        else {
            panic!("text")
        };
        assert_eq!(text.as_str(), "old 🧪");
        assert_eq!(text.logprobs().value().unwrap().len(), 2);
        assert_eq!(
            decoded.metadata.context.execution.service_tier,
            openbridge::semantic::value::Presence::Value(
                openbridge::semantic::context::ServiceTier::Default
            )
        );
        if stream {
            assert!(decoded.metadata.context.execution.metadata.is_absent());
        }
    }
    // Images traverse the same request IR and real provider I/O, including cross-wire.
    for (profile, model) in [
        (Profile::Chat, "public-model"),
        (Profile::Responses, "public-model"),
        (Profile::Responses, "cross-model"),
    ] {
        for stream in [false, true] {
            let parts = if profile == Profile::Chat {
                json!([
                    {"type":"text","text":"first"},
                    {"type":"image_url","image_url":{"url":"data:image/png;base64,AQID"}},
                    {"type":"text","text":"second"},
                    {"type":"image_url","image_url":{"url":"https://example.test/synthetic.png","detail":"low"}}
                ])
            } else {
                json!([
                    {"type":"input_text","text":"first"},
                    {"type":"input_image","image_url":"data:image/png;base64,AQID"},
                    {"type":"input_text","text":"second"},
                    {"type":"input_image","image_url":"https://example.test/synthetic.png","detail":"low"}
                ])
            };
            let request = if profile == Profile::Chat {
                json!({"model":model,"messages":[{"role":"user","content":parts}],"stream":stream,"stream_options":if stream {json!({"include_usage":true,"include_obfuscation":false})}else{Value::Null}})
            } else {
                json!({"model":model,"input":[{"role":"user","content":parts}],"stream":stream})
            };
            let path = if profile == Profile::Chat {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            let response = client
                .post(format!("{url}{path}"))
                .bearer_auth(support::CLIENT_KEY)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let bytes = response.bytes().await.unwrap();
            let decoded = if !stream {
                openbridge::adapter::Adapter::new(
                    profile,
                    openbridge::adapter::Dialect::Standard,
                    None,
                )
                .decode_response(&bytes)
                .unwrap()
            } else if profile == Profile::Chat {
                let mut decoder = openbridge::protocol::openai::chat_sse::ChatSseDecoder::new(
                    200,
                    "text/event-stream",
                    Default::default(),
                )
                .unwrap();
                let mut rest = bytes.as_ref();
                while !rest.is_empty() {
                    let (used, _) = decoder.consume(rest).unwrap();
                    assert!(used > 0);
                    rest = &rest[used..];
                }
                decoder.finish().unwrap();
                decoder.materialize().unwrap()
            } else {
                let mut decoder = openbridge::protocol::openai::sse::ResponsesSseDecoder::new(
                    200,
                    "text/event-stream",
                    Default::default(),
                    None,
                )
                .unwrap();
                let mut rest = bytes.as_ref();
                while !rest.is_empty() {
                    let (used, _) = decoder.consume(rest).unwrap();
                    assert!(used > 0);
                    rest = &rest[used..];
                }
                decoder.finish().unwrap();
                decoder.materialize().unwrap()
            };
            assert_eq!(
                decoded.semantic.outcome(),
                openbridge::semantic::task::generation::Outcome::Completed
            );
            let openbridge::semantic::task::generation::Item::Message(message) =
                &decoded.semantic.items()[0].1
            else {
                panic!("text output")
            };
            let expected = if profile == Profile::Responses && model == "public-model" {
                "{\"ok\":false}"
            } else {
                "old 🧪"
            };
            assert!(
                matches!(&message.parts[0].content,openbridge::semantic::task::generation::ContentPart::Text(text) if text.as_str()==expected)
            );
        }
    }
    // Tool image results are a Responses history carrier, not a user message
    // or a Chat tool-result extension. No URL/inline bytes are fetched locally.
    for stream in [false, true] {
        let request = json!({"model":"public-model","metadata":{"case":"tool-images"},"stream":stream,"input":[
            {"type":"function_call","call_id":"media-call","name":"lookup","arguments":"{}"},
            {"type":"function_call_output","call_id":"media-call","output":[
                {"type":"input_text","text":"caption"},
                {"type":"input_image","image_url":"data:image/png;base64,AQID"},
                {"type":"input_image","image_url":"https://example.test/tool.png","detail":"low"}
            ]}
        ]});
        let before = observed.0.lock().unwrap().len();
        let response = client
            .post(format!("{url}/v1/responses"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let decoded = responses_delivery(&response.bytes().await.unwrap(), stream);
        assert_eq!(decoded.semantic.outcome(), Outcome::Completed);
        assert_eq!(observed.0.lock().unwrap().len(), before + 1);
        let mut unrepresentable = request;
        unrepresentable["model"] = json!("cross-model");
        let response = client
            .post(format!("{url}/v1/responses"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&unrepresentable)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(observed.0.lock().unwrap().len(), before + 1);
    }
    // Private attachments fail admission before any Provider I/O.
    for input in [
        json!({"model":"public-model","input":"hi","_openbridge":{"version":1,"progress":"turn_finished"}}),
        json!({"model":"public-model","input":[{"type":"function_call","call_id":"c","name":"lookup","arguments":"{}","_openbridge":{"version":1,"arguments":"json"}}]}),
    ] {
        let before = observed.0.lock().unwrap().len();
        let response = client
            .post(format!("{url}/v1/responses"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&input)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(observed.0.lock().unwrap().len(), before);
    }
    // Responses has no message-call membership carrier: static projection fails,
    // while an already-published stream aborts without a fabricated terminal.
    for stream in [false, true] {
        let before = observed.0.lock().unwrap().len();
        let mut response = client
            .post(format!("{url}/v1/responses"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&json!({"model":"cross-model","input":"lookup","stream":stream}))
            .send()
            .await
            .unwrap();
        if stream {
            assert_eq!(response.status(), 200);
            let mut body = Vec::new();
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) => body.extend_from_slice(&chunk),
                    Err(_) => break,
                    Ok(None) => panic!("unrepresentable output completed its HTTP body"),
                }
            }
            let text = String::from_utf8_lossy(&body);
            assert!(!text.contains("private-model"));
            assert!(!text.contains("_openbridge"));
            assert!(!text.contains("response.completed"));
        } else {
            assert_eq!(response.status(), 502);
            let value: Value = response.json().await.unwrap();
            assert_eq!(value["error"]["code"], "upstream_error");
        }
        assert_eq!(observed.0.lock().unwrap().len(), before + 1);

        // Opposite request direction: explicit Chat history must be rejected
        // during candidate projection, before any HTTP Provider call.
        let before = observed.0.lock().unwrap().len();
        let response = client
            .post(format!("{url}/v1/chat/completions"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&json!({"model":"cross-model","stream":stream,"messages":[
                {"role":"assistant","content":null,"tool_calls":[{"id":"c","type":"function","function":{"name":"lookup","arguments":"{}"}}]},
                {"role":"tool","tool_call_id":"c","content":"ok"}
            ]}))
            .send().await.unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(observed.0.lock().unwrap().len(), before);
    }
    let before = observed.0.lock().unwrap().len();
    let response = client
        .post(format!("{url}/v1/responses"))
        .json(&json!({"model":"public-model","input":"hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = client
        .post(format!("{url}/v1/responses"))
        .bearer_auth(support::CLIENT_KEY)
        .json(&json!({"model":"missing","input":"hello"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let response = client
        .post(format!("{url}/v1/responses"))
        .bearer_auth(support::CLIENT_KEY)
        .json(
            &json!({"model":"public-model","input":"hello","base_url":"https://untrusted.invalid"}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = client
        .post(format!("{url}/v1/responses"))
        .bearer_auth(support::CLIENT_KEY)
        .json(&json!({"model":"public-model","input":[
            {"type":"program","id":"p","call_id":"pending-program","code":"opaque code","fingerprint":"opaque fingerprint"}
        ]}))
        .send().await.unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(observed.0.lock().unwrap().len(), before);
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), serving)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    drop(upstream_guard);
}

#[tokio::test]
async fn binary_bootstraps_only_explicit_files_and_ignores_environment_keys() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::AsyncBufReadExt;
    // A rejecting loopback proxy makes even an accidental upstream dispatch offline.
    let trap = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = format!("http://{}", trap.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let _trap = Guard(tokio::spawn(async move {
        axum::serve(
            trap,
            Router::new().fallback(move || {
                let observed = observed.clone();
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    StatusCode::BAD_GATEWAY
                }
            }),
        )
        .await
        .unwrap();
    }));
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("store");
    let manager = openbridge::credential::CredentialManager::new(&root, vec![]).unwrap();
    manager
        .add_api_key(
            "deepseek",
            "one",
            openbridge::credential::Secret::new("synthetic-file-key".into()).unwrap(),
        )
        .unwrap();
    manager
        .set_pool(
            "deepseek",
            "deepseek-api-key",
            0,
            openbridge::credential::CredentialPool {
                members: vec![openbridge::credential::CredentialRef::ApiKey {
                    alias: "one".into(),
                }],
                fallback: false,
                max_attempts: 1,
            },
        )
        .unwrap();
    let path = root.join("gateway.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&path)
        .unwrap()
        .write_all(
            &serde_json::to_vec(
                &json!({"client_key":support::CLIENT_KEY,"bind":"127.0.0.1:0","proxy":proxy}),
            )
            .unwrap(),
        )
        .unwrap();
    let mut process = tokio::process::Command::new(env!("CARGO_BIN_EXE_openbridge"))
        .args(["--credentials-dir"])
        .arg(&root)
        .env_clear()
        .env(
            "OPENBRIDGE_CLIENT_KEY",
            "synthetic-ignored-environment-key-0001",
        )
        .env(
            "OPENBRIDGE_DEEPSEEK_API_KEY",
            "synthetic-not-a-provider-key-0001",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = tokio::io::BufReader::new(process.stdout.take().unwrap());
    let mut ready = String::new();
    tokio::time::timeout(Duration::from_secs(3), output.read_line(&mut ready))
        .await
        .unwrap()
        .unwrap();
    let origin = ready
        .trim()
        .strip_prefix("OpenBridge listening on ")
        .expect("bounded readiness line");
    assert!(origin.starts_with("http://127.0.0.1:"));
    assert!(ready.len() < 128);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    assert_eq!(
        client
            .post(format!("{origin}/v1/responses"))
            .json(&json!({"model":"missing","input":"hello"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(format!("{origin}/v1/responses"))
            .bearer_auth(support::CLIENT_KEY)
            .json(&json!({"model":"missing","input":"hello"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    // No admitted request is issued: the synthetic key must never reach a Provider.
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    process.kill().await.unwrap();
    process.wait().await.unwrap();
}
