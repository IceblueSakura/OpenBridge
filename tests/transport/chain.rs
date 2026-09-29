//! Full request/response chain against synthetic DeepSeek/Xiaomi-style upstreams.
//!
//! The chain uses the same entry points as the authorized probe: plan admission,
//! attempt preparation, semantically blind transport, response intake and
//! downstream rendering. Wire oracles are the independent support fixtures, not
//! encoder output.
use crate::{chat_wire, wire};
use axum::{Router, routing::post};
use openbridge::{
    execution::{Attempt, AttemptError, UpstreamRequest, admit, prepare_chat, prepare_responses},
    lowering::generation::{GenerationRepresentationContract, RepresentationError},
    protocol::openai::{
        chat_envelope,
        chat_sse::ChatSseDecoder,
        envelope,
        sse::{Obfuscation, ResponsesSseDecoder, SseLimits},
    },
    provider::{
        AuthScheme, CredentialBindingId, CredentialKind, EndpointPath, ErrorClass,
        ProviderDefinition, ProviderId, SecretMaterial, TrustedOrigin,
    },
    semantic::{task::generation::*, value::Presence},
    topology::{
        Endpoint, EndpointId, EndpointTarget, ExecutionContract, ProtocolProfile, TaskKind,
        catalog::{deepseek_flash, mimo_v2_6_pro},
    },
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Authorization header values as the synthetic upstream observed them.
type Seen = Arc<Mutex<Vec<String>>>;

fn contract() -> GenerationRepresentationContract {
    GenerationRepresentationContract {
        replay_origin: None,
        custom_tools: false,
        strict_tools: false,
        image_input: false,
        audio_input: false,
        file_input: false,
        cache_hints: false,
        ..GenerationRepresentationContract::full()
    }
}

fn provider(origin: &str, chat_path: &str, responses_path: &str) -> ProviderDefinition {
    ProviderDefinition {
        id: ProviderId::new("fixture").unwrap(),
        origin: TrustedOrigin::parse(origin).unwrap(),
        chat_completions: EndpointPath::new(chat_path).unwrap(),
        responses: EndpointPath::new(responses_path).unwrap(),
        auth: AuthScheme::Bearer,
    }
}

fn endpoint(definition: &ProviderDefinition, protocol: ProtocolProfile, model: &str) -> Endpoint {
    let path = match protocol {
        ProtocolProfile::OpenAiChat => definition.chat_completions.clone(),
        ProtocolProfile::OpenAiResponses => definition.responses.clone(),
    };
    Endpoint {
        id: EndpointId::new("fixture-endpoint").unwrap(),
        provider: definition.id.clone(),
        target: EndpointTarget {
            origin: definition.origin.clone(),
            path,
        },
        task: TaskKind::Generation,
        protocol,
        upstream_model: model.into(),
        representation: contract(),
        execution: ExecutionContract {
            streaming: true,
            retry_before_commit: false,
            request_body_limit: 256 * 1024,
            response_body_limit: 8 * 1024 * 1024,
            credential_kind: CredentialKind::ApiKey,
            timeout_ms: 30_000,
        },
        credential: CredentialBindingId::new("fixture-key").unwrap(),
    }
}

fn secret() -> SecretMaterial {
    SecretMaterial::new("sk-test-0001").unwrap()
}

fn chat_request(model: &str, stream: bool) -> chat_envelope::DecodedChatRequest {
    let mut body = json!({"model": model, "messages": [{"role": "user", "content": "hello"}]});
    if stream {
        body["stream"] = json!(true);
        body["stream_options"] = json!({"include_usage": true, "include_obfuscation": false});
    }
    chat_envelope::decode_request_bytes(body.to_string().as_bytes()).unwrap()
}

fn responses_request(model: &str, stream: bool) -> envelope::DecodedResponsesRequest {
    let mut body = json!({"model": model, "input": "hello"});
    if stream {
        body["stream"] = json!(true);
    }
    envelope::decode_request_bytes(body.to_string().as_bytes()).unwrap()
}

/// Handler factory capturing the authorization header seen by the upstream.
fn handler(
    seen: Seen,
    respond: fn() -> Value,
) -> impl Fn(axum::http::HeaderMap) -> std::future::Ready<axum::Json<Value>> + Clone {
    move |headers: axum::http::HeaderMap| {
        seen.lock().unwrap().push(
            headers
                .get("authorization")
                .map(|v| v.to_str().unwrap_or_default().to_string())
                .unwrap_or_default(),
        );
        std::future::ready(axum::Json(respond()))
    }
}

async fn serve(routes: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, routes).await.unwrap();
    });
    origin
}

