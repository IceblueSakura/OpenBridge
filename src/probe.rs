//! 管理员显式执行的上游 capability probe。
//!
//! probe 复用 deployment 的受信 endpoint、credential 和编译期 adapter，但它不走下游
//! HTTP API，也不会写回 routes.toml。下游 `/v1/models` 因而始终只列出 public alias；
//! probe 的 JSON 报告只是服务所有者更新 capability 配置时的证据。

use axum::body::to_bytes;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    config::{RegistrySnapshot, ResolvedDeployment},
    core::{Protocol, ValidatedRequest},
    provider::{CredentialSource, ProviderAdapter, RequestAdapter},
    transport::upstream::{UpstreamResponse, UpstreamTransport},
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;
const PROBE_PROMPT: &str = "Reply with exactly OK.";
const TOOL_NAME: &str = "openbridge_probe";

/// 明确选择要执行的 probe。CLI 不传任何选择时使用 `all()`；库调用方可仅执行无费用的
/// `list_models`，或只验证特定协议。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProbeSelection {
    pub list_models: bool,
    pub chat: bool,
    pub responses: bool,
    pub function_calling: bool,
}

impl ProbeSelection {
    pub const fn all() -> Self {
        Self {
            list_models: true,
            chat: true,
            responses: true,
            function_calling: true,
        }
    }

    pub const fn is_empty(self) -> bool {
        !self.list_models && !self.chat && !self.responses && !self.function_calling
    }
}

/// probe 对某个能力的保守结论。
///
/// `unsupported` 只用于端点明确不存在（404/405/501）。认证、限流、网络故障及请求
/// 形状被拒绝均保留为 `unknown`，避免一次临时故障错误关闭一条路由能力。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeState {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeOutcome {
    pub state: ProbeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

impl ProbeOutcome {
    const fn supported(status: StatusCode) -> Self {
        Self {
            state: ProbeState::Supported,
            http_status: Some(status.as_u16()),
        }
    }

    const fn from_http_status(status: StatusCode) -> Self {
        Self {
            state: if matches!(
                status,
                StatusCode::NOT_FOUND
                    | StatusCode::METHOD_NOT_ALLOWED
                    | StatusCode::NOT_IMPLEMENTED
            ) {
                ProbeState::Unsupported
            } else {
                ProbeState::Unknown
            },
            http_status: Some(status.as_u16()),
        }
    }

