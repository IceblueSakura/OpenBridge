//! Verifies upstream forwarding retry, fallback, header, stream, and cancellation boundaries.

mod support;

use std::{
    convert::Infallible,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, to_bytes},
    http::{
        Request, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, SET_COOKIE, USER_AGENT},
    },
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::Bytes;
use futures_util::{future::BoxFuture, stream};
use http::{HeaderMap, HeaderValue};
use openbridge::{
    bridge::{ChatStreamState, ResponsesStreamState, StreamTerminal},
    config::parse_bootstrap_config,
    core::{ApiProtocol, OperationKind},
    ingress::{GatewayState, build_router},
    provider::{PreparedUpstreamRequest, ProviderKind},
    providers::{build_compiled_registry, build_compiled_registry_with_active_pools},
    registry::{
        ReasoningLevel, ReasoningLevelMapping, ReasoningSupport, RegistryConfig, RouteConfig,
        RouteMode, UpstreamTarget, build_registry,
    },
    transport::sse::SseDecoder,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::Value;
use support::metrics::TestMetrics;
use tower::ServiceExt;

#[derive(Debug)]
struct RecordedRequest {
    path: String,
    authorization: String,
    user_agent: Option<String>,
    body: Value,
}

#[derive(Default)]
struct RecordingTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

const MIMO_RESPONSES_PARALLEL_TOOL_STREAM: &[u8] = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_mimo_1","status":"in_progress"}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"fc_0","type":"function_call","call_id":"call_0","name":"lookup_weather","arguments":""}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":1,"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"lookup_time","arguments":""}}

event: response.function_call_arguments.delta
data: {"type":"response.function_call_arguments.delta","item_id":"fc_0","output_index":0,"delta":"{\"city\":"}

event: response.function_call_arguments.delta
data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":"{\"tz\":"}

event: response.function_call_arguments.delta
data: {"type":"response.function_call_arguments.delta","item_id":"fc_0","output_index":0,"delta":"\"Shanghai\"}"}

event: response.function_call_arguments.delta
data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":1,"delta":"\"Asia/Shanghai\"}"}

event: response.function_call_arguments.done
data: {"type":"response.function_call_arguments.done","item_id":"fc_0","output_index":0,"arguments":"{\"city\":\"Shanghai\"}"}

event: response.function_call_arguments.done
data: {"type":"response.function_call_arguments.done","item_id":"fc_1","output_index":1,"arguments":"{\"tz\":\"Asia/Shanghai\"}"}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"fc_0","type":"function_call","call_id":"call_0","name":"lookup_weather","arguments":"{\"city\":\"Shanghai\"}"}}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":1,"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"lookup_time","arguments":"{\"tz\":\"Asia/Shanghai\"}"}}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_mimo_1","status":"completed"}}

"#;

const DEEPSEEK_CHAT_REASONING_STREAM: &[u8] = br#"data: {"id":"chatcmpl_deepseek_reasoning","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"\u5148\u5206\u6790"},"finish_reason":null}]}