async fn send(request: &UpstreamRequest) -> (u16, String, Vec<u8>) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut call = client.post(format!("{}{}", request.origin, request.path));
    for (name, value) in &request.safe_headers {
        call = call.header(name, value);
    }
    call = call.header(&request.auth_header.0, &request.auth_header.1);
    let response = call.body(request.body.clone()).send().await.unwrap();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    (
        status,
        content_type,
        response.bytes().await.unwrap().to_vec(),
    )
}

fn run_attempt(
    protocol: ProtocolProfile,
    status: u16,
    content_type: &str,
    body: &[u8],
    chunk: usize,
) -> Attempt {
    run_attempt_with_scope(protocol, status, content_type, body, chunk, None)
}

fn run_attempt_with_scope(
    protocol: ProtocolProfile,
    status: u16,
    content_type: &str,
    body: &[u8],
    chunk: usize,
    replay_origin: Option<openbridge::semantic::value::ReplayOrigin>,
) -> Attempt {
    let mut attempt = Attempt::new(protocol, 8 << 20, SseLimits::default(), replay_origin);
    attempt.begin(status, content_type).unwrap();
    for piece in body.chunks(chunk) {
        let mut rest = piece;
        while !rest.is_empty() {
            let (n, _) = attempt.push(rest).unwrap();
            assert!(n > 0 && n <= rest.len());
            rest = &rest[n..];
        }
    }
    attempt.finish().unwrap();
    attempt
}

fn chat_stream_body(turn: u8) -> Vec<u8> {
    let mut body = vec![];
    for value in chat_wire::events(turn) {
        body.extend_from_slice(format!("data: {value}\n\n").as_bytes());
    }
    body.extend_from_slice(b"data: [DONE]\n\n");
    body
}

fn responses_stream_body(turn: u8) -> Vec<u8> {
    wire::events(turn)
        .iter()
        .flat_map(|value| {
            openbridge::protocol::openai::sse::encode_frame(
                value,
                SseLimits::default().max_event_bytes,
            )
            .unwrap()
        })
        .collect()
}

#[tokio::test]
async fn replay_target_scope_comes_from_the_delivery_contract() {
    let body = serde_json::to_vec(&wire::response(1)).unwrap();
    let source = openbridge::semantic::value::ReplayOrigin::new("deepseek").unwrap();
    let attempt = || {
        run_attempt_with_scope(
            ProtocolProfile::OpenAiResponses,
            200,
            "application/json",
            &body,
            7,
            Some(source.clone()),
        )
    };
    let native = GenerationRepresentationContract {
        replay_origin: Some(source.clone()),
        ..GenerationRepresentationContract::full()
    };
    let other = GenerationRepresentationContract {
        replay_origin: Some(
            openbridge::semantic::value::ReplayOrigin::new("other-provider").unwrap(),
        ),
        ..GenerationRepresentationContract::full()
    };

    // A target that declares a different scope cannot receive the replay token.
    assert!(matches!(
        attempt().render_json(ProtocolProfile::OpenAiResponses, &other, "deepseek-flash"),
        Err(AttemptError::Representation(
            RepresentationError::ReplayOrigin
        ))
    ));
    // A target without any scope still refuses it.
    assert!(matches!(
        attempt().render_json(
            ProtocolProfile::OpenAiResponses,
            &GenerationRepresentationContract::full(),
            "deepseek-flash"
        ),
        Err(AttemptError::Representation(
            RepresentationError::ReplayOrigin
        ))
    ));
    // The matching target scope delivers the encrypted token natively.
    let delivered = attempt()
        .render_json(ProtocolProfile::OpenAiResponses, &native, "deepseek-flash")
        .unwrap();
    let value: Value = serde_json::from_slice(&delivered).unwrap();
    assert_eq!(
        value["output"][0]["encrypted_content"],
        "synthetic-final-token"
    );
}

fn stream_options() -> chat_envelope::StreamOptions {
    chat_envelope::StreamOptions {
        include_usage: Presence::Value(true),
        include_obfuscation: Presence::Value(false),
    }
}

