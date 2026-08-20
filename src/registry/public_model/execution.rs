//! Private Public Model execution interfaces and safe public projections.
//!
//! Each operation keeps one conservative capability contract beside the exact static candidates
//! that produced it. Only the capability copy is projected into downstream Models responses.

use std::collections::BTreeMap;

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

/// Private executable contract selected by one closed downstream operation.
#[derive(Clone, Debug)]
pub(super) enum OperationExecutionContract {
    /// Chat Completions or Responses generation contract.
    Generation(Box<ModelInterfaceCapabilities>),
    /// Embeddings create contract.
    Embeddings(EmbeddingInterfaceCapabilities),
}

impl OperationExecutionContract {
    fn supports_operation(&self, operation: OperationKind) -> bool {
        matches!(
            (self, operation),
            (
                Self::Generation(_),
                OperationKind::ChatCompletions | OperationKind::Responses
            ) | (Self::Embeddings(_), OperationKind::EmbeddingsCreate)
        )
    }

    fn generation(&self) -> Option<&ModelInterfaceCapabilities> {
        match self {
            Self::Generation(capabilities) => Some(capabilities.as_ref()),
            Self::Embeddings(_) => None,
        }
    }

    fn embeddings(&self) -> Option<&EmbeddingInterfaceCapabilities> {
        match self {
            Self::Generation(_) => None,
            Self::Embeddings(capabilities) => Some(capabilities),
        }
    }
}

/// Response buffering and SSE limits owned by one operation interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationResponseBudget {
    /// Generation may return bounded JSON or per-event bounded SSE.
    Generation {
        /// Maximum successful JSON response body size.
        max_json_body_bytes: usize,
        /// Maximum size of one SSE event.
        max_sse_event_bytes: usize,
    },
    /// Embeddings returns bounded JSON and never SSE.
    Embeddings {
        /// Maximum successful JSON response body size.
        max_json_body_bytes: usize,
    },
}

impl OperationResponseBudget {
    fn supports_operation(self, operation: OperationKind) -> bool {
        matches!(
            (self, operation),
            (
                Self::Generation { .. },
                OperationKind::ChatCompletions | OperationKind::Responses
            ) | (Self::Embeddings { .. }, OperationKind::EmbeddingsCreate)
        )
    }

    /// Returns the operation's successful JSON response body limit.
    pub(crate) const fn max_json_body_bytes(self) -> usize {
        match self {
            Self::Generation {
                max_json_body_bytes,
                ..
            }
            | Self::Embeddings {
                max_json_body_bytes,
            } => max_json_body_bytes,
        }
    }

    /// Returns the per-event SSE limit only for generation operations.
    pub(crate) const fn max_sse_event_bytes(self) -> Option<usize> {
        match self {
            Self::Generation {
                max_sse_event_bytes,
                ..
            } => Some(max_sse_event_bytes),
            Self::Embeddings { .. } => None,
        }
    }
}

/// One immutable executable interface shared by request preflight and Route planning.
#[derive(Debug)]
pub(crate) struct ModelExecutionInterface {
    pub(super) operation: OperationKind,
    pub(super) task: CanonicalTaskKind,
    pub(super) contract: OperationExecutionContract,
    pub(super) continuation: PublicContinuationContract,
    pub(super) response_budget: OperationResponseBudget,
    pub(super) candidates: Vec<RouteExecutionCandidate>,
}

impl ModelExecutionInterface {
    /// Returns the single downstream operation indexing this interface.
    pub(crate) const fn operation(&self) -> OperationKind {
        self.operation
    }

    /// Returns the single canonical task selected at startup for this interface.
    pub(crate) const fn task(&self) -> CanonicalTaskKind {
        self.task
    }

    /// Returns the fixed capability contract derived from exactly these static candidates.
    pub(crate) fn capabilities(&self) -> &ModelInterfaceCapabilities {
        self.contract
            .generation()
            .expect("generation preflight selected a generation execution interface")
    }

    /// Returns the fixed Embeddings contract derived from this interface's single Native candidate.
    pub(crate) fn embedding_capabilities(&self) -> Option<&EmbeddingInterfaceCapabilities> {
        self.contract.embeddings()
    }

