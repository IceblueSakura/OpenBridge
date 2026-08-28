//! Compatibility facade for operation-owned analysis and planning errors.

pub use super::{
    embeddings::error::EmbeddingRequestError,
    generation::error::{GenerationCapabilityReason, RequestPlanningError},
    images::error::ImagesRequestError,
};
