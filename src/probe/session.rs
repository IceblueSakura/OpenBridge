//! 固定 Upstream Target 上执行 capability probe 的受信会话。
//!
//! 会话只使用编译注册表中的 endpoint、model、adapter 与启动 credential 快照；单项网络或
//! 协议失败只形成保守 outcome，不阻断同一报告中的其他 probe。

use axum::body::to_bytes;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use serde_json::Value;

use crate::{
    core::{ApiProtocol, ApiRequest},
    credential::CredentialStore,
    provider::ProviderAdapter,
    registry::{RuntimeRegistry, UpstreamApi, UpstreamTarget},
    transport::upstream::{UpstreamResponse, UpstreamTransport},
};

use super::{
    ModelListProbeResult, ProbeError, ProbeOptions, ProbeResult, SupportStatus, TargetProbeReport,
    ToolCallProbeResult,
    payload::{
        is_protocol_response, probe_text_request, probe_tool_request, tool_result_replay_request,
    },
};

const PROBE_MAX_OUTPUT_TOKENS: u32 = 16;

/// 使用与数据面相同的受信配置执行选定 probe。
///
/// 该函数只访问 `upstream_target_id` 对应的固定 endpoint；没有接受 URL、model 或 header 的
/// 外部参数，避免诊断能力扩大 SSRF 或 credential 使用范围。
pub async fn probe_upstream_target(
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
    transport: &dyn UpstreamTransport,
    credentials: &CredentialStore,
    selection: ProbeOptions,
) -> Result<TargetProbeReport, ProbeError> {
    // 从不可变 registry 解析 target，并按其 binding 借用启动 credential 快照。
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or_else(|| ProbeError::UnknownUpstreamTarget {
            upstream_target: upstream_target_id.to_owned(),
        })?;
    let credential = credentials
        .upstream(target.kind(), target.credential().id())
        .map_err(|_| ProbeError::CredentialUnavailable)?;

    // 选择编译期 adapter 并准备 probe 所需的敏感出站 header。
    let adapter = ProviderAdapter::for_kind(target.kind());
    let headers = adapter
        .build_outbound_headers(&credential, &HeaderMap::new())
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

    // 汇总不含 credential、请求正文或响应正文的结构化报告。
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
    /// 查询固定模型列表 endpoint，并提取可见 model ids。
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

    /// 执行目标协议的最小非流式文本请求。
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

    /// 执行 function call 与 tool-result replay 的双请求 probe。
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

    /// 让编译期 adapter 绑定协议请求并通过受信 transport 发送。
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

    /// 发送已准备请求，并将 transport/HTTP/JSON 失败归一化为保守 outcome。
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

    /// 把 probe 输出上限收窄到模型声明值与固定安全上限的交集。
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

/// 在固定上限内读取成功 JSON response，并先分类 HTTP failure。
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