#[tokio::test]
async fn chat_json_chain_delivers_public_label_and_preserved_output() {
    let seen: Seen = Arc::default();
    let routes = Router::new().route(
        "/chat/completions",
        post(handler(seen.clone(), || chat_wire::response(2))),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/chat/completions", "/responses");
    let endpoint = endpoint(&definition, ProtocolProfile::OpenAiChat, "deepseek-flash");

    let request = chat_request("deepseek-flash", false);
    admit(&deepseek_flash(), &request.task).unwrap();
    let upstream = prepare_chat(&endpoint, &definition, &secret(), &request).unwrap();
    assert_eq!(upstream.path, "/chat/completions");
    let sent: Value = serde_json::from_slice(&upstream.body).unwrap();
    assert_eq!(sent["model"], "deepseek-flash", "upstream model binding");
    assert_eq!(sent["messages"][0]["content"], "hello");

    let (status, content_type, body) = send(&upstream).await;
    assert_eq!(status, 200);
    let mut attempt = run_attempt(ProtocolProfile::OpenAiChat, status, &content_type, &body, 7);
    let delivered = attempt
        .render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash")
        .unwrap();
    assert!(
        !attempt.lifecycle().may_retry_or_fallback(),
        "post-commit fallback is forbidden"
    );

    let downstream: Value = serde_json::from_slice(&delivered).unwrap();
    assert_eq!(downstream["object"], "chat.completion");
    assert_eq!(
        downstream["model"], "deepseek-flash",
        "public label, not upstream locator"
    );
    assert_eq!(downstream["choices"][0]["message"]["content"], "old 🧪");
    assert_eq!(downstream["choices"][0]["finish_reason"], "stop");
    assert_eq!(downstream["usage"]["prompt_tokens"], 3);
    // Client consumption through the independent decode path.
    let decoded = chat_envelope::decode_response_bytes(&delivered).unwrap();
    assert_eq!(decoded.semantic.items().len(), 1);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        ["Bearer sk-test-0001".to_string()],
        "the upstream sees only the bound credential"
    );
}

#[tokio::test]
async fn chat_sse_chain_replays_downstream_frames_with_done_terminal() {
    let routes = Router::new().route(
        "/chat/completions",
        post(|| async {
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/event-stream; charset=utf-8",
                )],
                chat_stream_body(2),
            )
        }),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/chat/completions", "/responses");
    let endpoint = endpoint(&definition, ProtocolProfile::OpenAiChat, "deepseek-flash");

    let request = chat_request("deepseek-flash", true);
    let upstream = prepare_chat(&endpoint, &definition, &secret(), &request).unwrap();
    let sent: Value = serde_json::from_slice(&upstream.body).unwrap();
    assert_eq!(sent["stream"], true);
    assert_eq!(
        sent["stream_options"]["include_usage"], true,
        "presence survives native projection"
    );

    let (status, content_type, body) = send(&upstream).await;
    let mut attempt = run_attempt(ProtocolProfile::OpenAiChat, status, &content_type, &body, 3);
    assert!(!attempt.events().is_empty());
    let frames = attempt
        .render_stream(
            ProtocolProfile::OpenAiChat,
            &contract(),
            "deepseek-flash",
            SseLimits::default(),
            stream_options(),
            Obfuscation::Disabled,
        )
        .unwrap();
    assert_eq!(&frames.last().unwrap()[..], b"data: [DONE]\n\n");

    // Client consumption: the downstream frames decode back to the same semantics.
    let mut decoder = ChatSseDecoder::new(
        200,
        "text/event-stream; charset=utf-8",
        SseLimits::default(),
    )
    .unwrap();
    let mut text = String::new();
    for frame in &frames {
        let mut rest = &frame[..];
        while !rest.is_empty() {
            let (n, events) = decoder.consume(rest).unwrap();
            assert!(n > 0);
            rest = &rest[n..];
            for event in events {
                if let StreamEvent::Delta { fragment, .. } = event {
                    text.push_str(&fragment);
                }
            }
        }
    }
    decoder.finish().unwrap();
    assert_eq!(text, "old 🧪");
}

