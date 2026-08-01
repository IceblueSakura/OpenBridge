//! 将 OpenAI-compatible 请求分解为请求事实，并基于固定注册表规划执行候选。
//!
//! pipeline 先验证 JSON、提取 public `model` 与请求实际使用的 capability，再沿 Public
//! Model 的有序 Route 生成 `RoutePlan`。Native candidate 保留原始协议；Bridged candidate
//! 生成受限 `BridgePlan`。Provider-specific endpoint/header/auth 改写仍由 adapter 完成。

use bytes::Bytes;
use serde_json::Value;
use thiserror::Error;

use crate::{
    bridge::BridgePlan,
    core::{ApiProtocol, ApiRequest, EndpointCapabilities},
    registry::{
        ReasoningLevel, ReasoningSupport, RouteMode, RuntimeRegistry, UpstreamApiCapabilities,
    },
};

/// 请求不能被安全地绑定到兼容 Route 时返回的规划错误。
#[derive(Debug, Error)]
pub enum RequestPlanningError {
    /// 请求 body 不是 JSON object。
    #[error("request body must be a JSON object")]
    InvalidJson,
    /// 请求缺少非空的 public model。
    #[error("request body must contain a non-empty model")]
    MissingModel,
    /// 请求的 public model 未在 registry 中注册。
    #[error("requested model is not configured")]
    UnknownModel,
    /// Public Model 没有可用的 route。
    #[error("configured model has no route candidate")]
    NoRoute,
    /// route 与请求协议不匹配。
    #[error("selected route does not support this protocol")]
    UnsupportedProtocol,
    /// route 不支持请求的 streaming 模式。
    #[error("selected route does not support streaming")]
    StreamingUnsupported,
    /// route 不支持请求声明的 capability。
    #[error("selected route does not support requested capabilities")]
    UnsupportedCapabilities,
    /// 请求的最大输出超过了生效上限。
    #[error("requested maximum output exceeds the configured model limit")]
    OutputLimitExceeded,
    /// 模型不支持请求的 reasoning。
    #[error("selected model does not support requested reasoning")]
    ReasoningUnsupported,
    /// 模型不支持请求的 reasoning level。
    #[error("selected model does not support the requested reasoning level")]
    ReasoningLevelUnsupported,
}

/// 从下游请求中提取出的、与 registry 无关的路由事实。
#[derive(Debug)]
pub struct RequestRequirements {
    public_model: String,
    protocol: ApiProtocol,
    is_streaming: bool,
    requested_output_tokens: Option<u64>,
    requested_capabilities: RequestedCapabilities,
}

/// 已完成 Public Model/Route/capability 解析的执行计划。
///
/// candidates 保持 route 配置顺序；`allows_fallback` 不是一般性的重试开关，而是保护
/// `previous_response_id` 等 provider-issued opaque state 不被重放到其他 target。
#[derive(Debug)]
pub struct RoutePlan {
    candidates: Vec<RouteCandidate>,
    is_streaming: bool,
    allows_fallback: bool,
}

/// 一个已通过能力门控、绑定到具体 target/upstream API 的执行候选。
#[derive(Debug)]
pub struct RouteCandidate {
    route_id: String,
    upstream_target_id: String,
    upstream_api_id: String,
    request: ApiRequest,
    bridge: Option<BridgePlan>,
}

/// 单次请求实际使用的能力。它不等同于 upstream API 配置：`protocol` 是两个端点共享的
/// 需求，Responses 专有状态单独保留，避免被误用于 Chat Completions 路由。
#[derive(Clone, Copy, Debug)]
struct RequestedCapabilities {
    protocol: EndpointCapabilities,
    unmodeled_tools: bool,
    reasoning: RequestedReasoning,
    previous_response_id: bool,
    background: bool,
}

#[derive(Clone, Copy, Debug)]
enum RequestedReasoning {
    None,
    Unspecified,
    Level(ReasoningLevel),
    UnknownLevel,
}

impl RequestRequirements {
    /// 返回下游请求选择的 public model 名称。
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// 返回请求使用的原生协议。
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// 判断请求是否要求 streaming response。
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }
}

