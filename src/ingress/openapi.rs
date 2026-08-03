//! Provides the static OpenAPI specification and Swagger UI test page.
//!
//! Documentation resources describe only public health, models, Chat, and Responses endpoints and
//! do not read Provider, target, or credential details from the registry. Swagger UI lets callers
//! enter a temporary Bearer token in the browser; existing authentication middleware still protects
//! business endpoints.

use axum::response::{Html, IntoResponse};
use http::{HeaderMap, HeaderValue, header::CONTENT_TYPE};

const OPENAPI_SPEC: &str = include_str!("../../docs/openapi.yaml");
const SWAGGER_UI_PAGE: &str = include_str!("../../docs/swagger-ui.html");

/// Returns static OpenAPI YAML without requiring business Bearer authentication.
pub(super) async fn openapi_spec() -> impl IntoResponse {
    // Set an explicit YAML media type so browsers and Swagger UI do not parse the specification as HTML.
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/yaml; charset=utf-8"),
    );
    (headers, OPENAPI_SPEC)
}

/// Returns the Swagger UI page, which loads the local OpenAPI specification from a same-origin URL.
pub(super) async fn swagger_ui() -> Html<&'static str> {
    Html(SWAGGER_UI_PAGE)
}