#[tokio::test]
async fn responses_json_chain_preserves_usage_details_and_public_label() {
    let seen: Seen = Arc::default();
    let routes = Router::new().route(
        "/v1/responses",
        post(handler(seen.clone(), || wire::response(2))),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/v1/chat/completions", "/v1/responses");
    let endpoint = endpoint(
        &definition,
        ProtocolProfile::OpenAiResponses,
        "mimo-v2.6-pro",
    );

    let request = responses_request("mimo-v2.6-pro", false);
    admit(&mimo_v2_6_pro(), &request.task).unwrap();
    let upstream = prepare_responses(&endpoint, &definition, &secret(), &request).unwrap();
    assert_eq!(
        upstream.path, "/v1/responses",
        "Xiaomi entries carry the /v1 prefix"
    );
    let sent: Value = serde_json::from_slice(&upstream.body).unwrap();
    assert_eq!(sent["model"], "mimo-v2.6-pro");
    assert_eq!(sent["store"], false);

    let (status, content_type, body) = send(&upstream).await;
    let mut attempt = run_attempt(
        ProtocolProfile::OpenAiResponses,
        status,
        &content_type,
        &body,
        11,
    );
    let delivered = attempt
        .render_json(
            ProtocolProfile::OpenAiResponses,
            &contract(),
            "mimo-v2.6-pro",
        )
        .unwrap();

    let downstream: Value = serde_json::from_slice(&delivered).unwrap();
    assert_eq!(downstream["object"], "response");
    assert_eq!(downstream["model"], "mimo-v2.6-pro");
    assert_eq!(downstream["status"], "completed");
    assert_eq!(
        downstream["usage"]["input_tokens_details"]["cache_write_tokens"], 0,
        "usage details survive the chain without estimation"
    );
    let decoded = envelope::decode_response_bytes(&delivered).unwrap();
    assert_eq!(decoded.metadata.model, "mimo-v2.6-pro");
}

#[tokio::test]
async fn responses_sse_chain_projects_stream_events_to_downstream_frames() {
    let routes = Router::new().route(
        "/v1/responses",
        post(|| async {
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/event-stream; charset=utf-8",
                )],
                responses_stream_body(2),
            )
        }),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/v1/chat/completions", "/v1/responses");
    let endpoint = endpoint(
        &definition,
        ProtocolProfile::OpenAiResponses,
        "mimo-v2.6-pro",
    );

    let request = responses_request("mimo-v2.6-pro", true);
    let upstream = prepare_responses(&endpoint, &definition, &secret(), &request).unwrap();
    let (status, content_type, body) = send(&upstream).await;
    let mut attempt = run_attempt(
        ProtocolProfile::OpenAiResponses,
        status,
        &content_type,
        &body,
        5,
    );
    let frames = attempt
        .render_stream(
            ProtocolProfile::OpenAiResponses,
            &contract(),
            "mimo-v2.6-pro",
            SseLimits::default(),
            chat_envelope::StreamOptions::default(),
            Obfuscation::Disabled,
        )
        .unwrap();
    let wire_bytes: Vec<u8> = frames.iter().flat_map(|f| f.to_vec()).collect();
    assert!(
        wire_bytes
            .windows(b"response.completed".len())
            .any(|w| w == b"response.completed"),
        "typed terminal survives downstream projection"
    );
    assert!(!wire_bytes.windows(b"[DONE]".len()).any(|w| w == b"[DONE]"));

    let mut decoder = ResponsesSseDecoder::new(
        200,
        "text/event-stream; charset=utf-8",
        SseLimits::default(),
        None,
    )
    .unwrap();
    let mut text = String::new();
    for frame in &frames {
        let mut rest = &frame[..];
        while !rest.is_empty() {
            let (n, events) = decoder.consume(rest).unwrap();
            assert!(n > 0);
            rest = &rest[n..];
            for event in events {
                if let StreamEvent::Delta { fragment, .. } = event {
                    text.push_str(&fragment);
                }
            }
        }
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert_eq!(text, "{\"ok\":false}");
    assert_eq!(decoded.semantic.items().len(), 1);
}

#[tokio::test]
async fn fixed_paths_and_credentials_are_never_request_selectable() {
    let seen: Seen = Arc::default();
    let routes = Router::new().route(
        "/v1/chat/completions",
        post(handler(seen.clone(), || chat_wire::response(2))),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/v1/chat/completions", "/v1/responses");
    let endpoint = endpoint(&definition, ProtocolProfile::OpenAiChat, "mimo-v2.6-pro");

    let request = chat_request("mimo-v2.6-pro", false);
    let upstream = prepare_chat(&endpoint, &definition, &secret(), &request).unwrap();
    assert_eq!(upstream.path, "/v1/chat/completions");
    assert_eq!(
        upstream.auth_header,
        (
            "authorization".to_string(),
            "Bearer sk-test-0001".to_string()
        )
    );
    let (status, _, _) = send(&upstream).await;
    assert_eq!(status, 200);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        ["Bearer sk-test-0001".to_string()]
    );
}

