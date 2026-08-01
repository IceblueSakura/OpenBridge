//! 管理员显式执行的上游 capability probe。
//!
//! probe 复用 Upstream Target 的受信 endpoint、credential 和编译期 adapter，但它不走下游
//! HTTP API，也不会修改代码注册表。下游 `/v1/models` 因而始终只列出 Public Model；
//! probe 的 JSON 报告只是服务所有者更新 capability 配置时的证据。

use axum::body::to_bytes;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest},
    provider::{CredentialSource, ProviderAdapter},
    registry::{RuntimeRegistry, UpstreamApi, UpstreamTarget},
    transport::upstream::{UpstreamResponse, UpstreamTransport},
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;
const PROBE_PROMPT: &str = "Reply with exactly OK.";
const TOOL_NAME: &str = "openbridge_probe";

/// 明确选择要执行的 probe。CLI 不传任何选择时使用 `all()`；库调用方可仅执行无费用的
/// `list_models`，或只验证特定协议。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProbeOptions {
    /// 是否执行 `/v1/models` probe。
    pub list_models: bool,
    /// 是否执行 Chat Completions 文本请求 probe。
    pub chat: bool,
    /// 是否执行 Responses 文本请求 probe。
    pub responses: bool,
    /// 是否执行 function call 及结果回放 probe。
    pub function_calling: bool,
}

impl ProbeOptions {
    /// 选择全部已实现的 probe。
    pub const fn all() -> Self {
        Self {
            list_models: true,
            chat: true,
            responses: true,
            function_calling: true,
        }
    }

    /// 判断是否没有选择任何 probe。
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
pub enum SupportStatus {
    /// 请求符合该 probe 预期的协议形状。
    Supported,
    /// endpoint 明确返回不支持该操作的 status。
    Unsupported,
    /// 请求失败或响应形状不足以作出结论。
    Unknown,
}

/// 单项 probe 的状态和可选 HTTP status。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProbeResult {
    /// 本次 probe 的保守结论。
    pub state: SupportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 上游返回的 HTTP status；尚未收到响应时为空。
    pub http_status: Option<u16>,
}

impl ProbeResult {
    const fn supported(status: StatusCode) -> Self {
        Self {
            state: SupportStatus::Supported,
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
                SupportStatus::Unsupported
            } else {
                SupportStatus::Unknown
            },
            http_status: Some(status.as_u16()),
        }
    }

    const fn unknown(status: Option<StatusCode>) -> Self {
        Self {
            state: SupportStatus::Unknown,
            http_status: match status {
                Some(status) => Some(status.as_u16()),
                None => None,
            },
        }
    }
}

/// `/v1/models` probe 的模型列表观察结果。
#[derive(Debug, Serialize)]
pub struct ModelListProbeResult {
    /// `/v1/models` 请求本身的结论。
    pub outcome: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 配置的 upstream model 是否出现在返回列表中。
    pub configured_model_listed: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// 从响应中提取的 model id，可能为空或不完整。
    pub model_ids: Vec<String>,
}

/// function calling probe 及其 tool-result replay 的观察结果。
#[derive(Debug, Serialize)]
pub struct ToolCallProbeResult {
    /// 初始 function call 请求结论。
    pub initial_call: ProbeResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 将 tool result 回放后的请求结论。
    pub result_replay: Option<ProbeResult>,
}

/// 单个 Upstream Target 的 probe 报告。它不包含 credential、请求正文或上游响应正文。
#[derive(Debug, Serialize)]
pub struct TargetProbeReport {
    /// 被 probe 的内部 target id。
    pub upstream_target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `/v1/models` 的观察结果。
    pub list_models: Option<ModelListProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Chat Completions 文本 probe 的观察结果。
    pub chat: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Responses 文本 probe 的观察结果。
    pub responses: Option<ProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Chat Completions function calling probe 的观察结果。
    pub chat_function_calling: Option<ToolCallProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Responses function calling probe 的观察结果。
    pub responses_function_calling: Option<ToolCallProbeResult>,
}

