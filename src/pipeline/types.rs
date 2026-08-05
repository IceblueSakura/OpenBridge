//! Request facts and Route execution-plan data types.

use crate::{
    bridge::BridgePlan,
    core::{
        ApiProtocol, ApiRequest, EmbeddingEncoding, EmbeddingInputForm, EmbeddingRequest,
        GenerationCapabilities, OperationKind,
    },
    registry::ReasoningLevel,
};

/// Registry-independent request facts extracted from a downstream request.
#[derive(Debug)]
pub struct RequestRequirements {
    pub(super) public_model: String,
    pub(super) protocol: ApiProtocol,
    pub(super) is_streaming: bool,
    pub(super) requested_output_tokens: Option<u64>,
    pub(super) requested_capabilities: RequestedCapabilities,
}

/// Registry-independent facts extracted from one strict Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRequestRequirements {
    pub(super) public_model: String,
    pub(super) input_form: EmbeddingInputForm,
    pub(super) input_count: u32,
    pub(super) token_counts: Option<Vec<u32>>,
    pub(super) requested_encoding: Option<EmbeddingEncoding>,
    pub(super) requested_dimensions: Option<u32>,
    pub(super) user_present: bool,
}

/// Single-candidate Native execution plan for an Embeddings Create request.
#[derive(Debug)]
pub struct EmbeddingRoutePlan {
    pub(super) candidate: EmbeddingRouteCandidate,
    pub(super) input_count: u32,
    pub(super) encoding: EmbeddingEncoding,
    pub(super) dimensions: u32,
}

/// Trusted Native Embeddings Route candidate bound to one target and Upstream API.
#[derive(Debug)]
pub struct EmbeddingRouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_operation: OperationKind,
    pub(super) request: EmbeddingRequest,
}

/// Execution plan that passed the Public Model fixed contract and binds ordered Routes.
///
/// Candidates retain Route configuration order. `allows_fallback` is not a general retry switch;
/// it prevents Provider-issued opaque state such as `previous_response_id` from being replayed to another target.
#[derive(Debug)]
pub struct RoutePlan {
    pub(super) candidates: Vec<RouteCandidate>,
    pub(super) is_streaming: bool,
    pub(super) allows_fallback: bool,
}

/// Execution candidate inheriting Public Model preflight and bound to one target/Upstream API.
#[derive(Debug)]
pub struct RouteCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) upstream_operation: OperationKind,
    pub(super) request: ApiRequest,
    pub(super) bridge: Option<BridgePlan>,
}

/// Capabilities actually used by one request. This is not the Upstream API configuration:
/// `generation` is shared by both endpoints,
/// requirement; Responses-specific state remains separate to avoid conflating the fixed contracts.
#[derive(Clone, Copy, Debug)]
pub(super) struct RequestedCapabilities {
    pub(super) generation: GenerationCapabilities,
    pub(super) unmodeled_tools: bool,
    pub(super) reasoning: RequestedReasoning,
    pub(super) previous_response_id: bool,
    pub(super) background: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RequestedReasoning {
    None,
    Unspecified,
    Level(ReasoningLevel),
    UnknownLevel,
    Conflicting,
}

impl RequestRequirements {
    /// Returns the Public Model selected by the downstream request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the native protocol used by the request.
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// Returns whether the request requires a streaming response.
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }
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
}

impl EmbeddingRouteCandidate {
    /// Returns the candidate Route ID.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the trusted Upstream Target ID.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the trusted typed Upstream API operation.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_operation
    }

    /// Returns the preserved Native Embeddings request.
    pub fn request(&self) -> &EmbeddingRequest {
        &self.request
    }
}

impl RoutePlan {
    /// Returns the highest-priority target ID.
    pub fn upstream_target_id(&self) -> &str {
        self.primary().upstream_target_id()
    }

    /// Returns the request for the highest-priority candidate.
    pub fn request(&self) -> &ApiRequest {
        self.primary().request()
    }

    /// Returns execution candidates ordered by configured Routes.
    pub fn candidates(&self) -> &[RouteCandidate] {
        &self.candidates
    }

    /// Returns whether the original request requires streaming.
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    /// Returns whether cross-target fallback is allowed.
    pub fn allows_fallback(&self) -> bool {
        self.allows_fallback
    }

    /// Consumes the plan and returns its highest-priority candidate request.
    pub fn into_request(self) -> ApiRequest {
        self.candidates
            .into_iter()
            .next()
            .expect("route plan always has a candidate")
            .request
    }

    /// Returns the guaranteed highest-priority candidate.
    fn primary(&self) -> &RouteCandidate {
        self.candidates
            .first()
            .expect("route plan always has a candidate")
    }
}

impl RouteCandidate {
    /// Returns the candidate Route ID.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the Upstream Target ID bound to the candidate.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the typed Upstream API operation bound to the candidate.
    pub fn upstream_operation(&self) -> OperationKind {
        self.upstream_operation
    }

    /// Returns the Native request for the candidate.
    pub fn request(&self) -> &ApiRequest {
        &self.request
    }

    /// Returns the response conversion plan for a Bridged Route; a Native candidate returns `None`.
    pub fn bridge(&self) -> Option<&BridgePlan> {
        self.bridge.as_ref()
    }
}
