//! Closed operation driver state and policy for prepared-candidate execution.

use std::collections::HashSet;

use axum::response::Response;
use http::{HeaderMap, StatusCode};

use crate::{
    credential::UpstreamCredential,
    execution::{AttemptCoordinator, AttemptStep},
    oauth2_credentials::OAuth2CredentialLease,
    observability::{
        AttemptFailure, ErrorType, FailureStage, NextAction, ProviderAttemptContext,
        RequestObservation,
    },
    pipeline::{EmbeddingRequestRequirements, EmbeddingRoutePlan, RouteCandidate, RoutePlan},
    provider::{
        EmbeddingsProviderAdapter, GenerationProviderAdapter, PreparedUpstreamRequest,
        ProviderAdapter, UpstreamErrorKind,
    },
    registry::{CredentialPoolBinding, UpstreamApi, UpstreamTarget},
    transport::upstream::{TransportError, UpstreamResponse},
};

use super::super::{
    candidate::PreparedCandidate,
    configuration_error as generation_configuration_error,
    embedding_response::validated_embedding_response,
    embeddings::configuration_error as embedding_configuration_error,
    oauth::{oauth2_authentication_error, recover_after_unauthorized},
    policy::{
        http_attempt_failure, should_retry_error, should_retry_status, transport_attempt_failure,
    },
    response::{UpstreamResponseContext, UpstreamResponseOutcome, upstream_response},
};
use super::{
    GenerationCandidateOutcome, PreparedEmbeddingExecution, PreparedGenerationExecution,
    StoredHttpFailure,
};
use crate::ingress::{
    response::{
        api_error, embedding_server_error, embedding_upstream_error,
        normalized_embedding_upstream_error, upstream_error,
    },
    state::GatewayState,
};

/// Credential and headers selected for one physical attempt.
pub(super) struct SelectedAttempt {
    pub(super) member_index: usize,
    pub(super) member_id: String,
    pub(super) headers: HeaderMap,
}

/// Closed operation-specific state consumed by the shared attempt runner.
pub(super) enum OperationDriver<'a> {
    Generation {
        plan: &'a RoutePlan,
        candidate: &'a RouteCandidate,
        target: &'a UpstreamTarget,
        upstream_api: &'a UpstreamApi,
        credential_pool: &'a CredentialPoolBinding,
        uses_oauth2: bool,
        oauth2_lease: Option<OAuth2CredentialLease>,
        static_credentials: Option<Vec<UpstreamCredential<'a>>>,
        adapter: GenerationProviderAdapter,
        request: PreparedUpstreamRequest,
        candidate_index: usize,
        candidate_count: usize,
        rejected_members: HashSet<String>,
        current_member: Option<usize>,
        oauth2_replayed: bool,
    },
    Embeddings {
        requirements: &'a EmbeddingRequestRequirements,
        plan: &'a EmbeddingRoutePlan,
        target: &'a UpstreamTarget,
        upstream_api: &'a UpstreamApi,
        credential_pool: &'a CredentialPoolBinding,
        credentials: Vec<UpstreamCredential<'a>>,
        adapter: EmbeddingsProviderAdapter,
        request: PreparedUpstreamRequest,
        replayable: bool,
        rejected_members: HashSet<String>,
        current_member: Option<usize>,
    },
}

impl<'a> OperationDriver<'a> {
    pub(super) fn generation(execution: PreparedGenerationExecution<'a>) -> Self {
        let PreparedGenerationExecution {
            plan,
            candidate,
            target,
            prepared,
            candidate_index,
            candidate_count,
        } = execution;
        let PreparedCandidate {
            upstream_api,
            credential_pool,
            uses_oauth2,
            oauth2_lease,
            static_credentials,
            adapter,
            request,
        } = prepared;
        Self::Generation {
            plan,
            candidate,
            target,
            upstream_api,
            credential_pool,
            uses_oauth2,
            oauth2_lease,
            static_credentials,
            adapter,
            request,
            candidate_index,
            candidate_count,
            rejected_members: HashSet::new(),
            current_member: None,
            oauth2_replayed: false,
        }
    }

