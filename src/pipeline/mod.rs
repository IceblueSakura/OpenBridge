//! Package entry point for request analysis and Route planning.
//!
//! Submodules own stable errors, plan data types, request-fact analysis, and registry Route
//! planning; this file only declares modules and preserves existing public API paths.

mod analysis;
mod error;
mod planning;
mod types;

pub use analysis::analyze_request;
pub use error::RequestPlanningError;
pub use planning::plan_request;
pub use types::{RequestRequirements, RouteCandidate, RoutePlan};
