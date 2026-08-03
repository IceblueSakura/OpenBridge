//! Cross-request upstream health isolation within one process.
//!
//! Health keys can come only from startup-validated `quota_scope`, `fault_domain`, or target IDs;
//! business requests cannot create or override scopes. This module stores bounded cooldown deadlines
//! only. It does not implement dynamic weights, persistence, distributed coordination, credential
//! rotation, or background probes; member state is managed by a separate module.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

use http::{HeaderMap, header::RETRY_AFTER};

use crate::{provider::UpstreamErrorKind, registry::UpstreamTarget};

pub(super) const DEFAULT_COOLDOWN: Duration = Duration::from_secs(1);
pub(super) const MAX_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum HealthScope {
    Quota(String),
    Fault(String),
}

/// Short-lived cooldown table shared by every GatewayState clone.
#[derive(Debug, Default)]
pub(super) struct TargetHealth {
    deadlines: Mutex<HashMap<HealthScope, Instant>>,
}

impl TargetHealth {
    /// Returns whether a new stateless request may select the target.
    pub(super) fn is_available(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        now: Instant,
    ) -> bool {
        // Generate registry-constrained quota and fault keys.
        let scopes = Self::target_scopes(target_id, target);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Remove expired entries and reject selection while either boundary is cooling down.
        deadlines.retain(|_, deadline| *deadline > now);
        scopes.iter().all(|scope| !deadlines.contains_key(scope))
    }

    /// Records the corresponding isolation boundary from an adapter HTTP-failure category.
    pub(super) fn record_http_failure(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        kind: UpstreamErrorKind,
        headers: &HeaderMap,
        now: Instant,
    ) {
        // Credential-member cooldown handles 429; isolate only target-level temporary unavailability here.
        let scope = match kind {
            UpstreamErrorKind::RateLimited => return,
            UpstreamErrorKind::UpstreamUnavailable => Self::fault_scope(target_id, target),
            UpstreamErrorKind::InvalidRequest
            | UpstreamErrorKind::Authentication
            | UpstreamErrorKind::UpstreamFailure => return,
        };
        let delay = retry_after_delay(headers).unwrap_or(DEFAULT_COOLDOWN);
        self.record(scope, now, delay);
    }

    /// Records a timeout or transport failure in the fault domain.
    pub(super) fn record_transport_failure(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        now: Instant,
    ) {
        self.record(Self::fault_scope(target_id, target), now, DEFAULT_COOLDOWN);
    }

    /// A successful response clears quota and fault cooldowns belonging to the target.
    pub(super) fn record_success(&self, target_id: &str, target: &UpstreamTarget) {
        // Clear only the two boundaries explicitly owned by this target; other registered scopes are unaffected.
        let scopes = Self::target_scopes(target_id, target);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for scope in scopes {
            deadlines.remove(&scope);
        }
    }

    /// Writes or extends a bounded scope cooldown, preserving a later existing deadline.
    fn record(&self, scope: HealthScope, now: Instant, delay: Duration) {
        // Cap the Provider-suggested cooldown so a malformed header cannot block an in-process Route for too long.
        let deadline = now + delay.min(MAX_COOLDOWN);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        deadlines
            .entry(scope)
            .and_modify(|current| *current = (*current).max(deadline))
            .or_insert(deadline);
    }

    /// Generates the explicit or default-derived quota and fault isolation keys for a target.
    fn target_scopes(target_id: &str, target: &UpstreamTarget) -> [HealthScope; 2] {
        [
            Self::quota_scope(target_id, target),
            Self::fault_scope(target_id, target),
        ]
    }

    /// Returns the target quota scope, falling back to the target ID when absent.
    fn quota_scope(target_id: &str, target: &UpstreamTarget) -> HealthScope {
        HealthScope::Quota(target.quota_scope().unwrap_or(target_id).to_owned())
    }

    /// Returns the target fault domain, falling back to the target ID when absent.
    fn fault_scope(target_id: &str, target: &UpstreamTarget) -> HealthScope {
        HealthScope::Fault(target.fault_domain().unwrap_or(target_id).to_owned())
    }
}

/// Parses delay from the two standard Retry-After formats and normalizes past dates to zero.
pub(super) fn retry_after_delay(headers: &HeaderMap) -> Option<Duration> {
    // Parse delta-seconds first, then the standard HTTP-date form.
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let retry_at = httpdate::parse_http_date(value).ok()?;
    Some(
        retry_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use http::{HeaderMap, HeaderValue};

    use super::retry_after_delay;

    #[test]
    fn retry_after_accepts_seconds_and_http_date() {
        // Verify that both standard forms become bounded cooldown inputs.
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("3"));
        assert_eq!(retry_after_delay(&headers), Some(Duration::from_secs(3)));

        headers.insert(
            "retry-after",
            HeaderValue::from_static("Wed, 21 Oct 2037 07:28:00 GMT"),
        );
        assert!(retry_after_delay(&headers).is_some_and(|delay| !delay.is_zero()));

        headers.insert(
            "retry-after",
            HeaderValue::from_static("Wed, 21 Oct 2015 07:28:00 GMT"),
        );
        assert_eq!(retry_after_delay(&headers), Some(Duration::ZERO));

        headers.insert("retry-after", HeaderValue::from_static("invalid"));
        assert_eq!(retry_after_delay(&headers), None);
    }
}