#[tokio::test]
async fn failure_boundaries_classify_and_never_fake_success() {
    // Classified HTTP failures leave the attempt uncommitted: fallback stays legal.
    let mut attempt = Attempt::new(
        ProtocolProfile::OpenAiChat,
        1024,
        SseLimits::default(),
        None,
    );
    let error = attempt.begin(429, "application/json").unwrap_err();
    assert!(matches!(
        error,
        AttemptError::Status {
            status: 429,
            class: ErrorClass::RateLimit
        }
    ));
    assert!(attempt.lifecycle().may_retry_or_fallback());
    assert!(
        attempt
            .render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash")
            .is_err()
    );

    // Truncated static JSON body never becomes a successful terminal.
    let mut attempt = Attempt::new(
        ProtocolProfile::OpenAiChat,
        1024,
        SseLimits::default(),
        None,
    );
    attempt.begin(200, "application/json").unwrap();
    attempt.push(b"{\"object\":\"chat.compl").unwrap();
    assert!(attempt.finish().is_err());
    assert!(
        attempt
            .render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash")
            .is_err()
    );

    // SSE EOF without the Chat [DONE] terminal is not success.
    let mut body = vec![];
    for value in chat_wire::events(2).iter().take(2) {
        body.extend_from_slice(format!("data: {value}\n\n").as_bytes());
    }
    let mut attempt = Attempt::new(
        ProtocolProfile::OpenAiChat,
        8 << 20,
        SseLimits::default(),
        None,
    );
    attempt
        .begin(200, "text/event-stream; charset=utf-8")
        .unwrap();
    let mut rest = &body[..];
    while !rest.is_empty() {
        let (n, _) = attempt.push(rest).unwrap();
        rest = &rest[n..];
    }
    assert!(
        attempt.finish().is_err(),
        "EOF without DONE is not a success"
    );

    // Oversized static body hits the bounded budget.
    let mut attempt = Attempt::new(ProtocolProfile::OpenAiChat, 16, SseLimits::default(), None);
    attempt.begin(200, "application/json").unwrap();
    assert!(matches!(
        attempt.push(&[b' '; 64]),
        Err(AttemptError::Limit)
    ));

    // Unknown media types are protocol failures, not silent JSON.
    let mut attempt = Attempt::new(
        ProtocolProfile::OpenAiChat,
        1024,
        SseLimits::default(),
        None,
    );
    assert!(matches!(
        attempt.begin(200, "text/plain"),
        Err(AttemptError::Protocol(_))
    ));
}

#[tokio::test]
async fn committed_delivery_cannot_be_replayed() {
    let seen: Seen = Arc::default();
    let routes = Router::new().route(
        "/chat/completions",
        post(handler(seen.clone(), || chat_wire::response(2))),
    );
    let origin = serve(routes).await;
    let definition = provider(&origin, "/chat/completions", "/responses");
    let endpoint = endpoint(&definition, ProtocolProfile::OpenAiChat, "deepseek-flash");
    let request = chat_request("deepseek-flash", false);
    let upstream = prepare_chat(&endpoint, &definition, &secret(), &request).unwrap();
    let (status, content_type, body) = send(&upstream).await;
    let mut attempt = run_attempt(ProtocolProfile::OpenAiChat, status, &content_type, &body, 9);
    attempt
        .render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash")
        .unwrap();
    assert!(
        attempt
            .render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash")
            .is_err()
    );
    assert!(
        attempt
            .render_stream(
                ProtocolProfile::OpenAiChat,
                &contract(),
                "deepseek-flash",
                SseLimits::default(),
                stream_options(),
                Obfuscation::Disabled,
            )
            .is_err()
    );
}

#[tokio::test]
async fn cross_protocol_projection_is_deterministic_not_lossy() {
    // Downstream Chat -> upstream Responses: the request projects to the
    // Responses envelope with the bound upstream model identity.
    let definition = provider("http://127.0.0.1:39217", "/chat/completions", "/responses");
    let endpoint = endpoint(
        &definition,
        ProtocolProfile::OpenAiResponses,
        "deepseek-flash",
    );
    let request = chat_request("deepseek-flash", false);
    let upstream = prepare_chat(&endpoint, &definition, &secret(), &request).unwrap();
    let sent: Value = serde_json::from_slice(&upstream.body).unwrap();
    assert!(sent.get("object").is_none());
    assert_eq!(sent["model"], "deepseek-flash");
    assert!(
        sent.get("messages").is_none(),
        "Chat wire is not forwarded opaquely"
    );

    // Downstream Chat <- upstream Responses with cache-write usage: the Chat
    // profile has no projection for it yet, so delivery fails instead of
    // silently deleting the reported detail.
    let mut attempt = Attempt::new(
        ProtocolProfile::OpenAiResponses,
        8 << 20,
        SseLimits::default(),
        None,
    );
    attempt.begin(200, "application/json").unwrap();
    attempt
        .push(wire::response(2).to_string().as_bytes())
        .unwrap();
    attempt.finish().unwrap();
    assert!(matches!(
        attempt.render_json(ProtocolProfile::OpenAiChat, &contract(), "deepseek-flash"),
        Err(AttemptError::Representation(
            RepresentationError::UnmigratedSemantic
        ))
    ));
}