data: {"id":"chatcmpl_deepseek_reasoning","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"reasoning_content":"\u540e\u5f97\u51fa\u7ed3\u8bba"},"finish_reason":null}]}

data: {"id":"chatcmpl_deepseek_reasoning","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"content":"\u7b54\u6848"},"finish_reason":null}]}

data: {"id":"chatcmpl_deepseek_reasoning","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]

"#;

#[derive(Default)]
struct MimoResponsesToolStreamTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

#[derive(Default)]
struct MimoImageTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

#[derive(Default)]
struct DeepSeekReasoningStreamTransport {
    requests: Mutex<Vec<RecordedRequest>>,
}

struct TimeoutTransport;

struct NonSseErrorTransport;

#[derive(Default)]
struct RateLimitedTransport {
    attempts: Mutex<usize>,
}

#[derive(Default)]
struct CredentialRotationTransport {
    authorizations: Mutex<Vec<String>>,
}

struct FixedStatusCredentialTransport {
    status: StatusCode,
    authorizations: Mutex<Vec<String>>,
}

struct InvalidSseTransport;

struct EofWithoutTerminalTransport;

struct PartialStreamFailureTransport {
    attempts: AtomicUsize,
}

struct PendingSseTransport {
    dropped: Arc<AtomicBool>,
}

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct FailoverTransport {
    attempted_models: Mutex<Vec<String>>,
}

#[derive(Default)]
struct BoundedFailoverTransport {
    attempts: Mutex<Vec<(String, ProviderKind, Instant)>>,
}

struct PendingRequestTransport {
    attempts: AtomicUsize,
    started: tokio::sync::Notify,
    dropped: Arc<AtomicBool>,
}

struct BackoffCancellationTransport {
    attempts: AtomicUsize,
    first_attempt: tokio::sync::Notify,
}

#[derive(Default)]
struct ScopedHealthTransport {
    attempts: Mutex<Vec<String>>,
}

#[derive(Default)]
struct ScopedFaultTransport {
    attempts: Mutex<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyntheticTokenGeneration {
    First,
    Second,
    Unknown,
}

#[derive(Debug, Eq, PartialEq)]
struct ChatGptRecordedRequest {
    path: String,
    model: String,
    input_is_array: bool,
    store_is_false: bool,
    output_limit_present: bool,
    token_generation: SyntheticTokenGeneration,
    account_matches: bool,
    originator_matches: bool,
    user_agent_matches: bool,
    accepts_sse: bool,
    fedramp_header_present: bool,
}

struct ChatGptOAuthTransport {
    first_authorization: String,
    second_authorization: String,
    replacement: Mutex<Option<(PathBuf, Vec<u8>)>>,
    reject_after_replacement: bool,
    requests: Mutex<Vec<ChatGptRecordedRequest>>,
}

impl UpstreamTransport for TimeoutTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async { Err(TransportError::Timeout) })
    }
}

impl UpstreamTransport for PendingRequestTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the started pending request and notify the test task.
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        let signal = DropSignal(self.dropped.clone());

        // Keep the upstream future pending to observe whether downstream cancellation propagates destruction.
        Box::pin(async move {
            let _signal = signal;
            std::future::pending::<Result<UpstreamResponse, TransportError>>().await
        })
    }
}

impl UpstreamTransport for BoundedFailoverTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the candidate, Provider, and start time for budget and backoff assertions.
        let target_id = target.id().to_owned();
        let provider = target.kind();
        self.attempts
            .lock()
            .unwrap()
            .push((target_id.clone(), provider, Instant::now()));
        // Return different retryable HTTP failures by Provider.
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let status = if provider == ProviderKind::LongCat {
                headers.insert("retry-after", HeaderValue::from_static("3"));
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            Ok(UpstreamResponse::new(
                status,
                headers,
                Body::from(format!(r#"{{"error":{{"message":"{target_id} failed"}}}}"#)),
            ))
        })
    }
}

impl UpstreamTransport for BackoffCancellationTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the first attempt and wake the test task waiting for cancellation.
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if attempt == 1 {
            self.first_attempt.notify_one();
        }
        // Return a retryable failure so the handler enters cancellable backoff.
        Box::pin(async {
            Ok(UpstreamResponse::new(
                StatusCode::SERVICE_UNAVAILABLE,
                HeaderMap::new(),
                Body::from(r#"{"error":{"message":"temporary failure"}}"#),
            ))
        })
    }
}

impl UpstreamTransport for ScopedHealthTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Distinguish test targets by credential binding ID and record cross-request call order.
        let target_id = target.id().to_owned();
        self.attempts.lock().unwrap().push(target_id.clone());

        // The primary target returns 429 with a cooldown suggestion; other targets succeed consistently.
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if target_id == "openai-main" {
                headers.insert("retry-after", HeaderValue::from_static("10"));
                Ok(UpstreamResponse::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    headers,
                    Body::from(r#"{"error":{"message":"shared quota exhausted"}}"#),
                ))
            } else {
                Ok(UpstreamResponse::new(
                    StatusCode::OK,
                    headers,
                    Body::from(r#"{"id":"healthy-response"}"#),
                ))
            }
        })
    }
}

