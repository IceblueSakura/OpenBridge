//! Private Public Model execution interfaces and safe public projections.
//!
//! Each operation keeps one conservative capability contract beside the exact static candidates
//! that produced it. Only the capability copy is projected into downstream Models responses.

use crate::core::{ApiProtocol, OperationKind, ReasoningOutput};

use super::{
    EmbeddingInterfaceCapabilities, ModelInterfaceCapabilities, ModelInterfaces, PublicModelInfo,
    StandardModel,
};
use crate::registry::{
    CanonicalTaskKind, IgnorableGenerationParameter, ModelLifecycleStatus, RouteMode,
    UpstreamApiKey, UpstreamStreamingPolicy,
};

/// Private identity of the one Target/API allowed to receive an opaque continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContinuationIssuer {
    upstream_target: String,
    upstream_api_key: UpstreamApiKey,
}

impl ContinuationIssuer {
    /// Freezes one validated Target/API identity for Public Model continuation aggregation.
    pub(super) fn new(upstream_target: String, upstream_api_key: UpstreamApiKey) -> Self {
        Self {
            upstream_target,
            upstream_api_key,
        }
    }

    /// Returns whether a planning candidate belongs to this exact issuing Target/API.
    fn matches(&self, upstream_target: &str, upstream_api_key: UpstreamApiKey) -> bool {
        self.upstream_target == upstream_target && self.upstream_api_key == upstream_api_key
    }
}

/// Private continuation contract compiled from every candidate behind one downstream interface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum PublicContinuationContract {
    /// The interface cannot safely accept an opaque Provider-issued response ID.
    #[default]
    Unsupported,
    /// Every candidate resolves continuation to the same issuing Target/API.
    Supported {
        /// Unique issuer retained only for preflight and planning invariants.
        issuer: ContinuationIssuer,
    },
}

impl PublicContinuationContract {
    /// Creates a supported contract after conservative Route aggregation proves one issuer.
    pub(super) fn supported(issuer: ContinuationIssuer) -> Self {
        Self::Supported { issuer }
    }

    /// Returns whether request preflight may accept `previous_response_id`.
    pub(super) const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    /// Returns whether one candidate belongs to the unique compiled continuation issuer.
    fn candidate_matches(&self, candidate: &RouteExecutionCandidate) -> bool {
        match self {
            Self::Unsupported => false,
            Self::Supported { issuer } => {
                issuer.matches(candidate.upstream_target_id(), candidate.upstream_api_key())
            }
        }
    }
}

/// Private execution candidate compiled from one statically executable Route.
///
/// This type is never serialized or exposed by a downstream API. It freezes the Route identity and
/// the planning facts needed to construct a Native request or `BridgePlan` without re-resolving the
/// Public Model's configured Route list during a request.
#[derive(Clone, Debug)]
pub(crate) struct RouteExecutionCandidate {
    pub(super) route_id: String,
    pub(super) upstream_target_id: String,
    pub(super) downstream_operation: OperationKind,
    pub(super) upstream_api_key: UpstreamApiKey,
    pub(super) mode: RouteMode,
    pub(super) upstream_model: String,
    pub(super) reasoning_output: ReasoningOutput,
    pub(super) streaming_policy: UpstreamStreamingPolicy,
    pub(super) ignored_parameters: Vec<IgnorableGenerationParameter>,
}

impl RouteExecutionCandidate {
    /// Returns the configured Route ID retained for forwarding diagnostics and attempt attribution.
    pub(crate) fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the prevalidated Upstream Target ID used by forwarding.
    pub(crate) fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the downstream operation represented by this interface candidate.
    pub(crate) const fn downstream_operation(&self) -> OperationKind {
        self.downstream_operation
    }

    /// Returns the upstream operation represented by this interface candidate.
    pub(crate) const fn upstream_operation(&self) -> OperationKind {
        self.upstream_api_key.operation()
    }

    /// Returns the complete validated Upstream API identity.
    pub(crate) const fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the downstream generation protocol guaranteed by a generation execution interface.
    pub(crate) fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_operation
            .api_protocol()
            .expect("generation candidates have a downstream API protocol")
    }

    /// Returns the compiled Native or explicitly directed Generation Bridge mode.
    pub(crate) const fn mode(&self) -> RouteMode {
        self.mode
    }

    /// Returns the trusted model ID used only while rendering a bridged upstream request.
    pub(crate) fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    /// Returns the upstream reasoning-output classification required by bridge preparation.
    pub(crate) const fn reasoning_output(&self) -> ReasoningOutput {
        self.reasoning_output
    }

    /// Returns the trusted streaming requirement and non-streaming conversion policy.
    pub(crate) const fn streaming_policy(&self) -> UpstreamStreamingPolicy {
        self.streaming_policy
    }

    /// Returns the ordinary parameters removed only for this candidate before shape conversion.
    pub(crate) fn ignored_generation_parameters(&self) -> &[IgnorableGenerationParameter] {
        &self.ignored_parameters
    }
}

/// One immutable executable interface shared by request preflight and Route planning.
#[derive(Debug)]
pub(crate) struct ModelExecutionInterface {
    pub(super) task: CanonicalTaskKind,
    pub(super) generation_capabilities: Option<ModelInterfaceCapabilities>,
    pub(super) embedding_capabilities: Option<EmbeddingInterfaceCapabilities>,
    pub(super) continuation: PublicContinuationContract,
    pub(super) candidates: Vec<RouteExecutionCandidate>,
}