impl RoutePlan {
    /// 返回优先级最高的 target id。
    pub fn upstream_target_id(&self) -> &str {
        self.primary().upstream_target_id()
    }

    /// 返回优先候选对应的请求。
    pub fn request(&self) -> &ApiRequest {
        self.primary().request()
    }

    /// 返回按 route 顺序排列的兼容候选。
    pub fn candidates(&self) -> &[RouteCandidate] {
        &self.candidates
    }

    /// 判断原请求是否要求 streaming。
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    /// 判断是否允许跨 target fallback。
    pub fn allows_fallback(&self) -> bool {
        self.allows_fallback
    }

    /// 消费计划并取出其优先候选请求。
    pub fn into_request(self) -> ApiRequest {
        self.candidates
            .into_iter()
            .next()
            .expect("route plan always has a candidate")
            .request
    }

    fn primary(&self) -> &RouteCandidate {
        self.candidates
            .first()
            .expect("route plan always has a candidate")
    }
}

impl RouteCandidate {
    /// 返回候选 route id。
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// 返回候选绑定的 Upstream Target id。
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// 返回候选绑定的 Upstream API id。
    pub fn upstream_api_id(&self) -> &str {
        &self.upstream_api_id
    }

    /// 返回候选对应的原生请求。
    pub fn request(&self) -> &ApiRequest {
        &self.request
    }

    /// 返回 Bridged Route 的响应转换计划；Native candidate 返回 `None`。
    pub fn bridge(&self) -> Option<&BridgePlan> {
        self.bridge.as_ref()
    }
}

/// 解析下游请求并提取独立于 registry 的请求事实。
///
/// 此阶段不选择 route，也不改写请求正文。
pub fn analyze_request(
    protocol: ApiProtocol,
    body: &Bytes,
) -> Result<RequestRequirements, RequestPlanningError> {
    // 解析 JSON object 并提取 public model 与 stream 标志。
    let document: Value =
        serde_json::from_slice(body).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object()
        .ok_or(RequestPlanningError::InvalidJson)?;
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or(RequestPlanningError::MissingModel)?;
    let is_streaming = object.get("stream").and_then(Value::as_bool) == Some(true);
    // 根据协议字段推导请求实际使用的 capability。
    let requested_output_tokens = requested_output_tokens(object);
    let requests_function_calling = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(is_function_tool));
    let requests_unmodeled_tools = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(|tool| !is_function_tool(tool)));
    let requested_capabilities = RequestedCapabilities {
        protocol: EndpointCapabilities {
            enabled: false,
            streaming: is_streaming,
            function_calling: requests_function_calling,
            parallel_tool_calls: requests_function_calling
                && object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true),
            image_input: requests_image_input(protocol, object),
            structured_outputs: requests_structured_outputs(object),
            store: object.get("store").and_then(Value::as_bool) == Some(true),
        },
        unmodeled_tools: requests_unmodeled_tools,
        reasoning: requested_reasoning(object),
        previous_response_id: protocol == ApiProtocol::Responses
            && object
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null()),
        background: protocol == ApiProtocol::Responses
            && object.get("background").and_then(Value::as_bool) == Some(true),
    };
    // 固化请求事实，后续 route 规划不再重新解释 body。
    Ok(RequestRequirements {
        public_model: public_model.to_owned(),
        protocol,
        is_streaming,
        requested_output_tokens,
        requested_capabilities,
    })
}