impl UpstreamTransport for ScopedFaultTransport {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record target order and make the primary target produce a retryable transport failure.
        let target_id = target.id().to_owned();
        self.attempts.lock().unwrap().push(target_id.clone());
        Box::pin(async move {
            if target_id == "openai-main" {
                Err(TransportError::Timeout)
            } else {
                Ok(UpstreamResponse::new(
                    StatusCode::OK,
                    HeaderMap::new(),
                    Body::from(r#"{"id":"healthy-response"}"#),
                ))
            }
        })
    }
}

impl UpstreamTransport for ChatGptOAuthTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Classify only synthetic authentication values and retain no general-purpose token output.
        let authorization = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let token_generation = if authorization == self.first_authorization {
            SyntheticTokenGeneration::First
        } else if authorization == self.second_authorization {
            SyntheticTokenGeneration::Second
        } else {
            SyntheticTokenGeneration::Unknown
        };
        let body: Value = serde_json::from_slice(request.body()).unwrap();
        self.requests.lock().unwrap().push(ChatGptRecordedRequest {
            path: request.relative_uri().path().to_owned(),
            model: body["model"].as_str().unwrap().to_owned(),
            input_is_array: body["input"].is_array(),
            store_is_false: body["store"] == false,
            output_limit_present: ["max_output_tokens", "max_completion_tokens", "max_tokens"]
                .iter()
                .any(|field| body.get(*field).is_some()),
            token_generation,
            account_matches: headers
                .get("chatgpt-account-id")
                .is_some_and(|value| value == "synthetic-account"),
            originator_matches: headers
                .get("originator")
                .is_some_and(|value| value == "codex_cli_rs"),
            user_agent_matches: headers.get(USER_AGENT).is_some_and(|value| {
                value == "codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown"
            }),
            accepts_sse: headers
                .get(ACCEPT)
                .is_some_and(|value| value == "text/event-stream"),
            fedramp_header_present: headers.contains_key("x-openai-fedramp"),
        });

        // Replace the synthetic persisted bundle before returning the first unauthorized response.
        let replacement = self.replacement.lock().unwrap().take();
        if let Some((path, document)) = replacement {
            fs::write(path, document).unwrap();
            return Box::pin(async {
                Ok(UpstreamResponse::new(
                    StatusCode::UNAUTHORIZED,
                    HeaderMap::new(),
                    Body::from(r#"{"error":{"message":"synthetic rejection"}}"#),
                ))
            });
        }
        if self.reject_after_replacement {
            return Box::pin(async {
                Ok(UpstreamResponse::new(
                    StatusCode::UNAUTHORIZED,
                    HeaderMap::new(),
                    Body::from(r#"{"error":{"message":"synthetic rejection"}}"#),
                ))
            });
        }

        // Return one complete synthetic ChatGPT Responses stream for every accepted attempt.
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from(
                    "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_synthetic\",\"status\":\"in_progress\"}}\n\nevent: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_synthetic\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"item_id\":\"msg_synthetic\",\"delta\":\"hello\"}\n\nevent: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"msg_synthetic\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_synthetic\",\"status\":\"completed\"}}\n\n",
                ),
            ))
        })
    }
}

impl UpstreamTransport for NonSseErrorTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok(UpstreamResponse::new(
                StatusCode::BAD_REQUEST,
                response_headers,
                Body::from(Bytes::from_static(b"\xff")),
            ))
        })
    }
}

impl UpstreamTransport for RateLimitedTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        *self.attempts.lock().unwrap() += 1;
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            response_headers.insert("retry-after", HeaderValue::from_static("2"));
            response_headers.insert("x-should-retry", HeaderValue::from_static("true"));
            Ok(UpstreamResponse::new(
                StatusCode::TOO_MANY_REQUESTS,
                response_headers,
                Body::from(r#"{"error":{"message":"rate limited"}}"#),
            ))
        })
    }
}

