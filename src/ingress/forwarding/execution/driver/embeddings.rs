//! Embeddings-specific response validation, success accounting, and transport error mapping.

use axum::response::Response;
use http::StatusCode;

use crate::{
    credential::UpstreamCredential,
    ingress::{
        forwarding::{
            embedding_response::validated_embedding_response, response::UpstreamResponseOutcome,
        },
        response::{
            embedding_server_error, embedding_upstream_error, normalized_embedding_upstream_error,
        },
        state::GatewayState,
    },
    observability::{ErrorType, RequestObservation},
    pipeline::{EmbeddingRequestRequirements, EmbeddingRoutePlan},
    provider::EmbeddingsProviderAdapter,
    registry::{CredentialPoolBinding, UpstreamApi, UpstreamTarget},
    transport::upstream::{TransportError, UpstreamResponse},
};

/// Trusted Embeddings state needed only for response validation and success accounting.
pub(super) struct CompletionContext<'a, 'credential> {
    pub(super) member_index: usize,
    pub(super) requirements: &'a EmbeddingRequestRequirements,
    pub(super) plan: &'a EmbeddingRoutePlan,
    pub(super) target: &'a UpstreamTarget,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) credential_pool: &'a CredentialPoolBinding,
    pub(super) credentials: &'a [UpstreamCredential<'credential>],
    pub(super) adapter: EmbeddingsProviderAdapter,
}

pub(super) async fn finish_http(
    state: &GatewayState,
    observation: &RequestObservation,
    upstream: UpstreamResponse,
    context: CompletionContext<'_, '_>,
) -> UpstreamResponseOutcome {
    if !upstream.status().is_success() {
        return normalized_embedding_upstream_error(upstream).into();
    }
    let response = match validated_embedding_response(
        upstream,
        observation,
        context.requirements.public_model(),
        context.upstream_api.upstream_model(),
        context.plan.input_count(),
        context.plan.encoding(),
        context.plan.dimensions(),
        context.plan.max_json_response_body_bytes(),
        context.adapter,
        context.upstream_api.embedding_encoding_policy(),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            observation.record_stream_failure(ErrorType::InvalidUpstreamResponse);
            return embedding_server_error(
                StatusCode::BAD_GATEWAY,
                "invalid_upstream_response",
                "The upstream response is invalid",
            )
            .into();
        }
    };
    state.credential_health.record_success(
        context.credential_pool.id(),
        &context.credentials[context.member_index],
    );
    state.health.record_success(
        context.plan.candidate().upstream_target_id(),
        context.target,
    );
    response.into()
}

pub(super) fn finish_transport(error: TransportError) -> Response {
    embedding_upstream_error(error)
}
