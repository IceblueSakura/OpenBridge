//! Generation-specific successful-response completion and transport error mapping.

use axum::response::Response;

use crate::{
    credential::UpstreamCredential,
    ingress::{
        forwarding::{
            execution::StoredHttpFailure,
            response::{UpstreamResponseContext, UpstreamResponseOutcome, upstream_response},
        },
        response::upstream_error,
        state::GatewayState,
    },
    observability::RequestObservation,
    pipeline::{RouteCandidate, RoutePlan},
    provider::GenerationProviderAdapter,
    registry::{CredentialPoolBinding, UpstreamTarget},
    transport::upstream::{TransportError, UpstreamResponse},
};

/// Trusted Generation state needed only for successful-response completion.
pub(super) struct CompletionContext<'a, 'credential> {
    pub(super) member_index: usize,
    pub(super) plan: &'a RoutePlan,
    pub(super) candidate: &'a RouteCandidate,
    pub(super) target: &'a UpstreamTarget,
    pub(super) credential_pool: &'a CredentialPoolBinding,
    pub(super) static_credentials: Option<&'a [UpstreamCredential<'credential>]>,
    pub(super) adapter: GenerationProviderAdapter,
}

pub(super) async fn finish_http(
    state: &GatewayState,
    observation: &RequestObservation,
    upstream: UpstreamResponse,
    context: CompletionContext<'_, '_>,
) -> UpstreamResponseOutcome {
    let outcome = upstream_response(
        upstream,
        UpstreamResponseContext {
            validate_sse: context.plan.is_streaming(),
            adapter: context.adapter,
            max_sse_event_bytes: context.plan.max_sse_event_bytes(),
            max_json_body_bytes: context.plan.max_json_response_body_bytes(),
            bridge: context.candidate.bridge().cloned(),
            stream_response_conversion: context.candidate.stream_response_conversion(),
            observation: observation.clone(),
        },
    )
    .await;
    if matches!(
        &outcome,
        UpstreamResponseOutcome::Response(response) if response.status().is_success()
    ) {
        if let Some(credentials) = context.static_credentials {
            state.credential_health.record_success(
                context.credential_pool.id(),
                &credentials[context.member_index],
            );
        }
        state
            .health
            .record_success(context.candidate.upstream_target_id(), context.target);
    }
    outcome
}

pub(super) fn finish_transport(error: TransportError) -> Response {
    upstream_error(error)
}

pub(super) fn stored_http_failure(
    upstream: UpstreamResponse,
    candidate: &RouteCandidate,
    adapter: GenerationProviderAdapter,
) -> StoredHttpFailure {
    StoredHttpFailure {
        upstream,
        adapter,
        bridge: candidate.bridge().cloned(),
    }
}
