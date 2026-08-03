//! 提供静态 OpenAPI 规范与 Swagger UI 测试页面。
//!
//! 文档资源只描述下游公开的 health、models、Chat 和 Responses endpoint，不读取 registry
//! 中的 Provider、target 或 credential 细节。Swagger UI 页面允许调用方在浏览器内临时填写
//! Bearer token；实际业务 endpoint 仍由既有认证 middleware 保护。

use axum::response::{Html, IntoResponse};
use http::{HeaderMap, HeaderValue, header::CONTENT_TYPE};

const OPENAPI_SPEC: &str = include_str!("../../docs/openapi.yaml");
const SWAGGER_UI_PAGE: &str = include_str!("../../docs/swagger-ui.html");

/// 返回静态 OpenAPI YAML，并保持文档资源不受业务 Bearer 认证影响。
pub(super) async fn openapi_spec() -> impl IntoResponse {
    // 设置明确的 YAML media type，避免浏览器或 Swagger UI 按 HTML 解析规范。
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/yaml; charset=utf-8"),
    );
    (headers, OPENAPI_SPEC)
}

/// 返回 Swagger UI 页面；页面通过同源 URL 加载本地 OpenAPI 规范。
pub(super) async fn swagger_ui() -> Html<&'static str> {
    Html(SWAGGER_UI_PAGE)
}