/// 沿 Public Model 的有序 Route 生成 Native 或 Bridged 执行计划。
///
/// Native 请求字段除后续 adapter 改写的 `model` 外保持原样；Bridged 请求只转换明确
/// allowlist 内的共同语义。capability gate 和 Bridge preflight 都在 egress 前完成。
pub fn plan_request(
    registry: &RuntimeRegistry,
    profile: &RequestRequirements,
    body: Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    // 解析 Public Model 的有序 route 引用。
    let routes = registry
        .public_model(profile.public_model())
        .ok_or(RequestPlanningError::UnknownModel)?
        .routes();
    // 按 route 顺序执行协议、target 状态和 capability gate。
    let mut protocol_mismatch_seen = false;
    let mut first_candidate_error = None;
    let mut prepared_candidates = Vec::new();
    for route_id in routes {
        let route = registry
            .route(route_id)
            .ok_or(RequestPlanningError::NoRoute)?;
        if route.downstream_protocol() != profile.protocol() {
            protocol_mismatch_seen = true;
            continue;
        }
        let target = registry
            .upstream_target(route.upstream_target())
            .ok_or(RequestPlanningError::NoRoute)?;
        if !target.enabled() {
            continue;
        }
        let upstream_api = target
            .upstream_api(route.upstream_api())
            .ok_or(RequestPlanningError::NoRoute)?;
        if let Some(error) = candidate_error(
            upstream_api.protocol(),
            profile.requested_capabilities,
            upstream_api.capabilities(),
            upstream_api.model().context_length().output_tokens(),
            upstream_api.model().reasoning(),
            upstream_api.model().reasoning_levels(),
            profile.requested_output_tokens,
        ) {
            first_candidate_error.get_or_insert(error);
            continue;
        }
        let (request, bridge) = match route.mode() {
            RouteMode::Native => (ApiRequest::new(profile.protocol, body.clone()), None),
            RouteMode::Bridged => match BridgePlan::prepare(
                profile.protocol,
                upstream_api.protocol(),
                profile.public_model(),
                upstream_api.upstream_model(),
                body.clone(),
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => {
                    first_candidate_error
                        .get_or_insert(RequestPlanningError::UnsupportedCapabilities);
                    continue;
                }
            },
        };
        prepared_candidates.push(RouteCandidate {
            route_id: route_id.clone(),
            upstream_target_id: route.upstream_target().to_owned(),
            upstream_api_id: route.upstream_api().to_owned(),
            request,
            bridge,
        });
    }
    // 没有候选时返回最具体的规划错误，否则构造带 fallback 边界的计划。
    if prepared_candidates.is_empty() {
        return Err(first_candidate_error.unwrap_or(if protocol_mismatch_seen {
            RequestPlanningError::UnsupportedProtocol
        } else {
            RequestPlanningError::NoRoute
        }));
    }

    Ok(RoutePlan {
        candidates: prepared_candidates,
        is_streaming: profile.is_streaming,
        allows_fallback: !profile.requested_capabilities.previous_response_id,
    })
}

fn candidate_error(
    protocol: ApiProtocol,
    requested_features: RequestedCapabilities,
    capabilities: UpstreamApiCapabilities,
    configured_max_output_tokens: Option<u32>,
    reasoning: ReasoningSupport,
    reasoning_levels: &[ReasoningLevel],
    requested_output_tokens: Option<u64>,
) -> Option<RequestPlanningError> {
    let protocol_capabilities = capabilities.protocol_capabilities();
    if !protocol_capabilities.enabled {
        return Some(RequestPlanningError::UnsupportedProtocol);
    }
    if requested_features.unmodeled_tools {
        return Some(RequestPlanningError::UnsupportedCapabilities);
    }
    if requested_features.protocol.streaming && !protocol_capabilities.streaming {
        return Some(RequestPlanningError::StreamingUnsupported);
    }
    if !requested_features
        .protocol
        .is_subset_of(protocol_capabilities)
    {
        return Some(RequestPlanningError::UnsupportedCapabilities);
    }
    if protocol == ApiProtocol::Responses {
        let Some(responses) = capabilities.responses() else {
            return Some(RequestPlanningError::UnsupportedProtocol);
        };
        if (requested_features.previous_response_id && !responses.previous_response_id)
            || (requested_features.background && !responses.background)
        {
            return Some(RequestPlanningError::UnsupportedCapabilities);
        }
    }
    if configured_max_output_tokens.is_some_and(|limit| {
        requested_output_tokens.is_some_and(|requested| requested > u64::from(limit))
    }) {
        return Some(RequestPlanningError::OutputLimitExceeded);
    }
    match requested_features.reasoning {
        RequestedReasoning::None => {}
        RequestedReasoning::Unspecified if reasoning != ReasoningSupport::Supported => {
            return Some(RequestPlanningError::ReasoningUnsupported);
        }
        RequestedReasoning::Level(level)
            if reasoning != ReasoningSupport::Supported || !reasoning_levels.contains(&level) =>
        {
            return Some(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::UnknownLevel => {
            return Some(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::Unspecified | RequestedReasoning::Level(_) => {}
    }
    None
}

/// 仅识别 OpenAI-compatible message/input item 内的 image content part，而不尝试在热路径计算 token。
///
/// `image_url`（Chat）和 `input_image`（Responses）是协议字段；未知 part 会被原生
/// 透传，因此不能依据其他任意 JSON 中同名 `type` 推测视觉能力。
fn requests_image_input(protocol: ApiProtocol, object: &serde_json::Map<String, Value>) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => object
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages
                    .iter()
                    .any(|message| content_contains_part_type(message.get("content"), "image_url"))
            }),
        ApiProtocol::Responses => {
            object
                .get("input")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("input_image")
                            || content_contains_part_type(item.get("content"), "input_image")
                    })
                })
        }
    }
}

