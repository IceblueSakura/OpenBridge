//! Prepared-candidate attempt execution shared by operation forwarding handlers.
//!
//! A single runner owns select/header/attempt/send/retry/backoff. A closed driver preserves the
//! operation-specific OAuth, replay, health, response-validation, and commit policies.

use axum::response::Response;
use http::HeaderMap;

mod driver;
mod runner;

use crate::{
    bridge::BridgePlan,
    core::ApiProtocol,
    credential::UpstreamCredential,
    execution::AttemptCoordinator,
    observability::RequestObservation,
    pipeline::{EmbeddingRequestRequirements, EmbeddingRoutePlan, RouteCandidate, RoutePlan},
    provider::{PreparedUpstreamRequest, ProviderAdapter},
    registry::{CredentialPoolBinding, UpstreamApi, UpstreamTarget},
    transport::upstream::UpstreamResponse,
};

use super::candidate::PreparedCandidate;
use crate::ingress::state::GatewayState;

/// Retryable HTTP response retained while a later Generation candidate is attempted.
pub(super) struct StoredHttpFailure {
    pub(super) upstream: UpstreamResponse,
    pub(super) adapter: ProviderAdapter,
    pub(super) upstream_protocol: ApiProtocol,
    pub(super) bridge: Option<BridgePlan>,
}

/// Terminal response or request to continue the outer fixed Generation candidate sequence.
pub(super) enum GenerationCandidateOutcome {
    /// The selected candidate reached a terminal downstream response.
    Response(Response),
    /// The selected candidate yielded to the next configured candidate.
    NextCandidate {
        /// Retryable HTTP response preserved for final fallback rendering.
        failure: Option<StoredHttpFailure>,
        /// Whether this candidate was unavailable before an upstream attempt began.
        cooldown_skipped: bool,
    },
}

/// Trusted data needed to execute one prepared Generation candidate.
pub(super) struct PreparedGenerationExecution<'a> {
    pub(super) plan: &'a RoutePlan,
    pub(super) candidate: &'a RouteCandidate,
    pub(super) target: &'a UpstreamTarget,
    pub(super) prepared: PreparedCandidate<'a>,
    pub(super) candidate_index: usize,
    pub(super) candidate_count: usize,
}

/// Executes one prepared Generation candidate without owning outer Route order or final fallback rendering.
pub(super) async fn run_generation_candidate(
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
    attempts: &mut AttemptCoordinator,
    execution: PreparedGenerationExecution<'_>,
) -> GenerationCandidateOutcome {
    runner::run(
        state,
        observation,
        downstream_headers,
        attempts,
        driver::OperationDriver::generation(execution),
    )
    .await
}

/// Trusted data needed to execute one prepared Embeddings candidate.
pub(super) struct PreparedEmbeddingExecution<'a> {
    pub(super) requirements: &'a EmbeddingRequestRequirements,
    pub(super) plan: &'a EmbeddingRoutePlan,
    pub(super) target: &'a UpstreamTarget,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) credential_pool: &'a CredentialPoolBinding,
    pub(super) credentials: Vec<UpstreamCredential<'a>>,
    pub(super) adapter: ProviderAdapter,
    pub(super) request: PreparedUpstreamRequest,
    pub(super) replayable: bool,
}

/// Executes one prepared Native Embeddings candidate without owning analysis or planning.
pub(super) async fn run_embedding_candidate(
    state: &GatewayState,
    observation: &RequestObservation,
    downstream_headers: &HeaderMap,
    attempts: &mut AttemptCoordinator,
    execution: PreparedEmbeddingExecution<'_>,
) -> Response {
    match runner::run(
        state,
        observation,
        downstream_headers,
        attempts,
        driver::OperationDriver::embeddings(execution),
    )
    .await
    {
        GenerationCandidateOutcome::Response(response) => response,
        GenerationCandidateOutcome::NextCandidate { .. } => {
            unreachable!("Embeddings has no cross-candidate fallback")
        }
    }
}