    pub(super) fn embeddings(execution: PreparedEmbeddingExecution<'a>) -> Self {
        let PreparedEmbeddingExecution {
            requirements,
            plan,
            target,
            upstream_api,
            credential_pool,
            credentials,
            adapter,
            request,
            replayable,
        } = execution;
        Self::Embeddings {
            requirements,
            plan,
            target,
            upstream_api,
            credential_pool,
            credentials,
            adapter,
            request,
            replayable,
            rejected_members: HashSet::new(),
            current_member: None,
        }
    }

    pub(super) fn adapter(&self) -> ProviderAdapter {
        match self {
            Self::Generation { adapter, .. } => adapter.provider(),
            Self::Embeddings { adapter, .. } => adapter.provider(),
        }
    }

    pub(super) fn target(&self) -> &UpstreamTarget {
        match self {
            Self::Generation { target, .. } | Self::Embeddings { target, .. } => target,
        }
    }

    pub(super) fn request(&self) -> &PreparedUpstreamRequest {
        match self {
            Self::Generation { request, .. } | Self::Embeddings { request, .. } => request,
        }
    }

    pub(super) fn uses_oauth2(&self) -> bool {
        matches!(
            self,
            Self::Generation {
                uses_oauth2: true,
                ..
            }
        )
    }
}

pub(super) fn should_retry_http(driver: &OperationDriver<'_>, status: StatusCode) -> bool {
    should_retry_status(&driver.adapter(), status)
}

pub(super) fn should_retry_transport(error: &TransportError) -> bool {
    should_retry_error(error)
}

pub(super) fn http_failure(
    driver: &OperationDriver<'_>,
    status: StatusCode,
    next_action: NextAction,
) -> AttemptFailure {
    http_attempt_failure(&driver.adapter(), status, next_action)
}

pub(super) fn transport_failure(error: &TransportError, next_action: NextAction) -> AttemptFailure {
    transport_attempt_failure(error, next_action)
}

