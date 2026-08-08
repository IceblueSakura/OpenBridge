//! Private Public Model execution interfaces and safe public projections.
//!
//! Each operation keeps one conservative capability contract beside the exact static candidates
//! that produced it. Only the capability copy is projected into downstream Models responses.

use crate::core::{ApiProtocol, OperationKind, ReasoningOutput};

use super::{
    EmbeddingInterfaceCapabilities, ModelInterfaceCapabilities, ModelInterfaces, PublicModelInfo,
    StandardModel,
};
use crate::registry::{ModelLifecycleStatus, RouteMode, UpstreamStreamingPolicy};

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
    pub(super) upstream_operation: OperationKind,
    pub(super) mode: RouteMode,
    pub(super) upstream_model: String,
    pub(super) reasoning_output: ReasoningOutput,
    pub(super) streaming_policy: UpstreamStreamingPolicy,
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
        self.upstream_operation
    }

    /// Returns the downstream generation protocol guaranteed by a generation execution interface.
    pub(crate) fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_operation
            .api_protocol()
            .expect("generation candidates have a downstream API protocol")
    }

    /// Returns the upstream generation protocol guaranteed by a generation execution interface.
    pub(crate) fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_operation
            .api_protocol()
            .expect("generation candidates have an upstream API protocol")
    }

    /// Returns whether forwarding is Native or must use the restricted protocol bridge.
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
}

/// One immutable executable interface shared by request preflight and Route planning.
#[derive(Debug)]
pub(crate) struct ModelExecutionInterface {
    pub(super) generation_capabilities: Option<ModelInterfaceCapabilities>,
    pub(super) embedding_capabilities: Option<EmbeddingInterfaceCapabilities>,
    pub(super) candidates: Vec<RouteExecutionCandidate>,
}

impl ModelExecutionInterface {
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

    /// Returns whether the model remains visible to clients and has at least one executable interface.
    pub(crate) fn is_available(&self) -> bool {
        self.info.lifecycle.status != ModelLifecycleStatus::Retired
            && self.execution_interfaces.is_available()
    }
}