    const fn unknown(status: Option<StatusCode>) -> Self {
        Self {
            state: ProbeState::Unknown,
            http_status: match status {
                Some(status) => Some(status.as_u16()),
                None => None,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListModelsObservation {
    pub outcome: ProbeOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FunctionCallingObservation {
    pub initial_call: ProbeOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_replay: Option<ProbeOutcome>,
}

/// 单个 deployment 的 probe 报告。它不包含 credential、请求正文或上游响应正文。
#[derive(Debug, Serialize)]
pub struct DeploymentProbeReport {
    pub deployment_id: String,
    pub upstream_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_models: Option<ListModelsObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<ProbeOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<ProbeOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_function_calling: Option<FunctionCallingObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses_function_calling: Option<FunctionCallingObservation>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("configured deployment '{deployment}' does not exist")]
    UnknownDeployment { deployment: String },
    #[error("configured provider for deployment '{deployment}' does not exist")]
    MissingProvider { deployment: String },
    #[error("upstream credentials are unavailable for probe")]
    CredentialUnavailable,
    #[error("provider authentication could not be prepared for probe")]
    AuthenticationPreparation,
}

/// 使用与数据面相同的受信配置执行选定 probe。
///
/// 该函数只访问 `deployment_id` 对应的固定 endpoint；没有接受 URL、model 或 header 的
/// 外部参数，避免诊断能力扩大 SSRF 或 credential 使用范围。
pub async fn probe_deployment(
    snapshot: &RegistrySnapshot,
    deployment_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialSource,
    selection: ProbeSelection,
) -> Result<DeploymentProbeReport, ProbeError> {
    let deployment =
        snapshot
            .deployment(deployment_id)
            .ok_or_else(|| ProbeError::UnknownDeployment {
                deployment: deployment_id.to_owned(),
            })?;
    let provider =
        snapshot
            .provider(deployment.provider_id())
            .ok_or_else(|| ProbeError::MissingProvider {
                deployment: deployment_id.to_owned(),
            })?;
    let credential = credentials
        .resolve(
            provider.kind(),
            provider.credential().id(),
            provider.credential().secret_reference().locator(),
        )
        .map_err(|_| ProbeError::CredentialUnavailable)?;
    let adapter = ProviderAdapter::for_kind(provider.kind());
    let headers = adapter
        .build_outbound_headers(&credential)
        .map_err(|_| ProbeError::AuthenticationPreparation)?;
    let session = ProbeSession {
        deployment,
        transport,
        adapter,
        headers,
        max_response_bytes: snapshot.limits().max_request_body_bytes(),
    };

    let list_models = if selection.list_models {
        Some(session.probe_list_models().await)
    } else {
        None
    };
    let chat = if selection.chat {
        Some(session.probe_text(Protocol::ChatCompletions).await)
    } else {
        None
    };
    let responses = if selection.responses {
        Some(session.probe_text(Protocol::Responses).await)
    } else {
        None
    };
    let chat_function_calling = if selection.function_calling {
        Some(
            session
                .probe_function_calling(Protocol::ChatCompletions)
                .await,
        )
    } else {
        None
    };
    let responses_function_calling = if selection.function_calling {
        Some(session.probe_function_calling(Protocol::Responses).await)
    } else {
        None
    };

    Ok(DeploymentProbeReport {
        deployment_id: deployment_id.to_owned(),
        upstream_model: deployment.upstream_model().to_owned(),
        list_models,
        chat,
        responses,
        chat_function_calling,
        responses_function_calling,
    })
}

struct ProbeSession<'a> {
    deployment: &'a ResolvedDeployment,
    transport: &'a dyn UpstreamTransport,
    adapter: ProviderAdapter,
    headers: HeaderMap,
    max_response_bytes: usize,
}

impl ProbeSession<'_> {
    async fn probe_list_models(&self) -> ListModelsObservation {
        match self
            .send_json(self.adapter.encode_list_models_request())
            .await
        {
            Ok(response) => {
                let Some(entries) = response.body.get("data").and_then(Value::as_array) else {
                    return ListModelsObservation {
                        outcome: ProbeOutcome::unknown(Some(response.status)),
                        configured_model_listed: None,
                        model_ids: Vec::new(),
                    };
                };
                let model_ids = entries
                    .iter()
                    .filter_map(|entry| entry.get("id").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let configured_model_listed = Some(
                    model_ids
                        .iter()
                        .any(|model| model == self.deployment.upstream_model()),
                );
                ListModelsObservation {
                    outcome: ProbeOutcome::supported(response.status),
                    configured_model_listed,
                    model_ids,
                }
            }
            Err(outcome) => ListModelsObservation {
                outcome,
                configured_model_listed: None,
                model_ids: Vec::new(),
            },
        }
    }

    async fn probe_text(&self, protocol: Protocol) -> ProbeOutcome {
        let request = probe_text_request(
            protocol,
            self.deployment.upstream_model(),
            self.probe_max_output_tokens(),
        );
        match self.send_protocol_json(protocol, request).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeOutcome::supported(response.status)
            }
            Ok(response) => ProbeOutcome::unknown(Some(response.status)),
            Err(outcome) => outcome,
        }
    }

    async fn probe_function_calling(&self, protocol: Protocol) -> FunctionCallingObservation {
        let request = probe_tool_request(
            protocol,
            self.deployment.upstream_model(),
            self.probe_max_output_tokens(),
        );
        let response = match self.send_protocol_json(protocol, request).await {
            Ok(response) => response,
            Err(outcome) => {
                return FunctionCallingObservation {
                    initial_call: outcome,
                    result_replay: None,
                };
            }
        };
        let Some(replay) = tool_result_replay_request(
            protocol,
            self.deployment.upstream_model(),
            self.probe_max_output_tokens(),
            &response.body,
        ) else {
            return FunctionCallingObservation {
                initial_call: ProbeOutcome::unknown(Some(response.status)),
                result_replay: None,
            };
        };
        let replay = match self.send_protocol_json(protocol, replay).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeOutcome::supported(response.status)
            }
            Ok(response) => ProbeOutcome::unknown(Some(response.status)),
            Err(outcome) => outcome,
        };
        FunctionCallingObservation {
            initial_call: ProbeOutcome::supported(response.status),
            result_replay: Some(replay),
        }
    }

    async fn send_protocol_json(
        &self,
        protocol: Protocol,
        body: Value,
    ) -> Result<JsonResponse, ProbeOutcome> {
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = ValidatedRequest::new(protocol, Bytes::from(body));
        let request = self
            .adapter
            .encode_request(&request)
            .expect("compiled provider adapter accepts both probe protocols");
        self.send_json(request).await
    }

    async fn send_json(
        &self,
        request: crate::provider::UpstreamRequestParts,
    ) -> Result<JsonResponse, ProbeOutcome> {
        let response = self
            .transport
            .send(self.deployment, request, self.headers.clone())
            .await
            .map_err(|_| ProbeOutcome::unknown(None))?;
        decode_json_response(response, self.max_response_bytes).await
    }

    fn probe_max_output_tokens(&self) -> u32 {
        self.deployment
            .model()
            .context_length()
            .output_tokens()
            .unwrap_or(PROBE_MAX_OUTPUT_TOKENS)
            .min(PROBE_MAX_OUTPUT_TOKENS)
    }
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

