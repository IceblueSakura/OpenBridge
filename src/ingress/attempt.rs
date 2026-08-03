//! Bounded upstream attempts, candidate retention, and backoff for one downstream request.
//!
//! This module manages request-level counts and time boundaries only; it does not select Routes,
//! Providers, or error categories. Callers must provide a RoutePlan and adapter classification.
//! Fixed hard limits ensure that no request can create an infinite upstream loop.

use std::time::Duration;

const MAX_REQUEST_ATTEMPTS: usize = 6;
const MAX_CANDIDATE_ATTEMPTS: usize = 2;
const INITIAL_BACKOFF: Duration = Duration::from_millis(50);
const MAX_BACKOFF: Duration = Duration::from_millis(500);

/// Next action allowed after a retryable failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptStep {
    /// Retry the current candidate after backoff.
    RetryCandidate,
    /// Move to the next planned candidate after backoff.
    NextCandidate,
    /// The total budget or candidates are exhausted; return the current failure.
    Finish,
}

/// Shared attempt budget and capped exponential-backoff state for one downstream request.
pub(super) struct AttemptManager {
    attempts_started: usize,
    candidate_attempts: usize,
    next_backoff: Duration,
}

impl AttemptManager {
    /// Creates a request-level manager before any upstream call starts.
    pub(super) fn new() -> Self {
        Self {
            attempts_started: 0,
            candidate_attempts: 0,
            next_backoff: INITIAL_BACKOFF,
        }
    }

    /// Starts a new candidate and clears its local attempt count.
    pub(super) fn begin_candidate(&mut self) {
        self.candidate_attempts = 0;
    }

    /// Consumes one request-level and candidate-level attempt budget.
    pub(super) fn start_attempt(&mut self) -> bool {
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
    pub(super) fn attempts_started(&self) -> usize {
        self.attempts_started
    }

    /// Selects retry, fallback, or completion based on remaining untried candidates.
    ///
    /// The current candidate may retry only when the budget can still accommodate remaining
    /// candidates. The request-level hard limit always takes priority, regardless of Route count,
    /// so configuration size cannot multiply upstream calls per request.
    pub(super) fn next_step(&self, untried_candidates: usize) -> AttemptStep {
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

    /// Waits for the next attempt and advances capped exponential backoff.
    ///
    /// When the downstream task is cancelled, the `sleep` future and manager are dropped together;
    /// no background wake-up can continue the request.
    pub(super) async fn wait_before_next_attempt(&mut self) {
        // Fix this delay and compute the next capped exponential-backoff value.
        let delay = self.take_backoff_delay();

        // Wait for a cancellable Tokio timer before allowing the next upstream call.
        tokio::time::sleep(delay).await;
    }

    /// Returns the current attempt backoff and advances to the next capped value.
    fn take_backoff_delay(&mut self) -> Duration {
        let delay = self.next_backoff;
        self.next_backoff = self.next_backoff.saturating_mul(2).min(MAX_BACKOFF);
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::{AttemptManager, AttemptStep, INITIAL_BACKOFF, MAX_BACKOFF, MAX_REQUEST_ATTEMPTS};

    #[test]
    fn request_budget_reserves_untried_candidates_and_has_a_hard_limit() {
        let mut attempts = AttemptManager::new();
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
        let mut attempts = AttemptManager::new();

        // Verify that the delay doubles and remains capped at 500 ms.
        assert_eq!(attempts.take_backoff_delay(), INITIAL_BACKOFF);
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(2)
        );
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(4)
        );
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(8)
        );
        assert_eq!(attempts.take_backoff_delay(), MAX_BACKOFF);
        assert_eq!(attempts.take_backoff_delay(), MAX_BACKOFF);
    }
}