#[derive(Debug, Error)]
/// probe 准备阶段失败。
pub enum ProbeError {
    /// 请求的 Upstream Target 未注册。
    #[error("configured upstream target '{upstream_target}' does not exist")]
    UnknownUpstreamTarget {
        /// 未找到的内部 target id。
        upstream_target: String,
    },
    /// 受信 credential source 无法提供所需 secret。
    #[error("upstream credentials are unavailable for probe")]
    CredentialUnavailable,
    /// adapter 无法为 probe 构造认证 header。
    #[error("provider authentication could not be prepared for probe")]
    AuthenticationPreparation,
}

/// 使用与数据面相同的受信配置执行选定 probe。
///
/// 该函数只访问 `upstream_target_id` 对应的固定 endpoint；没有接受 URL、model 或 header 的
/// 外部参数，避免诊断能力扩大 SSRF 或 credential 使用范围。
pub async fn probe_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialSource,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // 从不可变 registry 解析 target，并按其 binding 读取短时 credential。
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    let credential = credentials
        .resolve(
            target.kind(),
            target.credential().id(),
            target.credential().secret_reference().locator(),
        )
        .map_err(|_| ProbeError::CredentialUnavailable)?;
    // 选择编译期 adapter 并准备 probe 所需的敏感出站 header。
    let adapter = ProviderAdapter::for_kind(target.kind());
    let headers = adapter
        .build_outbound_headers(&credential)
        .map_err(|_| ProbeError::AuthenticationPreparation)?;
    let session = ProbeSession {
        target,
        transport,
        adapter,
        headers,
        max_response_bytes: registry.limits().max_request_body_bytes(),
    };

    // 独立执行每项 probe；单项失败只体现在该项 outcome，不阻断其余观察。
    let list_models = if selection.list_models {
        Some(session.probe_list_models().await)
    } else {
        None
    };
    let chat = if selection.chat {
        Some(session.probe_text(ApiProtocol::ChatCompletions).await)
    } else {
        None
    };
    let responses = if selection.responses {
        Some(session.probe_text(ApiProtocol::Responses).await)
    } else {
        None
    };
    let chat_function_calling = if selection.function_calling {
        Some(
            session
                .probe_function_calling(ApiProtocol::ChatCompletions)
                .await,
        )
    } else {
        None
    };
    let responses_function_calling = if selection.function_calling {
        Some(session.probe_function_calling(ApiProtocol::Responses).await)
    } else {
        None
    };

    Ok(TargetProbeReport {
        upstream_target_id: upstream_target_id.to_owned(),
        list_models,
        chat,
        responses,
        chat_function_calling,
        responses_function_calling,
    })
}

struct ProbeSession<'a> {
    target: &'a UpstreamTarget,
    transport: &'a dyn UpstreamTransport,
    adapter: ProviderAdapter,
    headers: HeaderMap,
    max_response_bytes: usize,
}

