//! Pure fixed-route advancement: transport and downstream visibility own observations.
use super::DeliveryState;
use crate::{
    provider::ErrorClass,
    topology::{FallbackPolicy, RoutePolicy},
};
pub fn may_advance(
    policy: &RoutePolicy,
    state: DeliveryState,
    error: ErrorClass,
    attempts: usize,
    budget_remaining: bool,
) -> bool {
    policy.fallback == FallbackPolicy::BeforeCommit
        && state == DeliveryState::Uncommitted
        && attempts < policy.max_attempts
        && budget_remaining
        && matches!(
            error,
            ErrorClass::RateLimit | ErrorClass::Server | ErrorClass::Timeout | ErrorClass::Upstream
        )
}
