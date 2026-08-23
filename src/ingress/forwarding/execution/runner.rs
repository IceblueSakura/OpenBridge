//! Single prepared-candidate send/retry loop shared by Generation and Embeddings.

use http::{HeaderMap, StatusCode};

use crate::{
    execution::{AttemptCoordinator, AttemptStep},
    observability::{ErrorType, FailureStage, NextAction, RequestObservation, TimeoutPhase},
    transport::upstream::TransportError,
};

use super::super::response::UpstreamResponseOutcome;
use super::GenerationCandidateOutcome;
use super::driver::{
    OperationDriver, attempt_exhausted, finish_http, finish_transport, http_failure,
    record_attempt, record_reasoning_mapping, recover_oauth, retryable_http_step,
    retryable_transport_step, select_attempt, should_retry_http, should_retry_transport,
    stored_http_failure, transport_failure,
};
use crate::ingress::state::GatewayState;

pub(super) async fn run(
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
    attempts: &mut AttemptCoordinator,
    mut driver: OperationDriver<'_>,
) -> GenerationCandidateOutcome {
    loop {
        let selected = match select_attempt(&mut driver, state, observation, downstream_headers) {
            Ok(selected) => selected,
            Err(outcome) => return *outcome,
        };
        if !attempts.start_attempt() {
            return attempt_exhausted(&driver, observation);
        }
        record_attempt(&driver, observation, attempts);
        record_reasoning_mapping(&driver);

        let send_result = state
            .upstream
            .send(
                driver.target(),
                driver.request().clone(),
                selected.headers.clone(),
            )
            .await;
        match send_result {
            Ok(upstream)
                if driver.uses_oauth2() && upstream.status() == StatusCode::UNAUTHORIZED =>
            {
                match recover_oauth(&mut driver, state).await {
                    Ok(()) => {
                        observation.record_attempt_http_result(
                            attempts.attempts_started() as u64,
                            upstream.status(),
                            Some(http_failure(
                                &driver,
                                upstream.status(),
                                NextAction::RetryCandidate,
                            )),
                        );
                        observation.record_retry(
                            ErrorType::UpstreamAuthentication,
                            std::time::Duration::ZERO,
                        );
                        continue;
                    }
                    Err(response) => {
                        observation.record_attempt_http_result(
                            attempts.attempts_started() as u64,
                            upstream.status(),
                            Some(http_failure(&driver, upstream.status(), NextAction::Finish)),
                        );
                        observation.record_request_failure(
                            ErrorType::UpstreamAuthentication,
                            FailureStage::Credential,
                            false,
                        );
                        return GenerationCandidateOutcome::Response(response);
                    }
                }
            }
            Ok(upstream) if should_retry_http(&driver, upstream.status()) => {
                let step = retryable_http_step(&mut driver, state, &upstream, &selected, attempts);
                let attempt_failure = http_failure(&driver, upstream.status(), step.next_action());
                observation.record_attempt_http_result(
                    attempts.attempts_started() as u64,
                    upstream.status(),
                    Some(attempt_failure),
                );
                match step {
                    AttemptStep::RetryCandidate => {
                        let backoff = attempts.schedule_backoff();
                        observation.record_retry(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                    }
                    AttemptStep::NextCandidate => {
                        let failure = stored_http_failure(&driver, upstream);
                        let backoff = attempts.schedule_backoff();
                        observation.record_fallback(attempt_failure.error_type, backoff);
                        AttemptCoordinator::wait_before_next_attempt(backoff).await;
                        return GenerationCandidateOutcome::NextCandidate {
                            failure: Some(Box::new(failure)),
                            cooldown_skipped: false,
                        };
                    }
                    AttemptStep::Finish => {
                        return match finish_http(
                            &mut driver,
                            state,
                            observation,
                            upstream,
                            &selected,
                        )
                        .await
                        {
                            UpstreamResponseOutcome::Response(response) => {
                                GenerationCandidateOutcome::Response(response)
                            }
                            UpstreamResponseOutcome::PrecommitFailure(_) => unreachable!(
                                "non-success HTTP responses do not enter SSE precommit"
                            ),
                        };
                    }
                }
            }
            Ok(upstream) => {
                let status = upstream.status();
                match finish_http(&mut driver, state, observation, upstream, &selected).await {
                    UpstreamResponseOutcome::Response(response) => {
                        let failure = (!status.is_success())
                            .then(|| http_failure(&driver, status, NextAction::Finish));
                        observation.record_attempt_http_result(
                            attempts.attempts_started() as u64,
                            status,
                            failure,
                        );
                        return GenerationCandidateOutcome::Response(response);
                    }
                    UpstreamResponseOutcome::PrecommitFailure(error) => {
                        if let Some(outcome) = handle_retryable_transport(
                            &driver,
                            state,
                            observation,
                            attempts,
                            error,
                            TimeoutPhase::FirstEvent,
                        )
                        .await
                        {
                            return outcome;
                        }
                    }
                }
            }
            Err(error) if should_retry_transport(&error) => {
                if let Some(outcome) = handle_retryable_transport(
                    &driver,
                    state,
                    observation,
                    attempts,
                    error,
                    TimeoutPhase::ResponseHeaders,
                )
                .await
                {
                    return outcome;
                }
            }
            Err(error) => {
                if matches!(&error, TransportError::Timeout) {
                    observation.record_timeout_context(TimeoutPhase::ResponseHeaders);
                }
                observation.record_attempt_transport_failure(
                    attempts.attempts_started() as u64,
                    transport_failure(&error, NextAction::Finish),
                );
                return GenerationCandidateOutcome::Response(finish_transport(&driver, error));
            }
        }
    }
}

/// Applies the existing finite retry/fallback policy to one precommit transport-class failure.
async fn handle_retryable_transport(
    driver: &OperationDriver<'_>,
    state: &GatewayState,
    observation: &RequestObservation,
    attempts: &mut AttemptCoordinator,
    error: TransportError,
    timeout_phase: TimeoutPhase,
) -> Option<GenerationCandidateOutcome> {
    if matches!(&error, TransportError::Timeout) {
        observation.record_timeout_context(timeout_phase);
    }
    let step = retryable_transport_step(driver, state, attempts);
    let attempt_failure = transport_failure(&error, step.next_action());
    observation
        .record_attempt_transport_failure(attempts.attempts_started() as u64, attempt_failure);
    match step {
        AttemptStep::RetryCandidate => {
            let backoff = attempts.schedule_backoff();
            observation.record_retry(attempt_failure.error_type, backoff);
            AttemptCoordinator::wait_before_next_attempt(backoff).await;
            None
        }
        AttemptStep::NextCandidate => {
            let backoff = attempts.schedule_backoff();
            observation.record_fallback(attempt_failure.error_type, backoff);
            AttemptCoordinator::wait_before_next_attempt(backoff).await;
            Some(GenerationCandidateOutcome::NextCandidate {
                failure: None,
                cooldown_skipped: false,
            })
        }
        AttemptStep::Finish => Some(GenerationCandidateOutcome::Response(finish_transport(
            driver, error,
        ))),
    }
}