    /// Returns the response limits compiled beside this operation contract.
    pub(crate) const fn response_budget(&self) -> OperationResponseBudget {
        self.response_budget
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OperationInterfaceIndexError {
    /// More than one interface declared the same downstream operation.
    Duplicate(OperationKind),
    /// Operation, contract, budget, task, or candidate identities diverged.
    Inconsistent(OperationKind),
}

#[derive(Debug)]
pub(super) struct ModelExecutionInterfaces {
    by_operation: BTreeMap<OperationKind, ModelExecutionInterface>,
}

impl ModelExecutionInterfaces {
    /// Builds one deterministic operation index and rejects duplicate or mismatched interfaces.
    pub(super) fn try_from_iter(
        interfaces: impl IntoIterator<Item = ModelExecutionInterface>,
    ) -> Result<Self, OperationInterfaceIndexError> {
        let by_operation = collect_unique_operations(
            interfaces
                .into_iter()
                .map(|interface| (interface.operation(), interface)),
        )?;
        if let Some((operation, _)) = by_operation.iter().find(|(operation, interface)| {
            interface.operation() != **operation
                || !interface.contract.supports_operation(**operation)
                || !interface.response_budget.supports_operation(**operation)
                || interface.candidates().is_empty()
                || (**operation == OperationKind::EmbeddingsCreate
                    && interface.continuation.is_supported())
                || (interface.continuation.is_supported()
                    && !interface.continuation_candidates_match_issuer())
                || interface.candidates().iter().any(|candidate| {
                    candidate.downstream_operation() != **operation
                        || candidate.upstream_api_key().task() != interface.task()
                })
        }) {
            return Err(OperationInterfaceIndexError::Inconsistent(*operation));
        }
        Ok(Self { by_operation })
    }

    /// Returns the interface that owns both preflight capabilities and planning candidates.
    fn for_operation(&self, operation: OperationKind) -> Option<&ModelExecutionInterface> {
        self.by_operation.get(&operation)
    }

    /// Returns whether any executable operation interface selects the requested task.
    fn has_task(&self, task: CanonicalTaskKind) -> bool {
        self.by_operation
            .values()
            .any(|interface| interface.task() == task)
    }

    /// Returns whether this Public Model has any statically executable downstream protocol.
    fn is_available(&self) -> bool {
        !self.by_operation.is_empty()
    }

    /// Projects capability copies into the safe Models response without candidate topology.
    pub(super) fn public_projection(&self) -> ModelInterfaces {
        ModelInterfaces {
            chat_completions: self
                .for_operation(OperationKind::ChatCompletions)
                .and_then(|interface| interface.contract.generation().cloned()),
            responses: self
                .for_operation(OperationKind::Responses)
                .and_then(|interface| interface.contract.generation().cloned()),
            embeddings: self
                .for_operation(OperationKind::EmbeddingsCreate)
                .and_then(|interface| interface.contract.embeddings().cloned()),
        }
    }
}

fn collect_unique_operations<T>(
    entries: impl IntoIterator<Item = (OperationKind, T)>,
) -> Result<BTreeMap<OperationKind, T>, OperationInterfaceIndexError> {
    let mut by_operation = BTreeMap::new();
    for (operation, value) in entries {
        if by_operation.insert(operation, value).is_some() {
            return Err(OperationInterfaceIndexError::Duplicate(operation));
        }
    }
    Ok(by_operation)
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
    pub(crate) fn execution_interface(
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

#[cfg(test)]
mod tests {
    use super::super::{
        EmbeddingDimensionCapabilities, EmbeddingEncodingCapabilities, EmbeddingLimits,
    };
    use super::*;
    use crate::core::EmbeddingEncoding;

    fn embedding_interface(response_budget: OperationResponseBudget) -> ModelExecutionInterface {
        ModelExecutionInterface {
            operation: OperationKind::EmbeddingsCreate,
            task: CanonicalTaskKind::Embedding,
            contract: OperationExecutionContract::Embeddings(EmbeddingInterfaceCapabilities {
                input_forms: Vec::new(),
                encoding: EmbeddingEncodingCapabilities {
                    default: EmbeddingEncoding::Float,
                    allowed: None,
                },
                dimensions: EmbeddingDimensionCapabilities {
                    default: 1,
                    allowed: None,
                },
                limits: EmbeddingLimits {
                    max_inputs: 1,
                    max_tokens_per_input: None,
                    max_total_tokens: None,
                    locally_counted_input_forms: Vec::new(),
                },
                supported_parameters: Vec::new(),
            }),
            continuation: PublicContinuationContract::Unsupported,
            response_budget,
            candidates: vec![RouteExecutionCandidate {
                route_id: "route".to_owned(),
                upstream_target_id: "target".to_owned(),
                downstream_operation: OperationKind::EmbeddingsCreate,
                upstream_api_key: UpstreamApiKey::new(
                    OperationKind::EmbeddingsCreate,
                    CanonicalTaskKind::Embedding,
                ),
                mode: RouteMode::Native,
                upstream_model: "model".to_owned(),
                reasoning_output: ReasoningOutput::Unknown,
                streaming_policy: UpstreamStreamingPolicy::Optional,
                ignored_parameters: Vec::new(),
            }],
        }
    }

    #[test]
    fn operation_index_rejects_duplicate_and_inconsistent_interfaces() {
        let duplicate = ModelExecutionInterfaces::try_from_iter([
            embedding_interface(OperationResponseBudget::Embeddings {
                max_json_body_bytes: 1,
            }),
            embedding_interface(OperationResponseBudget::Embeddings {
                max_json_body_bytes: 1,
            }),
        ])
        .unwrap_err();
        assert_eq!(
            duplicate,
            OperationInterfaceIndexError::Duplicate(OperationKind::EmbeddingsCreate)
        );

        let inconsistent = ModelExecutionInterfaces::try_from_iter([embedding_interface(
            OperationResponseBudget::Generation {
                max_json_body_bytes: 1,
                max_sse_event_bytes: 1,
            },
        )])
        .unwrap_err();
        assert_eq!(
            inconsistent,
            OperationInterfaceIndexError::Inconsistent(OperationKind::EmbeddingsCreate)
        );
    }
}