async fn decode_json_response(
    response: UpstreamResponse,
    max_response_bytes: usize,
) -> Result<JsonResponse, ProbeOutcome> {
    let status = response.status();
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await
        .map_err(|_| ProbeOutcome::unknown(Some(status)))?;
    if !status.is_success() {
        return Err(ProbeOutcome::from_http_status(status));
    }
    let body = serde_json::from_slice(&body).map_err(|_| ProbeOutcome::unknown(Some(status)))?;
    Ok(JsonResponse { status, body })
}

fn probe_text_request(protocol: Protocol, model: &str, max_output_tokens: u32) -> Value {
    match protocol {
        Protocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        Protocol::Responses => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
    }
}

fn tool_definition(protocol: Protocol) -> Value {
    match protocol {
        Protocol::ChatCompletions => json!({
            "type": "function",
            "function": {
                "name": TOOL_NAME,
                "description": "Return a deterministic local probe value.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                },
            },
        }),
        Protocol::Responses => json!({
            "type": "function",
            "name": TOOL_NAME,
            "description": "Return a deterministic local probe value.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        }),
    }
}

fn probe_tool_request(protocol: Protocol, model: &str, max_output_tokens: u32) -> Value {
    let tools = vec![tool_definition(protocol)];
    match protocol {
        Protocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": "Call the openbridge_probe function."}],
            "tools": tools,
            "tool_choice": {"type": "function", "function": {"name": TOOL_NAME}},
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        Protocol::Responses => json!({
            "model": model,
            "input": "Call the openbridge_probe function.",
            "tools": tools,
            "tool_choice": {"type": "function", "name": TOOL_NAME},
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
    }
}

fn tool_result_replay_request(
    protocol: Protocol,
    model: &str,
    max_output_tokens: u32,
    response: &Value,
) -> Option<Value> {
    match protocol {
        Protocol::ChatCompletions => {
            let message = response.pointer("/choices/0/message")?.clone();
            let tool_calls = message.get("tool_calls")?.as_array()?;
            let call = tool_calls.iter().find(|call| {
                call.pointer("/function/name").and_then(Value::as_str) == Some(TOOL_NAME)
            })?;
            let call_id = call.get("id")?.as_str()?;
            let arguments = call.pointer("/function/arguments")?.as_str()?;
            serde_json::from_str::<Value>(arguments).ok()?;
            Some(json!({
                "model": model,
                "messages": [
                    {"role": "user", "content": "Call the openbridge_probe function."},
                    message,
                    {"role": "tool", "tool_call_id": call_id, "content": "{\"ok\":true}"},
                ],
                "tools": [tool_definition(protocol)],
                "max_completion_tokens": max_output_tokens,
                "stream": false,
            }))
        }
        Protocol::Responses => {
            let output = response.get("output")?.as_array()?;
            let call = output.iter().find(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call")
                    && item.get("name").and_then(Value::as_str) == Some(TOOL_NAME)
            })?;
            let call_id = call.get("call_id")?.as_str()?;
            let arguments = call.get("arguments")?.as_str()?;
            serde_json::from_str::<Value>(arguments).ok()?;
            Some(json!({
                "model": model,
                "input": [{
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": "{\"ok\":true}",
                }],
                "tools": [tool_definition(protocol)],
                "max_output_tokens": max_output_tokens,
                "store": false,
                "stream": false,
            }))
        }
    }
}

fn is_protocol_response(protocol: Protocol, response: &Value) -> bool {
    match protocol {
        Protocol::ChatCompletions => response
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|choices| !choices.is_empty()),
        Protocol::Responses => response.get("object").and_then(Value::as_str) == Some("response"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::body::Body;
    use futures_util::future::BoxFuture;
    use http::{HeaderMap, Method, StatusCode};
    use secrecy::SecretString;
    use serde_json::{Value, json};

    use super::{ProbeSelection, ProbeState, probe_deployment};
    use crate::{
        config::load_registry,
        provider::{CredentialSource, UpstreamRequestParts},
        transport::upstream::{UpstreamError, UpstreamResponse, UpstreamTransport},
    };

    const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
allowed_origins = ["https://api.openai.com"]
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

    const ROUTES: &str = r#"
schema_version = 1
config_version = "probe-test"
[[models]]
id = "openai/test-model"
name = "Test model"
supported_parameters = ["tools"]
reasoning = "unknown"
[models.context_length]
input = 128000
output = 8192
[[providers]]
id = "openai"
kind = "openai"
[providers.credential]
id = "primary"
kind = "api_key"
secret_ref = "env://OPENAI_API_KEY"
[[deployments]]
id = "openai-main"
provider = "openai"
model = "openai/test-model"
upstream_model = "test-model"
endpoint_profile = "public-api"
base_url = "https://api.openai.com"
request_timeout_ms = 120000
[deployments.capabilities.chat_completions]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false

[deployments.capabilities.responses]
enabled = true
streaming = true
function_calling = true
parallel_tool_calls = false
image_input = false
structured_outputs = false
store = false
previous_response_id = false
background = false
[[aliases]]
name = "public-model"
candidates = ["openai-main"]
"#;

    #[derive(Default)]
    struct FixtureTransport {
        requests: Mutex<Vec<(Method, String, Value)>>,
    }

    impl UpstreamTransport for FixtureTransport {
        fn send<'a>(
            &'a self,
            _deployment: &'a crate::config::ResolvedDeployment,
            request: UpstreamRequestParts,
            _headers: HeaderMap,
        ) -> BoxFuture<'a, Result<UpstreamResponse, UpstreamError>> {
            Box::pin(async move {
                let body = if request.body().is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(request.body()).unwrap()
                };
                self.requests.lock().unwrap().push((
                    request.method().clone(),
                    request.relative_uri().path().to_owned(),
                    body.clone(),
                ));
                let response = match request.relative_uri().path() {
                    "/v1/models" => {
                        json!({"object": "list", "data": [{"id": "test-model"}, {"id": "other-model"}]})
                    }
                    "/v1/chat/completions"
                        if body.get("tools").is_some() && !has_tool_result(&body) =>
                    {
                        json!({
                            "object": "chat.completion",
                            "choices": [{"message": {"role": "assistant", "content": null, "tool_calls": [{
                                "id": "call_chat", "type": "function", "function": {"name": "openbridge_probe", "arguments": "{}"}
                            }]}}]
                        })
                    }
                    "/v1/chat/completions" => {
                        json!({"object": "chat.completion", "choices": [{"message": {"role": "assistant", "content": "OK"}}]})
                    }
                    "/v1/responses" if body.get("tools").is_some() && !has_tool_result(&body) => {
                        json!({
                            "object": "response", "output": [{"type": "function_call", "call_id": "call_response", "name": "openbridge_probe", "arguments": "{}"}]
                        })
                    }
                    "/v1/responses" => json!({"object": "response", "output": []}),
                    _ => {
                        return Ok(UpstreamResponse::new(
                            StatusCode::NOT_FOUND,
                            HeaderMap::new(),
                            Body::empty(),
                        ));
                    }
                };
                Ok(UpstreamResponse::new(
                    StatusCode::OK,
                    HeaderMap::new(),
                    Body::from(response.to_string()),
                ))
            })
        }
    }

    fn has_tool_result(body: &Value) -> bool {
        body.get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| message.get("role").and_then(Value::as_str) == Some("tool"))
            })
            || body
                .get("input")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    })
                })
    }

    #[tokio::test]
    async fn probe_discovers_models_and_verifies_both_tool_loops_without_rewriting_configuration() {
        let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
        let transport = FixtureTransport::default();
        let credentials = CredentialSource::fixed("OPENAI_API_KEY", SecretString::from("test-key"));

        let report = probe_deployment(
            &snapshot,
            "openai-main",
            &transport,
            &credentials,
            ProbeSelection::all(),
        )
        .await
        .unwrap();

        let list_models = report.list_models.unwrap();
        assert_eq!(list_models.outcome.state, ProbeState::Supported);
        assert_eq!(list_models.configured_model_listed, Some(true));
        assert_eq!(list_models.model_ids, ["test-model", "other-model"]);
        assert_eq!(report.chat.unwrap().state, ProbeState::Supported);
        assert_eq!(report.responses.unwrap().state, ProbeState::Supported);
        assert_eq!(
            report
                .chat_function_calling
                .unwrap()
                .result_replay
                .unwrap()
                .state,
            ProbeState::Supported
        );
        assert_eq!(
            report
                .responses_function_calling
                .unwrap()
                .result_replay
                .unwrap()
                .state,
            ProbeState::Supported
        );

        let requests = transport.requests.lock().unwrap();
        assert!(
            requests
                .iter()
                .any(|(method, path, _)| method == Method::GET && path == "/v1/models")
        );
        assert!(
            requests
                .iter()
                .filter_map(|(_, path, body)| (path != "/v1/models").then_some(body))
                .all(|body| body.get("model").and_then(Value::as_str) == Some("test-model"))
        );
    }

    #[tokio::test]
    async fn probe_rejects_unknown_deployment_before_any_egress() {
        let snapshot = load_registry(BOOTSTRAP, ROUTES).unwrap();
        let transport = FixtureTransport::default();
        let credentials = CredentialSource::fixed("OPENAI_API_KEY", SecretString::from("test-key"));

        let error = probe_deployment(
            &snapshot,
            "missing",
            &transport,
            &credentials,
            ProbeSelection {
                list_models: true,
                ..ProbeSelection::default()
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, super::ProbeError::UnknownDeployment { .. }));
    }
}
