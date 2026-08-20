//! Stateful execution coordination shared by operation-specific forwarding paths.
//!
//! This facade owns request-level attempt state only. Operation pipelines remain pure, while ingress
//! retains downstream admission and commit boundaries.

mod coordinator;

pub(crate) use coordinator::{AttemptCoordinator, AttemptStep};
