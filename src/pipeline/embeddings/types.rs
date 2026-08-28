//! Embeddings request facts and single-candidate Native execution-plan types.

use crate::{
    core::{EmbeddingEncoding, EmbeddingInputForm, EmbeddingRequest, OperationKind},
    registry::{OperationResponseBudget, UpstreamApiKey},
};

/// Registry-independent facts extracted from one strict Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRequestRequirements {
    pub(in crate::pipeline) public_model: String,
    pub(in crate::pipeline) input_form: EmbeddingInputForm,
    pub(in crate::pipeline) input_count: u32,
    pub(in crate::pipeline) token_counts: Option<Vec<u32>>,
    pub(in crate::pipeline) requested_encoding: Option<EmbeddingEncoding>,
    pub(in crate::pipeline) requested_dimensions: Option<u32>,
    pub(in crate::pipeline) user_present: bool,
}

/// Single-candidate Native execution plan for an Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRoutePlan {
    pub(in crate::pipeline) candidate: EmbeddingRouteCandidate,
    pub(in crate::pipeline) input_count: u32,
    pub(in crate::pipeline) encoding: EmbeddingEncoding,
    pub(in crate::pipeline) dimensions: u32,
    pub(in crate::pipeline) response_budget: OperationResponseBudget,
}

/// Trusted Native Embeddings Route candidate bound to one target and Upstream API.
#[derive(Debug)]
pub struct EmbeddingRouteCandidate {
    pub(in crate::pipeline) upstream_target_id: String,
    pub(in crate::pipeline) upstream_api_key: UpstreamApiKey,
    pub(in crate::pipeline) request: EmbeddingRequest,
}

impl EmbeddingRequestRequirements {
    /// Returns the Public Model selected by the downstream Embeddings request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the exact analyzed input form.
    pub fn input_form(&self) -> EmbeddingInputForm {
        self.input_form
    }

    /// Returns the number of logical embedding inputs.
    pub fn input_count(&self) -> u32 {
        self.input_count
    }
}

impl EmbeddingRoutePlan {
    /// Returns the single trusted Embeddings candidate.
    pub fn candidate(&self) -> &EmbeddingRouteCandidate {
        &self.candidate
    }

    /// Returns the expected number of response vectors.
    pub fn input_count(&self) -> u32 {
        self.input_count
    }

    /// Returns the effective output encoding after fixed-interface preflight.
    pub fn encoding(&self) -> EmbeddingEncoding {
        self.encoding
    }

    /// Returns the effective vector dimension after fixed-interface preflight.
    pub fn dimensions(&self) -> u32 {
        self.dimensions
    }

    /// Returns the JSON response limit compiled with the Embeddings interface.
    pub(crate) const fn max_json_response_body_bytes(&self) -> usize {
        self.response_budget.max_json_body_bytes()
    }
}

impl EmbeddingRouteCandidate {
    /// Returns the trusted Upstream Target ID.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the trusted typed Upstream API operation.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_api_key.operation()
    }

    /// Returns the complete trusted Upstream API identity.
    pub fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the preserved Native Embeddings request.
    pub fn request(&self) -> &EmbeddingRequest {
        &self.request
    }
}