impl UpstreamTransport for CredentialRotationTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record synthetic Authorization and make the first member trigger a rotatable 429.
        let authorization = headers[AUTHORIZATION].to_str().unwrap().to_owned();
        self.authorizations
            .lock()
            .unwrap()
            .push(authorization.clone());
        Box::pin(async move {
            let status = if authorization == "Bearer key-a" {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::OK
            };
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from("{}"),
            ))
        })
    }
}

impl UpstreamTransport for FixedStatusCredentialTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the synthetic credential for each attempt and return the fixed HTTP status.
        self.authorizations
            .lock()
            .unwrap()
            .push(headers[AUTHORIZATION].to_str().unwrap().to_owned());
        let status = self.status;
        Box::pin(async move {
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from("{}"),
            ))
        })
    }
}

impl UpstreamTransport for InvalidSseTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(vec![Ok::<_, Infallible>(Bytes::from_static(
                    b"data: \xff\n\n",
                ))])),
            ))
        })
    }
}

impl UpstreamTransport for EofWithoutTerminalTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from(Bytes::from_static(
                    b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"hi\",\"logprobs\":[]}\n\n",
                )),
            ))
        })
    }
}

impl UpstreamTransport for PartialStreamFailureTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let event = b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"hi\",\"logprobs\":[]}\n\n";
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(vec![
                    Ok::<_, std::io::Error>(Bytes::from_static(event)),
                    Err(std::io::Error::other("upstream connection reset")),
                ])),
            ))
        })
    }
}

impl UpstreamTransport for PendingSseTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let signal = DropSignal(self.dropped.clone());
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            let body = Body::from_stream(stream::once(async move {
                let _signal = signal;
                std::future::pending::<Result<Bytes, Infallible>>().await
            }));
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                body,
            ))
        })
    }
}

impl UpstreamTransport for RecordingTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path: path.clone(),
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        });
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(if path.ends_with("responses") {
                    "text/event-stream"
                } else {
                    "application/json"
                }),
            );
            response_headers.insert("openai-request-id", HeaderValue::from_static("upstream-id"));
            response_headers.insert(SET_COOKIE, HeaderValue::from_static("must-not-pass=true"));
            let chunks = if path.ends_with("responses") {
                vec![
                    Ok::<_, Infallible>(Bytes::from_static(b"event: response.output_text.delta\n")),
                    Ok(Bytes::from_static(b"data: {\"delta\":\"hi\"}\n\n")),
                ]
            } else {
                vec![Ok(Bytes::from_static(b"{\"id\":\"chat-result\"}"))]
            };
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(chunks)),
            ))
        })
    }
}

impl UpstreamTransport for MimoImageTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Capture the exact Native request after trusted path, model, and authentication preparation.
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path: path.clone(),
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        });

        // Return one valid non-streaming response in the same protocol as the selected Native endpoint.
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = if path.ends_with("/responses") {
                Body::from(
                    r#"{"id":"resp_image","object":"response","status":"completed","model":"mimo-v2.5","output":[{"id":"msg_image","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"red and blue","annotations":[]}]}],"output_text":"red and blue"}"#,
                )
            } else {
                Body::from(
                    r#"{"id":"chat_image","object":"chat.completion","model":"mimo-v2.5","choices":[{"index":0,"message":{"role":"assistant","content":"red and blue"},"finish_reason":"stop"}]}"#,
                )
            };
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                body,
            ))
        })
    }
}

impl UpstreamTransport for MimoResponsesToolStreamTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the endpoint, authentication isolation, and JSON request actually submitted by the gateway.
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path,
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        });
        // Return a fragmented Responses tool stream that simulates upstream chunk boundaries and interleaved arguments.
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            response_headers.insert("openai-request-id", HeaderValue::from_static("mimo-id"));
            let chunks = MIMO_RESPONSES_PARALLEL_TOOL_STREAM
                .chunks(17)
                .map(Bytes::copy_from_slice)
                .map(Ok::<_, Infallible>)
                .collect::<Vec<_>>();
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(chunks)),
            ))
        })
    }
}

