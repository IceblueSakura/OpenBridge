//! 已规划 Route candidate 的有限 retry/fallback 与响应接管。

use axum::response::Response;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::{
    core::ApiProtocol,
    observability::RequestObservation,
    pipeline::{analyze_request, plan_request},
    provider::ProviderAdapter,
    transport::upstream::TransportError,
};

use super::{
    attempt::{AttemptManager, AttemptStep},
    response::{api_error, route_error, upstream_error},
    state::GatewayState,
};

use self::response::{UpstreamResponseContext, upstream_response};

mod response;

/// 将一个已经过 HTTP 输入检查的请求送往有序 Native/Bridged candidate。
///
/// 每次调用共享启动时构建的不可变 registry。仅 streaming 请求可在**尚未返回任何下游
/// body**时重试：一旦 `UpstreamResponse`
/// 被交给客户端，后续 SSE bytes 只能原样继续或以 body error 终止，绝不能拼接第二个
/// 上游尝试。`previous_response_id` 等 target-bound state 会令 pipeline 关闭跨 candidate
/// fallback，但仍可在同一 candidate 上执行有限 pre-output retry。
pub(super) async fn forward_request(
    state: GatewayState,
    observation: RequestObservation,
    protocol: ApiProtocol,
    downstream_headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 分析请求事实并生成带 capability/fallback 边界的 route plan。
    let registry = state.registry.clone();
    let profile = match analyze_request(protocol, &body) {
        Ok(profile) => profile,
        Err(error) => return route_error(error),
    };
    observation.record_request(protocol, profile.public_model());
    let plan = match plan_request(&registry, &profile, body) {
        Ok(plan) => plan,
        Err(error) => return route_error(error),
    };
    let candidate_count = if plan.allows_fallback() {
        plan.candidates().len()
    } else {
        1
    };
    let mut attempts = AttemptManager::new();
    let observe_cross_request_health = plan.allows_fallback();
    let mut cooldown_skipped = false;

    // 按优先级准备每个 candidate 的 target、credential、adapter 和原生请求。
    'candidates: for (candidate_index, candidate) in
        plan.candidates().iter().take(candidate_count).enumerate()
    {
        attempts.begin_candidate();
        let Some(target) = registry.upstream_target(candidate.upstream_target_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured upstream target is unavailable",
            );
        };
        // 新无状态请求跳过仍在 cooldown 的 scope；target-bound continuation 始终尝试原目标。
        if observe_cross_request_health
            && !state.health.is_available(
                candidate.upstream_target_id(),
                target,
                std::time::Instant::now(),
            )
        {
            cooldown_skipped = true;
            observation.record_cooldown_skip(candidate.upstream_target_id());
            continue;
        }
        let Some(upstream_api) = target.upstream_api(candidate.upstream_api_id()) else {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "Configured native upstream API is unavailable",
            );
        };
        let credential = match state
            .credentials
            .upstream(target.kind(), target.credential().id())
        {
            Ok(credential) => credential,
            Err(_) => {
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_authentication_error",
                    "Upstream credentials are unavailable",
                );
            }
        };
        let adapter = ProviderAdapter::for_kind(target.kind());
        let headers = match adapter.build_outbound_headers(&credential, &downstream_headers) {
            Ok(headers) => headers,
            Err(_) => {
                return api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "configuration_error",
                    "Provider authentication could not be prepared",
                );
            }
        };
        let request =
            match adapter.prepare_request(candidate.request(), upstream_api.upstream_model()) {
                Ok(request) => request,
                Err(_) => {
                    return api_error(
                        StatusCode::BAD_REQUEST,
                        "unsupported_request",
                        "Request is not supported by the selected provider",
                    );
                }
            };

        // 在尚未向下游提交 response 时执行请求级受限 attempt，并保持 body 的单一来源。
        loop {
            if !attempts.start_attempt() {
                return api_error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_attempts_exhausted",
                    "The upstream attempt budget was exhausted",
                );
            }
            observation.record_attempt(
                attempts.attempts_started() as u64,
                candidate.route_id(),
                candidate.upstream_target_id(),
                target.kind(),
                candidate.bridge().is_some(),
            );
            if let Some(mapping) = candidate.reasoning_level_mapping() {
                tracing::info!(
                    downstream_reasoning_level = mapping.downstream.as_wire(),
                    upstream_reasoning_level = mapping.upstream,
                    "reasoning_level_mapped"
                );
            }
            match state
                .upstream
                .send(target, request.clone(), headers.clone())
                .await
            {
                Ok(upstream) if should_retry_status(&adapter, upstream.status()) => {
                    // 在选择 retry/fallback 前记录跨请求 cooldown，但不改变本请求局部 retry 预算。
                    observation.record_attempt_http_result(
                        attempts.attempts_started() as u64,
                        upstream.status(),
                    );
                    let classification = adapter.classify_status(upstream.status());
                    state.health.record_http_failure(
                        candidate.upstream_target_id(),
                        target,
                        classification.kind(),
                        upstream.headers(),
                        std::time::Instant::now(),
                    );
                    let untried_candidates = candidate_count - candidate_index - 1;
                    match attempts.next_step(untried_candidates) {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => {
                            return upstream_response(
                                upstream,
                                UpstreamResponseContext {
                                    validate_sse: plan.is_streaming(),
                                    protocol,
                                    adapter,
                                    max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                                    max_json_body_bytes: registry.limits().max_request_body_bytes(),
                                    bridge: candidate.bridge().cloned(),
                                    observation: observation.clone(),
                                },
                            )
                            .await;
                        }
                    }
                }
                Ok(upstream) => {
                    // 只有成功 HTTP response 才清除该 target 的已知 cooldown。
                    observation.record_attempt_http_result(
                        attempts.attempts_started() as u64,
                        upstream.status(),
                    );
                    if upstream.status().is_success() {
                        state
                            .health
                            .record_success(candidate.upstream_target_id(), target);
                    }
                    return upstream_response(
                        upstream,
                        UpstreamResponseContext {
                            validate_sse: plan.is_streaming(),
                            protocol,
                            adapter,
                            max_sse_event_bytes: registry.limits().max_sse_event_bytes(),
                            max_json_body_bytes: registry.limits().max_request_body_bytes(),
                            bridge: candidate.bridge().cloned(),
                            observation: observation.clone(),
                        },
                    )
                    .await;
                }
                Err(error) if should_retry_error(&error) => {
                    // timeout/transport failure 只隔离 fault domain，不污染 quota scope。
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
                    );
                    state.health.record_transport_failure(
                        candidate.upstream_target_id(),
                        target,
                        std::time::Instant::now(),
                    );
                    let untried_candidates = candidate_count - candidate_index - 1;
                    match attempts.next_step(untried_candidates) {
                        AttemptStep::RetryCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_retry();
                            continue;
                        }
                        AttemptStep::NextCandidate => {
                            attempts.wait_before_next_attempt().await;
                            observation.record_fallback();
                            continue 'candidates;
                        }
                        AttemptStep::Finish => return upstream_error(error),
                    }
                }
                Err(error) => {
                    observation.record_attempt_transport_failure(
                        attempts.attempts_started() as u64,
                        transport_error_kind(&error),
                    );
                    return upstream_error(error);
                }
            }
        }
    }

    if cooldown_skipped {
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "upstream_cooldown",
            "All compatible upstream targets are temporarily unavailable",
        )
    } else {
        api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_error",
            "The upstream request failed",
        )
    }
}

fn should_retry_status(adapter: &ProviderAdapter, status: StatusCode) -> bool {
    adapter.classify_status(status).retry_hint() == crate::provider::RetryHint::BeforeFirstEvent
}

fn should_retry_error(error: &TransportError) -> bool {
    matches!(error, TransportError::Timeout | TransportError::Request(_))
}

fn transport_error_kind(error: &TransportError) -> &'static str {
    match error {
        TransportError::ClientBuild(_) => "client_build",
        TransportError::Request(_) => "request",
        TransportError::Timeout => "timeout",
        TransportError::InvalidTarget => "invalid_target",
    }
}