impl ModelExecutionInterface {
    /// Returns the single canonical task selected at startup for this interface.
    pub(crate) const fn task(&self) -> CanonicalTaskKind {
        self.task
    }

    /// Returns the fixed capability contract derived from exactly these static candidates.
    pub(crate) const fn capabilities(&self) -> &ModelInterfaceCapabilities {
        self.generation_capabilities
            .as_ref()
            .expect("generation preflight selected a generation execution interface")
    }

    /// Returns the fixed Embeddings contract derived from this interface's single Native candidate.
    pub(crate) const fn embedding_capabilities(&self) -> Option<&EmbeddingInterfaceCapabilities> {
        self.embedding_capabilities.as_ref()
    }

    /// Returns whether the private compiled contract admits `previous_response_id`.
    pub(crate) const fn supports_previous_response_id(&self) -> bool {
        self.continuation.is_supported()
    }

    /// Confirms that every candidate belongs to the one compiled continuation issuer.
    pub(crate) fn continuation_candidates_match_issuer(&self) -> bool {
        self.continuation.is_supported()
            && self
                .candidates
                .iter()
                .all(|candidate| self.continuation.candidate_matches(candidate))
    }

    /// Returns static candidates in their configured priority order.
    pub(crate) fn candidates(&self) -> &[RouteExecutionCandidate] {
        &self.candidates
    }
}

/// Operation execution interfaces compiled from one Public Model's static Route bindings.
#[derive(Debug)]
pub(super) struct ModelExecutionInterfaces {
    pub(super) chat_completions: Option<ModelExecutionInterface>,
    pub(super) responses: Option<ModelExecutionInterface>,
    pub(super) embeddings: Option<ModelExecutionInterface>,
}

impl ModelExecutionInterfaces {
    /// Returns the interface that owns both preflight capabilities and planning candidates.
    const fn for_operation(&self, operation: OperationKind) -> Option<&ModelExecutionInterface> {
        match operation {
            OperationKind::ChatCompletions => self.chat_completions.as_ref(),
            OperationKind::Responses => self.responses.as_ref(),
            OperationKind::EmbeddingsCreate => self.embeddings.as_ref(),
        }
    }

    /// Returns whether any executable operation interface selects the requested task.
    fn has_task(&self, task: CanonicalTaskKind) -> bool {
        [
            self.chat_completions.as_ref(),
            self.responses.as_ref(),
            self.embeddings.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|interface| interface.task() == task)
    }

    /// Returns whether this Public Model has any statically executable downstream protocol.
    const fn is_available(&self) -> bool {
        self.chat_completions.is_some() || self.responses.is_some() || self.embeddings.is_some()
    }

    /// Projects capability copies into the safe Models response without candidate topology.
    pub(super) fn public_projection(&self) -> ModelInterfaces {
        ModelInterfaces {
            chat_completions: self
                .chat_completions
                .as_ref()
                .and_then(|interface| interface.generation_capabilities.clone()),
            responses: self
                .responses
                .as_ref()
                .and_then(|interface| interface.generation_capabilities.clone()),
            embeddings: self
                .embeddings
                .as_ref()
                .and_then(|interface| interface.embedding_capabilities.clone()),
        }
    }
}

/// Resolved downstream Public Model, fixed information object, diagnostic Route IDs, and execution interfaces.
#[derive(Debug)]
pub struct PublicModel {
    pub(super) routes: Vec<String>,
    pub(super) execution_interfaces: ModelExecutionInterfaces,
    pub(super) info: PublicModelInfo,
}

impl PublicModel {
    /// Returns configured Route IDs ordered by priority for diagnostics and tests.
    ///
    /// Request planning does not read this raw list; it consumes the protocol-specific static
    /// candidate set in [`Self::execution_interface`].
    pub fn routes(&self) -> &[String] {
        &self.routes
    }

    /// Returns complete safe model information for the extension interface.
    pub fn info(&self) -> &PublicModelInfo {
        &self.info
    }

    /// Returns the standard OpenAI Models resource projection.
    pub fn standard(&self) -> &StandardModel {
        self.info.standard()
    }

    /// Returns the precompiled interface used by both request preflight and Route planning.
    pub(crate) const fn execution_interface(
        &self,
        operation: OperationKind,
    ) -> Option<&ModelExecutionInterface> {
        self.execution_interfaces.for_operation(operation)
    }

    /// Returns whether one downstream generation protocol has an executable Native candidate.
    ///
    /// This predicate intentionally exposes neither the matching candidate nor any deployment
    /// identity to the Models handler.
    pub(crate) fn has_native_candidate(&self, protocol: ApiProtocol) -> bool {
        self.execution_interface(protocol.operation())
            .is_some_and(|interface| {
                interface
                    .candidates()
                    .iter()
                    .any(|candidate| candidate.mode() == RouteMode::Native)
            })
    }

    /// Returns whether the model remains visible to clients and has at least one executable interface.
    pub(crate) fn is_available(&self) -> bool {
        self.info.lifecycle.status != ModelLifecycleStatus::Retired
            && self.execution_interfaces.is_available()
    }

    /// Returns whether any executable operation interface selects general Generation.
    pub(crate) fn has_general_generation_interface(&self) -> bool {
        self.execution_interfaces
            .has_task(CanonicalTaskKind::Generation)
    }
}