pub(super) fn select_attempt(
    driver: &mut OperationDriver<'_>,
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
) -> Result<SelectedAttempt, Box<GenerationCandidateOutcome>> {
    match driver {
        OperationDriver::Generation {
            plan,
            candidate,
            credential_pool,
            uses_oauth2,
            oauth2_lease,
            static_credentials,
            adapter,
            rejected_members,
            current_member,
            ..
        } => {
            let member_index = if *uses_oauth2 {
                0
            } else {
                let credentials = static_credentials
                    .as_ref()
                    .expect("API-key target must retain static credentials");
                match *current_member {
                    Some(index) => index,
                    None => {
                        if !plan.allows_fallback() {
                            if credentials.len() != 1 {
                                return selection_error(GenerationCandidateOutcome::Response(
                                    generation_configuration_error(
                                        observation,
                                        "State-bound routes require exactly one credential member",
                                    ),
                                ));
                            }
                            0
                        } else {
                            match state.credential_health.select_member(
                                credential_pool.id(),
                                credentials,
                                rejected_members,
                                std::time::Instant::now(),
                            ) {
                                Some(index) => {
                                    if !rejected_members.is_empty() {
                                        observation.record_credential_rotation();
                                    }
                                    index
                                }
                                None => {
                                    observation
                                        .record_cooldown_skip(candidate.upstream_target_id());
                                    return selection_error(
                                        GenerationCandidateOutcome::NextCandidate {
                                            failure: None,
                                            cooldown_skipped: true,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            };
            *current_member = Some(member_index);

            let (member_id, headers) = if *uses_oauth2 {
                let Some(lease) = oauth2_lease.as_ref() else {
                    return selection_error(GenerationCandidateOutcome::Response(
                        oauth2_authentication_error(),
                    ));
                };
                let credential = match lease.credential() {
                    Ok(credential) => credential,
                    Err(_) => {
                        observation.record_request_failure(
                            ErrorType::UpstreamAuthentication,
                            FailureStage::Credential,
                            false,
                        );
                        return selection_error(GenerationCandidateOutcome::Response(
                            oauth2_authentication_error(),
                        ));
                    }
                };
                let headers = match adapter.build_outbound_headers(&credential, downstream_headers)
                {
                    Ok(headers) => headers,
                    Err(_) => {
                        return selection_error(GenerationCandidateOutcome::Response(
                            generation_configuration_error(
                                observation,
                                "Provider authentication could not be prepared",
                            ),
                        ));
                    }
                };
                (credential.member_id().to_owned(), headers)
            } else {
                let credential = static_credentials
                    .as_ref()
                    .expect("API-key target must retain static credentials")[member_index];
                let headers = match adapter.build_outbound_headers(&credential, downstream_headers)
                {
                    Ok(headers) => headers,
                    Err(_) => {
                        return selection_error(GenerationCandidateOutcome::Response(
                            generation_configuration_error(
                                observation,
                                "Provider authentication could not be prepared",
                            ),
                        ));
                    }
                };
                (credential.member_id().to_owned(), headers)
            };
            Ok(SelectedAttempt {
                member_index,
                member_id,
                headers,
            })
        }
        OperationDriver::Embeddings {
            credential_pool,
            credentials,
            adapter,
            rejected_members,
            current_member,
            ..
        } => {
            let member_index = match *current_member {
                Some(index) => index,
                None => match state.credential_health.select_member(
                    credential_pool.id(),
                    credentials,
                    rejected_members,
                    std::time::Instant::now(),
                ) {
                    Some(index) => {
                        if !rejected_members.is_empty() {
                            observation.record_credential_rotation();
                        }
                        index
                    }
                    None => {
                        observation.record_request_failure(
                            ErrorType::UpstreamUnavailable,
                            FailureStage::Credential,
                            true,
                        );
                        return selection_error(GenerationCandidateOutcome::Response(
                            embedding_server_error(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "upstream_cooldown",
                                "The configured upstream target is temporarily unavailable",
                            ),
                        ));
                    }
                },
            };
            *current_member = Some(member_index);
            let credential = &credentials[member_index];
            let headers = match adapter.build_outbound_headers(credential, downstream_headers) {
                Ok(headers) => headers,
                Err(_) => {
                    return selection_error(GenerationCandidateOutcome::Response(
                        embedding_configuration_error(
                            observation,
                            "Provider authentication could not be prepared",
                        ),
                    ));
                }
            };
            Ok(SelectedAttempt {
                member_index,
                member_id: credential.member_id().to_owned(),
                headers,
            })
        }
    }
}

fn selection_error(
    outcome: GenerationCandidateOutcome,
) -> Result<SelectedAttempt, Box<GenerationCandidateOutcome>> {
    Err(Box::new(outcome))
}

pub(super) fn record_attempt(
    driver: &OperationDriver<'_>,
    observation: &RequestObservation,
    attempts: &AttemptCoordinator,
) {
    match driver {
        OperationDriver::Generation {
            candidate,
            target,
            upstream_api,
            ..
        } => observation.record_attempt(ProviderAttemptContext {
            attempt: attempts.attempts_started() as u64,
            provider: target.kind(),
            upstream_operation: candidate.upstream_operation(),
            upstream_model: upstream_api.upstream_model(),
            upstream_target: candidate.upstream_target_id(),
            bridged: candidate.bridge().is_some(),
        }),
        OperationDriver::Embeddings {
            plan,
            target,
            upstream_api,
            ..
        } => {
            let candidate = plan.candidate();
            observation.record_attempt(ProviderAttemptContext {
                attempt: attempts.attempts_started() as u64,
                provider: target.kind(),
                upstream_operation: candidate.upstream_operation(),
                upstream_model: upstream_api.upstream_model(),
                upstream_target: candidate.upstream_target_id(),
                bridged: false,
            });
        }
    }
}

pub(super) fn record_reasoning_mapping(driver: &OperationDriver<'_>) {
    if let OperationDriver::Generation { request, .. } = driver
        && let Some(mapping) = request.reasoning_level_mapping()
    {
        tracing::info!(
            downstream_reasoning_level = mapping.downstream.as_wire(),
            upstream_reasoning_level = mapping.upstream,
            "reasoning_level_mapped"
        );
    }
}

pub(super) fn attempt_exhausted(
    driver: &OperationDriver<'_>,
    observation: &RequestObservation,
) -> GenerationCandidateOutcome {
    observation.record_request_failure(ErrorType::UpstreamFailure, FailureStage::Upstream, false);
    match driver {
        OperationDriver::Generation { .. } => GenerationCandidateOutcome::Response(api_error(
            StatusCode::BAD_GATEWAY,
            "upstream_attempts_exhausted",
            "The upstream attempt budget was exhausted",
        )),
        OperationDriver::Embeddings { .. } => {
            GenerationCandidateOutcome::Response(embedding_server_error(
                StatusCode::BAD_GATEWAY,
                "upstream_attempts_exhausted",
                "The upstream attempt budget was exhausted",
            ))
        }
    }
}

pub(super) async fn recover_oauth(
    driver: &mut OperationDriver<'_>,
    state: &GatewayState,
) -> Result<(), Response> {
    let OperationDriver::Generation {
        target,
        credential_pool,
        oauth2_lease,
        oauth2_replayed,
        ..
    } = driver
    else {
        unreachable!("only Generation candidates can use OAuth2")
    };
    let Some(current_lease) = oauth2_lease.as_ref() else {
        return Err(oauth2_authentication_error());
    };
    let next_lease = recover_after_unauthorized(
        state,
        target.kind(),
        credential_pool.id(),
        current_lease,
        oauth2_replayed,
    )
    .await?;
    *oauth2_lease = Some(next_lease);
    Ok(())
}

pub(super) fn retryable_http_step(
    driver: &mut OperationDriver<'_>,
    state: &GatewayState,
    upstream: &UpstreamResponse,
    selected: &SelectedAttempt,
    attempts: &AttemptCoordinator,
) -> AttemptStep {
    match driver {
        OperationDriver::Generation {
            plan,
            candidate,
            target,
            credential_pool,
            uses_oauth2,
            static_credentials,
            adapter,
            candidate_index,
            candidate_count,
            rejected_members,
            current_member,
            ..
        } => {
            let classification = adapter.classify_status(upstream.status());
            let rate_limited = classification.kind() == UpstreamErrorKind::RateLimited;
            if rate_limited && !*uses_oauth2 {
                let credentials = static_credentials
                    .as_ref()
                    .expect("API-key target must retain static credentials");
                state.credential_health.record_rate_limited(
                    credential_pool.id(),
                    &credentials[selected.member_index],
                    upstream.headers(),
                    std::time::Instant::now(),
                );
                rejected_members.insert(selected.member_id.clone());
                *current_member = None;
            } else {
                state.health.record_http_failure(
                    candidate.upstream_target_id(),
                    target,
                    classification.kind(),
                    upstream.headers(),
                    std::time::Instant::now(),
                );
            }

            let untried_candidates = *candidate_count - *candidate_index - 1;
            let mut step = attempts.next_step(untried_candidates);
            if rate_limited
                && (*uses_oauth2
                    || !plan.allows_fallback()
                    || !static_credentials.as_ref().is_some_and(|credentials| {
                        state.credential_health.has_available_member(
                            credential_pool.id(),
                            credentials,
                            rejected_members,
                            std::time::Instant::now(),
                        )
                    }))
            {
                step = match step {
                    AttemptStep::RetryCandidate if untried_candidates > 0 => {
                        AttemptStep::NextCandidate
                    }
                    AttemptStep::RetryCandidate => AttemptStep::Finish,
                    other => other,
                };
            }
            step
        }
        OperationDriver::Embeddings {
            credential_pool,
            credentials,
            adapter,
            replayable,
            rejected_members,
            current_member,
            ..
        } => {
            let classification = adapter.classify_status(upstream.status());
            let rate_limited = classification.kind() == UpstreamErrorKind::RateLimited;
            if rate_limited {
                state.credential_health.record_rate_limited(
                    credential_pool.id(),
                    &credentials[selected.member_index],
                    upstream.headers(),
                    std::time::Instant::now(),
                );
                rejected_members.insert(selected.member_id.clone());
                *current_member = None;
            }
            let has_retry_credential = !rate_limited
                || state.credential_health.has_available_member(
                    credential_pool.id(),
                    credentials,
                    rejected_members,
                    std::time::Instant::now(),
                );
            if *replayable
                && has_retry_credential
                && attempts.next_step(0) == AttemptStep::RetryCandidate
            {
                AttemptStep::RetryCandidate
            } else {
                AttemptStep::Finish
            }
        }
    }
}

pub(super) fn retryable_transport_step(
    driver: &OperationDriver<'_>,
    state: &GatewayState,
    attempts: &AttemptCoordinator,
) -> AttemptStep {
    match driver {
        OperationDriver::Generation {
            candidate,
            target,
            candidate_index,
            candidate_count,
            ..
        } => {
            state.health.record_transport_failure(
                candidate.upstream_target_id(),
                target,
                std::time::Instant::now(),
            );
            attempts.next_step(*candidate_count - *candidate_index - 1)
        }
        OperationDriver::Embeddings { replayable, .. } => {
            if *replayable && attempts.next_step(0) == AttemptStep::RetryCandidate {
                AttemptStep::RetryCandidate
            } else {
                AttemptStep::Finish
            }
        }
    }
}

pub(super) async fn finish_http(
    driver: &mut OperationDriver<'_>,
    state: &GatewayState,
    observation: &RequestObservation,
    upstream: UpstreamResponse,
    selected: &SelectedAttempt,
) -> UpstreamResponseOutcome {
    match driver {
        OperationDriver::Generation {
            plan,
            candidate,
            target,
            credential_pool,
            static_credentials,
            adapter,
            ..
        } => {
            let outcome = upstream_response(
                upstream,
                UpstreamResponseContext {
                    validate_sse: plan.is_streaming(),
                    adapter: *adapter,
                    max_sse_event_bytes: plan.max_sse_event_bytes(),
                    max_json_body_bytes: plan.max_json_response_body_bytes(),
                    bridge: candidate.bridge().cloned(),
                    stream_response_conversion: candidate.stream_response_conversion(),
                    observation: observation.clone(),
                },
            )
            .await;
            if matches!(
                &outcome,
                UpstreamResponseOutcome::Response(response) if response.status().is_success()
            ) {
                if let Some(credentials) = static_credentials.as_ref() {
                    state
                        .credential_health
                        .record_success(credential_pool.id(), &credentials[selected.member_index]);
                }
                state
                    .health
                    .record_success(candidate.upstream_target_id(), target);
            }
            outcome
        }
        OperationDriver::Embeddings {
            requirements,
            plan,
            target,
            upstream_api,
            credential_pool,
            credentials,
            ..
        } => {
            if !upstream.status().is_success() {
                return normalized_embedding_upstream_error(upstream).into();
            }
            let response = match validated_embedding_response(
                upstream,
                observation,
                requirements.public_model(),
                upstream_api.upstream_model(),
                plan.input_count(),
                plan.encoding(),
                plan.dimensions(),
                plan.max_json_response_body_bytes(),
            )
            .await
            {
                Ok(response) => response,
                Err(_) => {
                    observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
                    return embedding_server_error(
                        StatusCode::BAD_GATEWAY,
                        "invalid_upstream_response",
                        "The upstream response is invalid",
                    )
                    .into();
                }
            };
            state
                .credential_health
                .record_success(credential_pool.id(), &credentials[selected.member_index]);
            state
                .health
                .record_success(plan.candidate().upstream_target_id(), target);
            response.into()
        }
    }
}

pub(super) fn finish_transport(driver: &OperationDriver<'_>, error: TransportError) -> Response {
    match driver {
        OperationDriver::Generation { .. } => upstream_error(error),
        OperationDriver::Embeddings { .. } => embedding_upstream_error(error),
    }
}

pub(super) fn stored_http_failure(
    driver: &OperationDriver<'_>,
    upstream: UpstreamResponse,
) -> StoredHttpFailure {
    let OperationDriver::Generation {
        candidate, adapter, ..
    } = driver
    else {
        unreachable!("Embeddings has no cross-candidate fallback")
    };
    StoredHttpFailure {
        upstream,
        adapter: *adapter,
        bridge: candidate.bridge().cloned(),
    }
}
