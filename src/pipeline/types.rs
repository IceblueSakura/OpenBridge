//! 请求事实与 Route 执行计划数据类型。

use crate::{
    bridge::BridgePlan,
    core::{ApiProtocol, ApiRequest, EndpointCapabilities},
    registry::{ReasoningLevel, ReasoningLevelMapping},
};

/// 从下游请求中提取出的、与 registry 无关的路由事实。
#[derive(Debug)]
pub struct RequestRequirements {
    pub(super) public_model: String,
    pub(super) protocol: ApiProtocol,
    pub(super) is_streaming: bool,
    pub(super) requested_output_tokens: Option<u64>,
    pub(super) requested_capabilities: RequestedCapabilities,
}

/// 已完成 Public Model/Route/capability 解析的执行计划。
///
/// candidates 保持 route 配置顺序；`allows_fallback` 不是一般性的重试开关，而是保护
/// `previous_response_id` 等 provider-issued opaque state 不被重放到其他 target。
#[derive(Debug)]
pub struct RoutePlan {
    pub(super) candidates: Vec<RouteCandidate>,
    pub(super) is_streaming: bool,
    pub(super) allows_fallback: bool,
}

/// 一个已通过能力门控、绑定到具体 target/upstream API 的执行候选。
#[derive(Debug)]
pub struct RouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_api_id: String,
    pub(super) request: ApiRequest,
    pub(super) bridge: Option<BridgePlan>,
    pub(super) reasoning_level_mapping: Option<ReasoningLevelMapping>,
}

/// 单次请求实际使用的能力。它不等同于 upstream API 配置：`protocol` 是两个端点共享的
/// 需求，Responses 专有状态单独保留，避免被误用于 Chat Completions 路由。
#[derive(Clone, Copy, Debug)]
pub(super) struct RequestedCapabilities {
    pub(super) protocol: EndpointCapabilities,
    pub(super) unmodeled_tools: bool,
    pub(super) reasoning: RequestedReasoning,
    pub(super) previous_response_id: bool,
    pub(super) background: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RequestedReasoning {
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

    /// 取得保证存在的最高优先级候选。
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

    /// 返回该候选实际应用的 reasoning level 映射。
    pub fn reasoning_level_mapping(&self) -> Option<&ReasoningLevelMapping> {
        self.reasoning_level_mapping.as_ref()
    }
}
