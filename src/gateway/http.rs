//! HTTP ingress owns authentication ordering, request deadlines and listener lifecycle.
use super::{
    ApiError, Gateway, Runtime, admission, body,
    diagnostics::{Stage, Trace},
};
use crate::protocol::openai::Profile;
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::net::TcpListener;
// Aborting the listener future must also stop detached body workers and the
// graceful-shutdown watcher, not leave them alive until their request deadlines.
struct StopOnDrop(Gateway);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
impl Gateway {
    /// Embedding callers own listener security; `serve` enforces loopback itself.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/v1/chat/completions", post(chat))
            .route("/v1/responses", post(responses))
            .fallback(|| async { ApiError::new(StatusCode::NOT_FOUND, "not_found") })
            .method_not_allowed_fallback(|| async {
                ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
            })
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                authenticate,
            ))
            .with_state(self.state.clone())
    }
    pub fn shutdown(&self) {
        self.state.shutdown.send_replace(true);
    }
    pub async fn serve(
        self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(std::io::Error::other("listener must be loopback"));
        }
        let _owner = StopOnDrop(self.clone());
        let mut cancelled = self.state.shutdown.subscribe();
        let router = self.router();
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let stopped = *cancelled.borrow();
                if !stopped {
                    tokio::select! {
                        _ = shutdown => {},
                        _ = cancelled.changed() => {},
                    }
                }
                self.shutdown();
            })
            .await
    }
}
pub(super) async fn authenticate(
    State(state): State<Arc<Runtime>>,
    request: Request,
    next: Next,
) -> Response {
    if !state.auth.accepts(request.headers()) {
        return ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    next.run(request).await
}
async fn chat(State(state): State<Arc<Runtime>>, request: Request) -> Response {
    handle(state, Profile::Chat, request)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}
async fn responses(State(state): State<Arc<Runtime>>, request: Request) -> Response {
    handle(state, Profile::Responses, request)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}
pub(super) async fn handle(
    state: Arc<Runtime>,
    profile: Profile,
    request: Request,
) -> Result<Response, ApiError> {
    let mut trace = Trace::new(state.diagnostics.as_ref(), request.headers());
    if *state.shutdown.borrow() {
        return Err(ApiError::shutdown());
    }
    let permit = state
        .permits
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, "gateway_busy"))?;
    admission::headers(request.headers(), state.limits.request_bytes)?;
    let mut shutdown = state.shutdown.subscribe();
    if *shutdown.borrow() {
        return Err(ApiError::shutdown());
    }
    let bytes = tokio::select! {
        biased;
        _=shutdown.changed()=>return Err(ApiError::shutdown()),
        result=tokio::time::timeout(state.limits.body_timeout,admission::collect(request.into_body(),state.limits.request_bytes))=>result.map_err(|_|ApiError::new(StatusCode::REQUEST_TIMEOUT,"request_timeout"))??,
    };
    let (entry, semantic) = admission::prepare(&state, profile, &bytes)?;
    let timeout = state
        .limits
        .exchange_timeout
        .min(Duration::from_millis(entry.endpoint.execution.timeout_ms));
    let deadline = tokio::time::Instant::now() + timeout;
    trace.stage(Stage::Prepare);
    let prepared =
        crate::execution::prepare(&entry.endpoint, &entry.provider, &entry.secret, &semantic)
            .map_err(|_| ApiError::invalid())?;
    trace.stage(Stage::Connect);
    let upstream = tokio::select! {
        biased;
        _=shutdown.changed()=>return Err(ApiError::shutdown()),
        result=state.transport.send(prepared,timeout)=>result.map_err(|e|if e==crate::provider::ErrorClass::Timeout {ApiError::timeout()}else{ApiError::upstream()})?,
    };
    body::respond(
        entry,
        semantic,
        upstream,
        &state.limits,
        deadline,
        shutdown,
        permit,
        trace,
    )
    .await
}