impl ProbeSession<'_> {
    async fn probe_list_models(&self) -> ModelListProbeResult {
        // 发送固定模型列表请求并提取 model id。
        match self
            .send_json(self.adapter.prepare_model_list_request())
            .await
        {
            Ok(response) => {
                let Some(entries) = response.body.get("data").and_then(Value::as_array) else {
                    return ModelListProbeResult {
                        outcome: ProbeResult::unknown(Some(response.status)),
                        configured_model_listed: None,
                        model_ids: Vec::new(),
                    };
                };
                let model_ids = entries
                    .iter()
                    .filter_map(|entry| entry.get("id").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let configured_model_listed =
                    Some(self.target.upstream_apis().any(|(_, upstream_api)| {
                        model_ids
                            .iter()
                            .any(|model| model == upstream_api.upstream_model())
                    }));
                ModelListProbeResult {
                    outcome: ProbeResult::supported(response.status),
                    configured_model_listed,
                    model_ids,
                }
            }
            Err(outcome) => ModelListProbeResult {
                outcome,
                configured_model_listed: None,
                model_ids: Vec::new(),
            },
        }
    }

    async fn probe_text(&self, protocol: ApiProtocol) -> ProbeResult {
        // 按协议找到 target API，并构造最小文本请求。
        let Some(upstream_api) = self.target.upstream_api_for_protocol(protocol) else {
            return ProbeResult {
                state: SupportStatus::Unsupported,
                http_status: None,
            };
        };
        let request = probe_text_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
        );
        // 仅在响应形状符合目标协议时报告 supported。
        match self.send_protocol_json(protocol, request).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeResult::supported(response.status)
            }
            Ok(response) => ProbeResult::unknown(Some(response.status)),
            Err(outcome) => outcome,
        }
    }

    async fn probe_function_calling(&self, protocol: ApiProtocol) -> ToolCallProbeResult {
        // 发送 function call 请求并提取可回放的 tool call。
        let Some(upstream_api) = self.target.upstream_api_for_protocol(protocol) else {
            return ToolCallProbeResult {
                initial_call: ProbeResult {
                    state: SupportStatus::Unsupported,
                    http_status: None,
                },
                result_replay: None,
            };
        };
        let request = probe_tool_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
        );
        let response = match self.send_protocol_json(protocol, request).await {
            Ok(response) => response,
            Err(outcome) => {
                return ToolCallProbeResult {
                    initial_call: outcome,
                    result_replay: None,
                };
            }
        };
        let Some(replay) = tool_result_replay_request(
            protocol,
            upstream_api.upstream_model(),
            self.probe_max_output_tokens(upstream_api),
            &response.body,
        ) else {
            return ToolCallProbeResult {
                initial_call: ProbeResult::unknown(Some(response.status)),
                result_replay: None,
            };
        };
        // 回放 tool result，确认第二个请求仍返回目标协议响应。
        let replay = match self.send_protocol_json(protocol, replay).await {
            Ok(response) if is_protocol_response(protocol, &response.body) => {
                ProbeResult::supported(response.status)
            }
            Ok(response) => ProbeResult::unknown(Some(response.status)),
            Err(outcome) => outcome,
        };
        ToolCallProbeResult {
            initial_call: ProbeResult::supported(response.status),
            result_replay: Some(replay),
        }
    }

    async fn send_protocol_json(
        &self,
        protocol: ApiProtocol,
        body: Value,
    ) -> Result<JsonResponse, ProbeResult> {
        // 序列化 probe body，并交给编译期 adapter 绑定上游模型和相对 path。
        let body = serde_json::to_vec(&body).expect("probe request JSON is serializable");
        let request = ApiRequest::new(protocol, Bytes::from(body));
        let request = self
            .adapter
            .prepare_request(
                &request,
                self.target
                    .upstream_api_for_protocol(protocol)
                    .expect("probe protocol has a configured upstream API")
                    .upstream_model(),
            )
            .expect("compiled provider adapter accepts both probe protocols");
        // 通过受信 transport 发送并按统一 body 上限解码响应。
        self.send_json(request).await
    }

    async fn send_json(
        &self,
        request: crate::provider::PreparedUpstreamRequest,
    ) -> Result<JsonResponse, ProbeResult> {
        // 发送请求并将 transport failure 转换为保守的 unknown outcome。
        let response = self
            .transport
            .send(self.target, request, self.headers.clone())
            .await
            .map_err(|_| ProbeResult::unknown(None))?;
        // 校验 status 和 JSON body，避免由无效响应推导 capability 结论。
        decode_json_response(response, self.max_response_bytes).await
    }

    fn probe_max_output_tokens(&self, upstream_api: &UpstreamApi) -> u32 {
        upstream_api
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
) -> Result<JsonResponse, ProbeResult> {
    // 在配置上限内读取 response body，并先区分 HTTP failure。
    let status = response.status();
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await
        .map_err(|_| ProbeResult::unknown(Some(status)))?;
    if !status.is_success() {
        return Err(ProbeResult::from_http_status(status));
    }
    // 只接受合法 JSON，确保 probe 报告不把错误页面当成协议成功。
    let body = serde_json::from_slice(&body).map_err(|_| ProbeResult::unknown(Some(status)))?;
    Ok(JsonResponse { status, body })
}

fn probe_text_request(protocol: ApiProtocol, model: &str, max_output_tokens: u32) -> Value {
    match protocol {
        ApiProtocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": PROBE_PROMPT}],
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        ApiProtocol::Responses => json!({
            "model": model,
            "input": PROBE_PROMPT,
            "max_output_tokens": max_output_tokens,
            "store": false,
            "stream": false,
        }),
    }
}

