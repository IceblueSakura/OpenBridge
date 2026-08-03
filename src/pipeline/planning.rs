//! 基于不可变 registry 生成有序 Native/Bridged Route 候选。

use bytes::Bytes;
use serde_json::Value;

use crate::{
    bridge::BridgePlan,
    core::{ApiProtocol, ApiRequest},
    registry::{
        ReasoningLevel, ReasoningLevelMapping, ReasoningSupport, RouteMode, RuntimeRegistry,
        UpstreamApi, UpstreamApiCapabilities,
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
            RouteMode::Bridged => match BridgePlan::prepare_with_reasoning_output(
                profile.protocol,
                upstream_api.protocol(),
                profile.public_model(),
                upstream_api.upstream_model(),
                body.clone(),
                upstream_api.reasoning_output(),
            ) {
                Ok((bridge, request)) => (request, Some(bridge)),
                Err(_) => {
                    first_candidate_error
                        .get_or_insert(RequestPlanningError::UnsupportedCapabilities);
                    continue;
                }
            },
        };
        let (request, reasoning_level_mapping) = apply_reasoning_level_mapping(
            request,
            profile.requested_capabilities.reasoning,
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

/// 返回一个候选不满足当前请求时最具体的 fail-closed 规划错误。
fn candidate_error(
    protocol: ApiProtocol,
    requested_features: RequestedCapabilities,
    capabilities: UpstreamApiCapabilities,
    configured_max_output_tokens: Option<u32>,
    reasoning: ReasoningSupport,
    reasoning_levels: &[ReasoningLevel],
    requested_output_tokens: Option<u64>,
) -> Option<RequestPlanningError> {
    // 先确认候选原生协议已启用，并拒绝未建模的工具语义。
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
    // 再检查 Responses 专有 state/background 约束与配置上限。
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
    // 最后校验 reasoning 的支持状态和请求的具体 level。
    match requested_features.reasoning {
        RequestedReasoning::None | RequestedReasoning::Level(ReasoningLevel::None) => {}
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
        RequestedReasoning::Conflicting => {
            return Some(RequestPlanningError::InvalidReasoningConfiguration);
        }
        RequestedReasoning::Unspecified | RequestedReasoning::Level(_) => {}
    }
    None
}