/// 判断 content 数组中是否存在指定协议 part type。
fn content_contains_part_type(content: Option<&Value>, expected_type: &str) -> bool {
    content.and_then(Value::as_array).is_some_and(|parts| {
        parts
            .iter()
            .any(|part| part.get("type").and_then(Value::as_str) == Some(expected_type))
    })
}

/// `function_calling` 只覆盖 OpenAI 的 JSON-schema function tools。Built-in 和 custom
/// tools 需要各自的配置语义与 probe，尚未建模时不得借由 `tools[]` 原生透传而被误认为
/// 已支持。
fn is_function_tool(tool: &Value) -> bool {
    tool.get("type").and_then(Value::as_str) == Some("function")
}

/// Chat 兼容面存在新旧两个输出上限字段；当客户端同时给出多个字段时，选择其中最大的
/// 值做本地上界校验，绝不悄悄改写上游请求。字段值不是非负整数时仍交由上游协议验证。
fn requested_output_tokens(object: &serde_json::Map<String, Value>) -> Option<u64> {
    ["max_output_tokens", "max_completion_tokens", "max_tokens"]
        .iter()
        .filter_map(|field| object.get(*field).and_then(Value::as_u64))
        .max()
}

/// `reasoning` 是 OpenAI-compatible 请求中的模型级能力；`reasoning_effort` 同样代表
/// 调用方要求使用该能力。没有该字段时不得据模型目录推测调用方需要 reasoning。
fn requested_reasoning(object: &serde_json::Map<String, Value>) -> RequestedReasoning {
    if let Some(value) = object
        .get("reasoning_effort")
        .filter(|value| !value.is_null())
    {
        return value
            .as_str()
            .and_then(ReasoningLevel::from_wire)
            .map(RequestedReasoning::Level)
            .unwrap_or(RequestedReasoning::UnknownLevel);
    }
    let Some(reasoning) = object
        .get("reasoning")
        .filter(|value| !value.is_null() && *value != &Value::Bool(false))
    else {
        return RequestedReasoning::None;
    };
    let Some(effort) = reasoning
        .as_object()
        .and_then(|reasoning| reasoning.get("effort"))
    else {
        return RequestedReasoning::Unspecified;
    };
    effort
        .as_str()
        .and_then(ReasoningLevel::from_wire)
        .map(RequestedReasoning::Level)
        .unwrap_or(RequestedReasoning::UnknownLevel)
}

/// 识别 response format、text format 或 strict function tool 对结构化输出的请求。
fn requests_structured_outputs(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("response_format")
        .is_some_and(is_non_text_format)
        || object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format"))
            .is_some_and(is_non_text_format)
        || object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| tools.iter().any(tool_requests_strict_mode))
}

/// Chat Completions 将 strict 放在 `function` 内，Responses 则直接放在 function tool 上。
/// 两种 wire 形状都属于 Structured Outputs 语义，因此都需要 `structured_outputs` 能力。
fn tool_requests_strict_mode(tool: &Value) -> bool {
    tool.get("strict").and_then(Value::as_bool) == Some(true)
        || tool
            .get("function")
            .and_then(Value::as_object)
            .and_then(|function| function.get("strict"))
            .and_then(Value::as_bool)
            == Some(true)
}

/// 判断 format object 是否显式要求非纯文本输出。
fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