impl UpstreamTransport for DeepSeekReasoningStreamTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // Record the endpoint, model, and reasoning configuration submitted by DeepSeek Chat Native.
        let path = request.relative_uri().path().to_owned();
        self.requests.lock().unwrap().push(RecordedRequest {
            path,
            authorization: headers[AUTHORIZATION].to_str().unwrap().to_owned(),
            user_agent: headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: serde_json::from_slice(request.body()).unwrap(),
        });
        // Return reasoning_content in irregular UTF-8 chunks to verify that Native streaming preserves the plaintext channel.
        Box::pin(async move {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            response_headers.insert("openai-request-id", HeaderValue::from_static("deepseek-id"));
            let chunks = DEEPSEEK_CHAT_REASONING_STREAM
                .chunks(13)
                .map(Bytes::copy_from_slice)
                .map(Ok::<_, Infallible>)
                .collect::<Vec<_>>();
            Ok(UpstreamResponse::new(
                StatusCode::OK,
                response_headers,
                Body::from_stream(stream::iter(chunks)),
            ))
        })
    }
}

impl UpstreamTransport for FailoverTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        let model = serde_json::from_slice::<Value>(request.body()).unwrap()["model"]
            .as_str()
            .unwrap()
            .to_owned();
        self.attempted_models.lock().unwrap().push(model.clone());
        Box::pin(async move {
            let status = if model == "fallback-model" {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            Ok(UpstreamResponse::new(
                status,
                HeaderMap::new(),
                Body::from("{}"),
            ))
        })
    }
}

fn app_with_transport(transport: Arc<dyn UpstreamTransport>) -> axum::Router {
    app_with_transport_and_definition(
        transport,
        support::definition("forward-test", "public-model", "upstream-model"),
    )
}

async fn authenticated_response(app: &axum::Router, path: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::get(path)
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn authenticated_get(app: &axum::Router, path: &str) -> Value {
    let response = authenticated_response(app, path).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn compiled_authenticated_get(app: &axum::Router, path: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header(
                    AUTHORIZATION,
                    "Bearer downstream-token-00000000000000000000000000000000",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

fn app_with_compiled_registry(transport: Arc<dyn UpstreamTransport>) -> axum::Router {
    // Compile the real code registry so the test uses the fixed mimo-v2.5 public contract and production Route order.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("checked-in bootstrap must be valid");
    let registry = build_compiled_registry(bootstrap).expect("compiled registry must be valid");
    // Inject a test identity and synthetic credentials for every registered pool without reading private runtime credentials.
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-00000000000000000000000000000000",
        &registry,
        "upstream-token",
    );
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials);
    build_router(state)
}

fn app_with_chatgpt_oauth(
    transport: Arc<dyn UpstreamTransport>,
    auth_json_file: &std::path::Path,
) -> (
    axum::Router,
    Arc<openbridge::oauth2_credentials::OAuth2CredentialManager>,
) {
    // Compile the production ChatGPT targets and load only the synthetic OAuth2 source needed by this test.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("checked-in bootstrap must be valid");
    let active_pools = std::collections::BTreeSet::from(["chatgpt-codex".to_owned()]);
    let registry = build_compiled_registry_with_active_pools(bootstrap, &active_pools)
        .expect("compiled registry must be valid");
    let (users, credentials, oauth2_credentials) = support::users_and_oauth_credentials(
        "downstream-token-00000000000000000000000000000000",
        &registry,
        auth_json_file,
    );

    // Inject the guarded manager beside the immutable downstream/static credential snapshot.
    let state = GatewayState::new_with_oauth2_credentials(
        Arc::new(registry),
        transport,
        users,
        credentials,
        Arc::clone(&oauth2_credentials),
    );
    (build_router(state), oauth2_credentials)
}

fn app_with_transport_and_definition(
    transport: Arc<dyn UpstreamTransport>,
    definition: RegistryConfig,
) -> axum::Router {
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();
    let (users, credentials) = support::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "upstream-token",
    );
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials);
    build_router(state)
}

