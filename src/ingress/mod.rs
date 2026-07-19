use std::sync::Arc;

use axum::{Json, Router, extract::State, http::header::AUTHORIZATION, routing::get};
use http::HeaderName;
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::{
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    trace::TraceLayer,
};

use crate::config::ConfigManager;

pub fn build_router(config: Arc<ConfigManager>) -> Router {
    let max_request_body_bytes = config.snapshot().limits().max_request_body_bytes();
    let request_id = HeaderName::from_static("x-request-id");
    let middleware = ServiceBuilder::new()
        .layer(SetSensitiveRequestHeadersLayer::new(std::iter::once(
            AUTHORIZATION,
        )))
        .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id))
        .layer(RequestBodyLimitLayer::new(max_request_body_bytes));

    Router::new()
        .route("/healthz", get(health))
        .layer(middleware)
        .with_state(config)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    config_version: String,
}

async fn health(State(config): State<Arc<ConfigManager>>) -> Json<HealthResponse> {
    let snapshot = config.snapshot();
    Json(HealthResponse {
        status: "ok",
        config_version: snapshot.version().as_str().to_owned(),
    })
}