fn tool_definition(protocol: ApiProtocol) -> Value {
    match protocol {
        ApiProtocol::ChatCompletions => json!({
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
        ApiProtocol::Responses => json!({
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

fn probe_tool_request(protocol: ApiProtocol, model: &str, max_output_tokens: u32) -> Value {
    let tools = vec![tool_definition(protocol)];
    match protocol {
        ApiProtocol::ChatCompletions => json!({
            "model": model,
            "messages": [{"role": "user", "content": "Call the openbridge_probe function."}],
            "tools": tools,
            "tool_choice": {"type": "function", "function": {"name": TOOL_NAME}},
            "max_completion_tokens": max_output_tokens,
            "stream": false,
        }),
        ApiProtocol::Responses => json!({
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
    protocol: ApiProtocol,
    model: &str,
    max_output_tokens: u32,
    response: &Value,
) -> Option<Value> {
    match protocol {
        ApiProtocol::ChatCompletions => {
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
        ApiProtocol::Responses => {
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

fn is_protocol_response(protocol: ApiProtocol, response: &Value) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => response
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|choices| !choices.is_empty()),
        ApiProtocol::Responses => {
            response.get("object").and_then(Value::as_str) == Some("response")
        }
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

    use super::{ProbeOptions, SupportStatus, probe_upstream_target};
    use crate::{
        config::parse_bootstrap_config,
        provider::{CredentialSource, PreparedUpstreamRequest},
        providers,
        registry::{RuntimeRegistry, UpstreamTarget, build_registry},
        transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
    };

    const BOOTSTRAP: &str = r#"
schema_version = 1
listen = "127.0.0.1:8080"
users_file = "config/users.toml"
max_request_body_bytes = 1048576
max_sse_event_bytes = 262144
upstream_connect_timeout_ms = 5000
upstream_pool_idle_timeout_ms = 90000
upstream_pool_max_idle_per_host = 16
"#;

    fn registry() -> RuntimeRegistry {
        let mut definition = providers::compiled_config();
        definition.version = "probe-test".to_owned();
        for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
            upstream_api.upstream_model = "test-model".to_owned();
        }
        build_registry(parse_bootstrap_config(BOOTSTRAP).unwrap(), definition).unwrap()
    }

    #[derive(Default)]
    struct FixtureTransport {
        requests: Mutex<Vec<(Method, String, Value)>>,
    }

    impl UpstreamTransport for FixtureTransport {
        fn send<'a>(
            &'a self,
            _target: &'a UpstreamTarget,
            request: PreparedUpstreamRequest,
            _headers: HeaderMap,
        ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
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
        let registry = registry();
        let transport = FixtureTransport::default();
        let credentials = CredentialSource::fixed("OPENAI_API_KEY", SecretString::from("test-key"));

        let report = probe_upstream_target(
            &registry,
            "openai-main",
            &transport,
            &credentials,
            ProbeOptions::all(),
        )
        .await
        .unwrap();

        let list_models = report.list_models.unwrap();
        assert_eq!(list_models.outcome.state, SupportStatus::Supported);
        assert_eq!(list_models.configured_model_listed, Some(true));
        assert_eq!(list_models.model_ids, ["test-model", "other-model"]);
        assert_eq!(report.chat.unwrap().state, SupportStatus::Supported);
        assert_eq!(report.responses.unwrap().state, SupportStatus::Supported);
        assert_eq!(
            report
                .chat_function_calling
                .unwrap()
                .result_replay
                .unwrap()
                .state,
            SupportStatus::Supported
        );
        assert_eq!(
            report
                .responses_function_calling
                .unwrap()
                .result_replay
                .unwrap()
                .state,
            SupportStatus::Supported
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
    async fn probe_rejects_unknown_target_before_any_egress() {
        let registry = registry();
        let transport = FixtureTransport::default();
        let credentials = CredentialSource::fixed("OPENAI_API_KEY", SecretString::from("test-key"));

        let error = probe_upstream_target(
            &registry,
            "missing",
            &transport,
            &credentials,
            ProbeOptions {
                list_models: true,
                ..ProbeOptions::default()
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            super::ProbeError::UnknownUpstreamTarget { .. }
        ));
    }
}