fn app_with_transport_and_pool(
    transport: Arc<dyn UpstreamTransport>,
    upstream_secrets: &[&str],
) -> (axum::Router, TestMetrics) {
    let registry = build_registry(
        support::bootstrap(support::BOOTSTRAP),
        support::definition("forward-test", "public-model", "upstream-model"),
    )
    .unwrap();
    let (users, credentials) = support::users_and_credential_pool(
        "downstream-token-0000000000000000",
        &registry,
        upstream_secrets,
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(Arc::new(registry), transport, users, credentials)
        .with_metrics(metrics.instruments());
    (build_router(state), metrics)
}

fn add_responses_fallback(
    definition: &mut RegistryConfig,
    target_id: &str,
    provider: ProviderKind,
) {
    // Resolve or register one trusted Provider instance, then bind the copied target to it.
    let provider_instance = match provider {
        ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => {
            if !definition
                .provider_instances
                .iter()
                .any(|instance| instance.id == "longcat-test")
            {
                definition
                    .provider_instances
                    .push(openbridge::registry::ProviderInstanceConfig {
                        id: "longcat-test".to_owned(),
                        kind: ProviderKind::LongCat,
                        base_url: "https://api.longcat.chat".to_owned(),
                    });
            }
            "longcat-test"
        }
        ProviderKind::ChatGpt
        | ProviderKind::DeepSeek
        | ProviderKind::MiMo
        | ProviderKind::OpenRouter => {
            panic!("test fallback helper only accepts connected providers")
        }
    };
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = target_id.to_owned();
    fallback.provider_instance = provider_instance.to_owned();
    fallback.provider_model = provider.routing_model_id(&fallback.canonical_model);
    if provider != definition.credential_pools[0].provider {
        let pool_id = format!("{target_id}-pool");
        definition
            .credential_pools
            .push(openbridge::registry::CredentialPoolConfig {
                id: pool_id.clone(),
                provider,
                kind: openbridge::provider::CredentialKind::ApiKey,
            });
        fallback.credential_pool = pool_id;
    }
    definition.upstream_targets.push(fallback);

    // Register the new target as a complete Responses Route for the same Public Model.
    let route_id = format!("{target_id}-responses");
    definition.routes.push(RouteConfig {
        id: route_id.clone(),
        upstream_target: target_id.to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: ApiProtocol::Responses.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes.push(route_id);
}

static NEXT_CHATGPT_AUTH_TEST: AtomicUsize = AtomicUsize::new(1);

struct SyntheticAuthDirectory {
    path: PathBuf,
}

impl SyntheticAuthDirectory {
    fn new() -> Self {
        let id = NEXT_CHATGPT_AUTH_TEST.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "openbridge-chatgpt-forwarding-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn auth_file(&self) -> PathBuf {
        self.path.join("synthetic-auth.json")
    }
}

impl Drop for SyntheticAuthDirectory {
    fn drop(&mut self) {
        // Remove only artifacts created under this process-unique synthetic test directory.
        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&self.path);
    }
}

fn synthetic_chatgpt_document(generation: u64) -> (Vec<u8>, String) {
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3_600
        + generation;
    let access_token = synthetic_jwt(serde_json::json!({
        "exp": expiry,
        "synthetic_generation": generation,
    }));
    let id_token = synthetic_jwt(serde_json::json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "synthetic-account",
            "chatgpt_account_is_fedramp": false,
        }
    }));
    let document = serde_json::to_vec(&serde_json::json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": id_token,
            "access_token": access_token,
            "refresh_token": format!("synthetic-refresh-{generation}"),
            "account_id": "synthetic-account",
        },
        "last_refresh": "2026-08-06T00:00:00Z",
    }))
    .unwrap();
    (document, access_token)
}

fn synthetic_jwt(payload: Value) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(b"{}"),
        URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes()),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}

#[path = "forwarding_contract/admission.rs"]
mod admission;
#[path = "forwarding_contract/chatgpt.rs"]
mod chatgpt;
#[path = "forwarding_contract/mimo.rs"]
mod mimo;
#[path = "forwarding_contract/models.rs"]
mod models;
#[path = "forwarding_contract/native.rs"]
mod native;
#[path = "forwarding_contract/resilience.rs"]
mod resilience;
