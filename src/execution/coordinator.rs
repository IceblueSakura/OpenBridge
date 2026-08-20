//! Bounded upstream attempts, candidate retention, and backoff for one downstream request.
//!
//! This module manages request-level counts and time boundaries only; it does not select Routes,
//! Providers, or error categories. Callers provide a fixed operation plan and adapter classification.
//! Fixed hard limits ensure that no request can create an infinite upstream loop.

use std::time::Duration;

use crate::observability::NextAction;

const MAX_REQUEST_ATTEMPTS: usize = 6;
const MAX_CANDIDATE_ATTEMPTS: usize = 2;
const INITIAL_BACKOFF: Duration = Duration::from_millis(50);
const MAX_BACKOFF: Duration = Duration::from_millis(500);

/// Next action allowed after a retryable failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttemptStep {
    /// Retry the current candidate after backoff.
    RetryCandidate,
    /// Move to the next planned candidate after backoff.
    NextCandidate,
    /// The total budget or candidates are exhausted; return the current failure.
    Finish,
}

impl AttemptStep {
    /// Converts the execution step into the stable observability action.
    pub(crate) const fn next_action(self) -> NextAction {
        match self {
            Self::RetryCandidate => NextAction::RetryCandidate,
            Self::NextCandidate => NextAction::NextCandidate,
            Self::Finish => NextAction::Finish,
        }
    }
}

/// Shared attempt budget and capped exponential-backoff state for one downstream request.
pub(crate) struct AttemptCoordinator {
    attempts_started: usize,
    candidate_attempts: usize,
    next_backoff: Duration,
}

impl AttemptCoordinator {
    /// Creates request-level coordination before any upstream call starts.
    pub(crate) fn new() -> Self {
        Self {
            attempts_started: 0,
            candidate_attempts: 0,
            next_backoff: INITIAL_BACKOFF,
        }
    }

    /// Starts a new candidate and clears its local attempt count.
    pub(crate) fn begin_candidate(&mut self) {
        self.candidate_attempts = 0;
    }

    /// Consumes one request-level and candidate-level attempt budget.
    pub(crate) fn start_attempt(&mut self) -> bool {
        // Reject a call that would exceed the request-level hard limit.
        if self.attempts_started >= MAX_REQUEST_ATTEMPTS {
            return false;
        }

        // Record the attempt at both request and current-candidate levels.
        self.attempts_started += 1;
        self.candidate_attempts += 1;
        true
    }

    /// Returns the number of attempts actually started for this request.
    pub(crate) fn attempts_started(&self) -> usize {
        self.attempts_started
    }

    /// Selects retry, fallback, or completion based on remaining untried candidates.
    ///
    /// The current candidate may retry only when the budget can still accommodate remaining
    /// candidates. The request-level hard limit always takes priority, regardless of Route count,
    /// so configuration size cannot multiply upstream calls per request.
    pub(crate) fn next_step(&self, untried_candidates: usize) -> AttemptStep {
        // Determine whether a retry preserves a chance for every remaining candidate within budget.
        let reserves_untried_candidates =
            self.attempts_started + untried_candidates < MAX_REQUEST_ATTEMPTS;

        // Prefer bounded retry on the current candidate, then fallback, and finally return the current failure.
        if self.candidate_attempts < MAX_CANDIDATE_ATTEMPTS
            && reserves_untried_candidates
            && self.attempts_started < MAX_REQUEST_ATTEMPTS
        {
            AttemptStep::RetryCandidate
        } else if untried_candidates > 0 && self.attempts_started < MAX_REQUEST_ATTEMPTS {
            AttemptStep::NextCandidate
        } else {
            AttemptStep::Finish
        }
    }

    /// Returns the scheduled backoff and advances the capped exponential policy.
    pub(crate) fn schedule_backoff(&mut self) -> Duration {
        let delay = self.next_backoff;
        self.next_backoff = self.next_backoff.saturating_mul(2).min(MAX_BACKOFF);
        delay
    }

    /// Waits for one previously scheduled delay without creating background work.
    pub(crate) async fn wait_before_next_attempt(delay: Duration) {
        tokio::time::sleep(delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttemptCoordinator, AttemptStep, INITIAL_BACKOFF, MAX_BACKOFF, MAX_REQUEST_ATTEMPTS,
    };

    #[test]
    fn request_budget_reserves_untried_candidates_and_has_a_hard_limit() {
        let mut attempts = AttemptCoordinator::new();
        attempts.begin_candidate();

        // Verify that a candidate-local retry preserves opportunities for remaining candidates.
        assert!(attempts.start_attempt());
        assert_eq!(attempts.next_step(3), AttemptStep::RetryCandidate);
        assert!(attempts.start_attempt());
        assert_eq!(attempts.next_step(3), AttemptStep::NextCandidate);

        // Consume the remaining request budget and verify that the hard limit rejects another attempt.
        for _ in 2..MAX_REQUEST_ATTEMPTS {
            attempts.begin_candidate();
            assert!(attempts.start_attempt());
        }
        assert!(!attempts.start_attempt());
        assert_eq!(attempts.next_step(1), AttemptStep::Finish);
    }

    #[test]
    fn backoff_doubles_and_stops_at_the_cap() {
        let mut attempts = AttemptCoordinator::new();

        // Verify that the delay doubles and remains capped at 500 ms.
        assert_eq!(attempts.schedule_backoff(), INITIAL_BACKOFF);
        assert_eq!(
            attempts.schedule_backoff(),
            INITIAL_BACKOFF.saturating_mul(2)
        );
        assert_eq!(
            attempts.schedule_backoff(),
            INITIAL_BACKOFF.saturating_mul(4)
        );
        assert_eq!(
            attempts.schedule_backoff(),
            INITIAL_BACKOFF.saturating_mul(8)
        );
        assert_eq!(attempts.schedule_backoff(), MAX_BACKOFF);
        assert_eq!(attempts.schedule_backoff(), MAX_BACKOFF);
    }
}
