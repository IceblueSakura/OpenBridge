//! OpenAI-compatible endpoint and health-check handlers.

use axum::{
    Extension, Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use serde::Serialize;

use crate::{
    core::ApiProtocol,
    observability::RequestObservation,
    registry::{PublicModelInfo, StandardModel},
};

use super::{
    forwarding::forward_request,
    response::{api_error, model_not_found},
    state::GatewayState,
};

/// Builds the Public Model list from the immutable registry without exposing upstream models or targets.
pub(super) async fn models(
    State(state): State<GatewayState>,
) -> Json<ModelListResponse<StandardModel>> {
    // Project strict four-field OpenAI objects from complete Public Model information.
    let data = state
        .registry
        .public_models()
        .map(|model| model.standard().clone())
        .collect();
    Json(ModelListResponse {
        object: "list",
        data,
    })
}

/// Returns the strict four-field OpenAI projection for one Public Model.
pub(super) async fn model(
    State(state): State<GatewayState>,
    Path(model): Path<String>,
) -> Response {
    // Query the same immutable catalog as the list endpoint and hide all internal model/deployment information.
    state
        .registry
        .public_model(&model)
        .map(|model| Json(model.standard().clone()).into_response())
        .unwrap_or_else(model_not_found)
}

/// Returns the OpenBridge extended capability objects for all Public Models.
pub(super) async fn extended_models(
    State(state): State<GatewayState>,
) -> Json<ModelListResponse<PublicModelInfo>> {
    // Clone precompiled DTOs; the handler does not traverse Routes or rededuce capabilities during a request.
    let data = state
        .registry
        .public_models()
        .map(|model| model.info().clone())
        .collect();
    Json(ModelListResponse {
        object: "list",
        data,
    })
}

/// Returns the complete OpenBridge extended capability object for one Public Model.
pub(super) async fn extended_model(
    State(state): State<GatewayState>,
    Path(model): Path<String>,
) -> Response {
    // Reuse the same precompiled DTO as the extended list to keep fields identical.
    state
        .registry
        .public_model(&model)
        .map(|model| Json(model.info().clone()).into_response())
        .unwrap_or_else(model_not_found)
}

#[derive(Serialize)]
pub(super) struct ModelListResponse<T> {
    object: &'static str,
    data: Vec<T>,
}

/// Accepts a Chat Completions JSON request and sends it to the shared forwarding pipeline.
pub(super) async fn chat_completions(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Validate the native JSON media type before entering the shared Route/egress pipeline.
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(
        state,
        observation,
        ApiProtocol::ChatCompletions,
        headers,
        body,
    )
    .await
}

/// Accepts a Responses JSON request and sends it to the shared forwarding pipeline.
pub(super) async fn responses(
    State(state): State<GatewayState>,
    Extension(observation): Extension<RequestObservation>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Validate the native JSON media type before entering the shared Route/egress pipeline.
    if !has_json_content_type(&headers) {
        return unsupported_media_type();
    }
    forward_request(state, observation, ApiProtocol::Responses, headers, body).await
}

/// Returns whether the request carries exactly the application/json media type.
fn has_json_content_type(headers: &HeaderMap) -> bool {
    // Reject missing or duplicate Content-Type values so multiple values cannot create ambiguous boundaries.
    let mut values = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    // Ignore media-type parameters but accept only application/json.
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

/// Builds a stable protocol error response for a non-JSON request.
fn unsupported_media_type() -> Response {
    api_error(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "invalid_content_type",
        "Content-Type must be application/json",
    )
}

#[derive(Serialize)]
pub(super) struct HealthResponse {
    status: &'static str,
    registry_version: String,
}

/// Returns local service health and the current compiled registry version.
pub(super) async fn health(State(state): State<GatewayState>) -> Json<HealthResponse> {
    // Report service liveness and the compiled registry version only; do not probe real Providers.
    Json(HealthResponse {
        status: "ok",
        registry_version: state.registry.version().as_str().to_owned(),
    })
}
