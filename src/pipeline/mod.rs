//! Package entry point for request analysis and Route planning.
//!
//! Submodules own stable errors, plan data types, request-fact analysis, fixed-interface preflight,
//! and registry Route planning; this file only declares modules and preserves public API paths.

mod analysis;
mod embeddings;
mod error;
mod instructions;
mod planning;
mod preflight;
mod types;

pub use analysis::analyze_request;
pub(crate) use embeddings::{
    EmbeddingResponseError, validate_embedding_response_body, validate_embedding_response_headers,
};
pub use embeddings::{analyze_embedding_request, plan_embedding_request};
pub use error::{EmbeddingRequestError, RequestPlanningError};
pub(crate) use instructions::normalize_probe_generation_request;
pub use planning::plan_request;
pub(crate) use types::StreamResponseConversion;
pub use types::{
    EmbeddingRequestRequirements, EmbeddingRouteCandidate, EmbeddingRoutePlan, RequestRequirements,
    RouteCandidate, RoutePlan,
};
