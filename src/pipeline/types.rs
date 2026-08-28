//! Compatibility facade for operation-owned request facts and execution-plan types.

pub(crate) use super::generation::types::StreamResponseConversion;
pub use super::{
    embeddings::types::{
        EmbeddingRequestRequirements, EmbeddingRouteCandidate, EmbeddingRoutePlan,
    },
    generation::types::{RequestRequirements, RouteCandidate, RoutePlan},
    images::types::{
        ImagesRequestRequirements, ImagesRequestedSize, ImagesRouteCandidate, ImagesRoutePlan,
    },
};
pub(super) use super::{
    generation::types::{
        AudioInputRequirements, FileInputRequirements, GeneratedAudioMessageShape,
        ImageInputRequirements, InputAudioMessageShape, RequestedAsrLanguage, RequestedAsrOptions,
        RequestedAudio, RequestedAudioDelivery, RequestedCapabilities, RequestedInstructions,
        RequestedJsonSchemaStrictness, RequestedOutputTokens, RequestedParallelToolCalls,
        RequestedReasoning, RequestedReasoningSummary, RequestedStructuredOutput, RequestedVoice,
    },
    images::types::{DashScopeImagesRequestRequirements, ImagesUnsupportedStandardField},
};
