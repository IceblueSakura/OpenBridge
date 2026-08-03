//! 基于不可变 registry 生成有序 Native/Bridged Route 候选。

use bytes::Bytes;
use serde_json::Value;

use crate::{
    bridge::BridgePlan,
    core::ApiRequest,
    registry::{
        ModelInterfaceCapabilities, ReasoningLevel, ReasoningLevelMapping, RouteMode,
        RuntimeRegistry, SupportState, UpstreamApi,
    },
};

use super::{
    error::RequestPlanningError,
    types::{
        RequestRequirements, RequestedCapabilities, RequestedReasoning, RouteCandidate, RoutePlan,
    },
};

/// 沿 Public Model 的有序 Route 生成 Native 或 Bridged 执行计划。
///
/// Native 请求字段除后续 adapter 改写的 `model` 外保持原样；Bridged 请求只转换明确
/// allowlist 内的共同语义。Public Model 能力只预检一次；任一 BridgePlan 失败会拒绝整个
/// 请求，不会成为跳过该 Route 的条件。
pub fn plan_request(
    registry: &RuntimeRegistry,
    requirements: &RequestRequirements,
    body: Bytes,
) -> Result<RoutePlan, RequestPlanningError> {
    // 解析 Public Model，并在查看任何 Route 前按其唯一接口契约完成能力预检。
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(RequestPlanningError::UnknownModel)?;
    let interface = public_model
        .interface(requirements.protocol())
        .ok_or(RequestPlanningError::UnsupportedProtocol)?;
    if let Some(error) = public_model_preflight_error(
        requirements.requested_capabilities,
        requirements.requested_output_tokens,
        interface,
    ) {
        return Err(error);
    }

    // 严格按配置顺序构造可执行 Route；请求能力不改变候选资格或顺序。
    let mut protocol_mismatch_seen = false;
    let mut prepared_candidates = Vec::new();
    for route_id in public_model.routes() {
        let route = registry
            .route(route_id)
            .ok_or(RequestPlanningError::NoRoute)?;
        if route.downstream_protocol() != requirements.protocol() {
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
        if !upstream_api
            .capabilities()
            .generation_capabilities()
            .enabled
        {
            continue;
        }
        let (request, bridge) = match route.mode() {
            RouteMode::Native => (ApiRequest::new(requirements.protocol, body.clone()), None),
            RouteMode::Bridged => match BridgePlan::prepare_with_reasoning_output(
                requirements.protocol,
                upstream_api.protocol(),
                requirements.public_model(),
                upstream_api.upstream_model(),
                body.clone(),
                upstream_api.reasoning_output(),
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => return Err(RequestPlanningError::UnsupportedCapabilities),
            },
        };
        let (request, reasoning_level_mapping) = apply_reasoning_level_mapping(
            request,
            requirements.requested_capabilities.reasoning,
            upstream_api,
        )?;
        prepared_candidates.push(RouteCandidate {
            route_id: route_id.clone(),
            upstream_target_id: route.upstream_target().to_owned(),
            upstream_api_id: route.upstream_api().to_owned(),
            request,
            bridge,
            reasoning_level_mapping,
        });
    }
    // 没有候选时返回最具体的规划错误，否则构造带 fallback 边界的计划。
    if prepared_candidates.is_empty() {
        return Err(if protocol_mismatch_seen {
            RequestPlanningError::UnsupportedProtocol
        } else {
            RequestPlanningError::NoRoute
        });
    }

    Ok(RoutePlan {
        candidates: prepared_candidates,
        is_streaming: requirements.is_streaming,
        allows_fallback: !requirements.requested_capabilities.previous_response_id,
    })
}

/// 将候选的显式 level 映射写入独立请求副本，并保留其他 Native 字段。
fn apply_reasoning_level_mapping(
    request: ApiRequest,
    requested: RequestedReasoning,
    upstream_api: &UpstreamApi,
) -> Result<(ApiRequest, Option<ReasoningLevelMapping>), RequestPlanningError> {
    // 只有已识别 level 且候选显式声明映射时才改写请求。
    let RequestedReasoning::Level(level) = requested else {
        return Ok((request, None));
    };
    let Some(upstream) = upstream_api.reasoning_level_mapping(level) else {
        return Ok((request, None));
    };
    let mapping = ReasoningLevelMapping {
        downstream: level,
        upstream: upstream.to_owned(),
    };

    // 按请求分析使用的相同字段优先级改写唯一生效位置。
    let mut document: Value =
        serde_json::from_slice(request.body()).map_err(|_| RequestPlanningError::InvalidJson)?;
    let object = document
        .as_object_mut()
        .ok_or(RequestPlanningError::InvalidJson)?;
    if object
        .get("reasoning_effort")
        .is_some_and(|value| !value.is_null())
    {
        object.insert(
            "reasoning_effort".to_owned(),
            Value::String(mapping.upstream.clone()),
        );
    } else if let Some(reasoning) = object.get_mut("reasoning").and_then(Value::as_object_mut) {
        reasoning.insert("effort".to_owned(), Value::String(mapping.upstream.clone()));
    }

    // 重新序列化候选请求，映射事实留在 RouteCandidate 供 tracing 观察。
    let body = serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RequestPlanningError::InvalidJson)?;
    Ok((ApiRequest::new(request.protocol(), body), Some(mapping)))
}

/// 按 Public Model 的固定接口契约返回最具体的 fail-closed 规划错误。
fn public_model_preflight_error(
    requested_features: RequestedCapabilities,
    requested_output_tokens: Option<u64>,
    interface: &ModelInterfaceCapabilities,
) -> Option<RequestPlanningError> {
    // 先校验共享生成能力，未知状态与明确不支持都不能进入 egress。
    if requested_features.unmodeled_tools {
        return Some(RequestPlanningError::UnsupportedCapabilities);
    }
    if requested_features.generation.streaming && !interface.supports_streaming() {
        return Some(RequestPlanningError::StreamingUnsupported);
    }
    if (requested_features.generation.function_calling && !interface.supports_function_calling())
        || (requested_features.generation.parallel_tool_calls
            && !interface.supports_parallel_tool_calls())
        || (requested_features.generation.image_input && !interface.supports_image_input())
        || (requested_features.generation.structured_outputs
            && !interface.supports_structured_outputs())
        || (requested_features.generation.store && !interface.supports_store())
    {
        return Some(RequestPlanningError::UnsupportedCapabilities);
    }
    if (requested_features.previous_response_id && !interface.supports_previous_response_id())
        || (requested_features.background && !interface.supports_background())
    {
        return Some(RequestPlanningError::UnsupportedCapabilities);
    }
    if interface.max_output_tokens().is_some_and(|limit| {
        requested_output_tokens.is_some_and(|requested| requested > u64::from(limit))
    }) {
        return Some(RequestPlanningError::OutputLimitExceeded);
    }
    // 最后校验 reasoning 的支持状态和固定公共 level 集合。
    match requested_features.reasoning {
        RequestedReasoning::None | RequestedReasoning::Level(ReasoningLevel::None) => {}
        RequestedReasoning::Unspecified
            if interface.reasoning_support() != SupportState::Supported =>
        {
            return Some(RequestPlanningError::ReasoningUnsupported);
        }
        RequestedReasoning::Level(level)
            if interface.reasoning_support() != SupportState::Supported
                || !interface.reasoning_levels().contains(&level) =>
        {
            return Some(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::UnknownLevel => {
            return Some(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::Conflicting => {
            return Some(RequestPlanningError::InvalidReasoningConfiguration);
        }
        RequestedReasoning::Unspecified | RequestedReasoning::Level(_) => {}
    }
    None
}
