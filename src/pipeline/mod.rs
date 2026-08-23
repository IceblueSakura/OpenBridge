//! Package entry point for request analysis and Route planning.
//!
//! Submodules own stable errors, plan data types, request-fact analysis, fixed-interface preflight,
//! and registry Route planning; this file only declares modules and preserves public API paths.

mod embeddings;
mod error;
mod generation;
mod images;
mod types;

pub(crate) use embeddings::{
    EmbeddingResponseError, validate_embedding_response_body, validate_embedding_response_headers,
};
pub use embeddings::{analyze_embedding_request, plan_embedding_request};
pub(crate) use error::GenerationCapabilityReason;
pub use error::{EmbeddingRequestError, ImagesRequestError, RequestPlanningError};
pub(crate) use generation::{
    GenerationResponseFacts, GenerationResponseMode, classify_generation_response,
    normalize_probe_generation_request,
};
pub use generation::{analyze_request, plan_request};
pub(crate) use images::{
    ImagesResponseError, validate_images_response_body, validate_images_response_headers,
};
pub use images::{analyze_images_request, plan_images_request};
pub(crate) use types::StreamResponseConversion;
pub use types::{
    EmbeddingRequestRequirements, EmbeddingRouteCandidate, EmbeddingRoutePlan,
    ImagesRequestRequirements, ImagesRequestedSize, ImagesRouteCandidate, ImagesRoutePlan,
    RequestRequirements, RouteCandidate, RoutePlan,
};
