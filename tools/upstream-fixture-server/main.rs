//! 用于 Native Path 验收的本地上游 fixture server。
//!
//! 它刻意不复用 OpenBridge 的 adapter 或 transport：mock 模式提供确定性 wire fixture，
//! proxy 模式仅用明确环境配置把两个 OpenAI endpoint 转发到真实上游。

use std::{env, io::ErrorKind, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{
        HeaderMap, HeaderName, HeaderValue, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;
use url::Url;

const DEFAULT_LISTEN: &str = "127.0.0.1:4010";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const FIXTURE_RATE_LIMIT_MARKER: &str = "__fixture:rate-limit__";
// `dotenvy::dotenv()` 取决于启动时的 cwd；验收工具必须始终读取它自己的配置文件。
const FIXTURE_ENV_PATH: &str = "tools/upstream-fixture-server/.env";

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;
    load_dotenv()?;

    let config = ServerConfig::from_environment()?;
    let listen = config.listen;
    let mode = config.mode.name();
    let app = router(config)?;
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind upstream fixture server to {listen}"))?;

    info!(%listen, mode, "upstream fixture server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("upstream fixture server stopped unexpectedly")
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))
}

fn load_dotenv() -> Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ENV_PATH);
    // dotenvy 不覆盖已有进程环境，便于 CI 或临时诊断安全地替换单个变量。
    match dotenvy::from_path(&path) {
        Ok(_) => Ok(()),
        Err(dotenvy::Error::Io(error)) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(anyhow::anyhow!(
            "failed to load fixture .env '{}': {error}",
            path.display()
        )),
    }
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl+C handler");
    }
}

struct ServerConfig {
    listen: SocketAddr,
    mode: Mode,
    timeout: Duration,
}

impl ServerConfig {
    fn from_environment() -> Result<Self> {
        let listen = env::var("UPSTREAM_FIXTURE_LISTEN")
            .unwrap_or_else(|_| DEFAULT_LISTEN.to_owned())
            .parse::<SocketAddr>()
            .context("UPSTREAM_FIXTURE_LISTEN must be a socket address")?;
        // proxy 会持有真实 credential；即使误填了公网地址，也不允许把它暴露为共享代理。
        if !listen.ip().is_loopback() {
            anyhow::bail!("UPSTREAM_FIXTURE_LISTEN must use a loopback address");
        }

        let timeout_ms = match env::var("UPSTREAM_FIXTURE_TIMEOUT_MS") {
            Ok(value) => value
                .parse::<u64>()
                .context("UPSTREAM_FIXTURE_TIMEOUT_MS must be an integer")?,
            Err(env::VarError::NotPresent) => DEFAULT_TIMEOUT_MS,
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "UPSTREAM_FIXTURE_TIMEOUT_MS could not be read: {error}"
                ));
            }
        };
        if timeout_ms == 0 {
            anyhow::bail!("UPSTREAM_FIXTURE_TIMEOUT_MS must be greater than zero");
        }

        Ok(Self {
            listen,
            mode: Mode::from_environment()?,
            timeout: Duration::from_millis(timeout_ms),
        })
    }
}

#[derive(Clone)]
enum Mode {
    Mock,
    Proxy(ProxyConfig),
}

impl Mode {
    fn from_environment() -> Result<Self> {
        match env::var("UPSTREAM_FIXTURE_MODE")
            .unwrap_or_else(|_| "mock".to_owned())
            .as_str()
        {
            "mock" => Ok(Self::Mock),
            "proxy" => Ok(Self::Proxy(ProxyConfig::from_environment()?)),
            _ => anyhow::bail!("UPSTREAM_FIXTURE_MODE must be either 'mock' or 'proxy'"),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Proxy(_) => "proxy",
        }
    }
}

#[derive(Clone)]
struct ProxyConfig {
    api_base: Url,
    api_key: SecretString,
    default_model: Option<String>,
}

impl ProxyConfig {
    fn from_environment() -> Result<Self> {
        let api_base = required_environment("UPSTREAM_FIXTURE_API_BASE")?;
        let mut api_base = Url::parse(&api_base)
            .context("UPSTREAM_FIXTURE_API_BASE must be an absolute HTTP(S) URL")?;
        if !matches!(api_base.scheme(), "http" | "https") || api_base.host_str().is_none() {
            anyhow::bail!("UPSTREAM_FIXTURE_API_BASE must be an absolute HTTP(S) URL");
        }
        if api_base.query().is_some() || api_base.fragment().is_some() {
            anyhow::bail!("UPSTREAM_FIXTURE_API_BASE must not contain a query or fragment");
        }
        // `Url::join` 会把不带尾部斜杠的最后一个 path segment 视为文件；统一为目录语义，
        // 使 `.../v1` 和 `.../v1/` 都能正确得到 `.../v1/responses`。
        if !api_base.path().ends_with('/') {
            api_base.set_path(&format!("{}/", api_base.path()));
        }

        Ok(Self {
            api_base,
            api_key: SecretString::from(required_environment("UPSTREAM_FIXTURE_API_KEY")?),
            default_model: optional_environment("UPSTREAM_FIXTURE_MODEL")?,
        })
    }
}

fn required_environment(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!("{name} must be configured"))?;
    if value.is_empty() {
        anyhow::bail!("{name} must not be empty");
    }
    Ok(value)
}

fn optional_environment(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) if value.is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(anyhow::anyhow!("{name} could not be read: {error}")),
    }
}

#[derive(Clone)]
struct FixtureState {
    mode: Mode,
    client: reqwest::Client,
}

fn router(config: ServerConfig) -> Result<Router> {
    let client = reqwest::Client::builder()
        .timeout(config.timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("failed to initialize proxy HTTP client")?;
    let state = FixtureState {
        mode: config.mode,
        client,
    };
    Ok(Router::new()
        .route("/health", get(health))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .with_state(state))
}

async fn health(State(state): State<FixtureState>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct Health<'a> {
        status: &'a str,
        mode: &'a str,
        default_model_configured: bool,
    }

    axum::Json(Health {
        status: "ok",
        mode: state.mode.name(),
        default_model_configured: state.mode.default_model().is_some(),
    })
}

async fn chat_completions(
    State(state): State<FixtureState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    state.handle("chat/completions", headers, body).await
}

async fn responses(State(state): State<FixtureState>, headers: HeaderMap, body: Bytes) -> Response {
    state.handle("responses", headers, body).await
}

impl FixtureState {
    async fn handle(&self, endpoint: &str, headers: HeaderMap, body: Bytes) -> Response {
        match &self.mode {
            Mode::Mock => mock_response(endpoint, &body),
            Mode::Proxy(proxy) => self.proxy_response(endpoint, proxy, headers, body).await,
        }
    }

    async fn proxy_response(
        &self,
        endpoint: &str,
        proxy: &ProxyConfig,
        _headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let url = match proxy.api_base.join(endpoint) {
            Ok(url) => url,
            Err(_) => return error_response(StatusCode::BAD_GATEWAY, "proxy_configuration_error"),
        };
        // 这是显式的 fixture convenience：只补全缺失 model，绝不改写调用方或 OpenBridge
        // 已经选择的模型，以免把测试服务变成第二个路由层。
        let body = apply_default_model(body, proxy.default_model.as_deref());
        let response = match self
            .client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            // 不转发调用方 header，尤其不能把下游 Bearer token 传到真实上游；proxy 的
            // 唯一 credential 来源是本地配置的 SecretString。
            .header(
                AUTHORIZATION,
                format!("Bearer {}", proxy.api_key.expose_secret()),
            )
            .body(body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return error_response(StatusCode::BAD_GATEWAY, "upstream_unavailable"),
        };

        let status = response.status();
        let source_headers = response.headers().clone();
        let mut downstream = Response::new(Body::from_stream(response.bytes_stream()));
        *downstream.status_mut() = status;
        copy_safe_response_headers(&source_headers, downstream.headers_mut());
        downstream
    }
}

impl Mode {
    fn default_model(&self) -> Option<&str> {
        match self {
            Self::Mock => None,
            Self::Proxy(proxy) => proxy.default_model.as_deref(),
        }
    }
}

fn apply_default_model(body: Bytes, default_model: Option<&str>) -> Bytes {
    let Some(default_model) = default_model else {
        return body;
    };
    let Ok(mut request) = serde_json::from_slice::<Value>(&body) else {
        return body;
    };
    let Some(object) = request.as_object_mut() else {
        return body;
    };
    if object.contains_key("model") {
        return body;
    }
    object.insert("model".to_owned(), Value::String(default_model.to_owned()));
    serde_json::to_vec(&request).map_or(body, Bytes::from)
}

fn copy_safe_response_headers(source: &reqwest::header::HeaderMap, target: &mut HeaderMap) {
    // 只保留 SDK 重试、限流和内容解码所需的 header，避免 Cookie、连接控制或未知上游
    // header 意外成为此 loopback tool 的对外契约。
    for name in [
        "content-type",
        "retry-after",
        "x-should-retry",
        "openai-request-id",
        "x-request-id",
        "x-ratelimit-limit-requests",
        "x-ratelimit-remaining-requests",
        "x-ratelimit-reset-requests",
    ] {
        let name = HeaderName::from_static(name);
        if let Some(value) = source.get(&name) {
            target.insert(name, value.clone());
        }
    }
}

fn mock_response(endpoint: &str, body: &[u8]) -> Response {
    let request = match serde_json::from_slice::<Value>(body) {
        Ok(request) => request,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid_json"),
    };
    if contains_string(&request, FIXTURE_RATE_LIMIT_MARKER) {
        // marker 允许 SDK/forwarding 测试在不新增测试专用 endpoint 的情况下稳定触发 429。
        return rate_limit_response();
    }

    let stream = request
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match (endpoint, stream) {
        ("chat/completions", false) => json_response(json!({
            "id": "chatcmpl_fixture",
            "object": "chat.completion",
            "created": 0,
            "model": fixture_model(&request),
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "fixture response", "refusal": null},
                "logprobs": null,
                "finish_reason": "stop"
            }]
        })),
        ("chat/completions", true) => sse_response(format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({
                "id": "chatcmpl_fixture_stream",
                "object": "chat.completion.chunk",
                "created": 0,
                "model": fixture_model(&request),
                "choices": [{"index": 0, "delta": {"role": "assistant", "content": "fixture"}, "finish_reason": null}]
            }),
            json!({
                "id": "chatcmpl_fixture_stream",
                "object": "chat.completion.chunk",
                "created": 0,
                "model": fixture_model(&request),
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
            }),
        )),
        ("responses", false) => json_response(json!({
            "id": "resp_fixture",
            "object": "response",
            "created_at": 0,
            "status": "completed",
            "model": fixture_model(&request),
            "output": [],
            "parallel_tool_calls": true,
            "tool_choice": "auto",
            "tools": []
        })),
        ("responses", true) => sse_response(format!(
            "event: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
            json!({
                "type": "response.output_text.delta",
                "sequence_number": 1,
                "item_id": "msg_fixture",
                "output_index": 0,
                "content_index": 0,
                "delta": "fixture",
                "logprobs": []
            }),
            json!({
                "type": "response.completed",
                "sequence_number": 2,
                "response": {
                    "id": "resp_fixture_stream",
                    "object": "response",
                    "created_at": 0,
                    "status": "completed",
                    "model": fixture_model(&request),
                    "output": [],
                    "parallel_tool_calls": true,
                    "tool_choice": "auto",
                    "tools": []
                }
            }),
        )),
        _ => error_response(StatusCode::NOT_FOUND, "unknown_fixture_endpoint"),
    }
}

fn fixture_model(request: &Value) -> &str {
    request
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("fixture-model")
}

fn contains_string(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => value.contains(needle),
        Value::Array(values) => values.iter().any(|value| contains_string(value, needle)),
        Value::Object(values) => values.values().any(|value| contains_string(value, needle)),
        _ => false,
    }
}

fn json_response(value: Value) -> Response {
    axum::Json(value).into_response()
}

fn rate_limit_response() -> Response {
    let mut response = json_response(json!({
        "error": {
            "message": "fixture rate limited",
            "type": "rate_limit_error",
            "code": "fixture_rate_limited"
        }
    }));
    *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from_static("1"));
    response
}

fn error_response(status: StatusCode, code: &str) -> Response {
    let mut response = json_response(json!({
        "error": {
            "message": "upstream fixture request failed",
            "type": "invalid_request_error",
            "code": code
        }
    }));
    *response.status_mut() = status;
    response
}

fn sse_response(body: String) -> Response {
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::to_bytes,
        http::{Request, header::AUTHORIZATION},
    };
    use tower::ServiceExt;

    use super::*;

    fn mock_app() -> Router {
        router(ServerConfig {
            listen: DEFAULT_LISTEN.parse().unwrap(),
            mode: Mode::Mock,
            timeout: Duration::from_secs(1),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn mock_serves_both_protocols_and_rate_limits_explicit_scenarios() {
        for path in ["/v1/chat/completions", "/v1/responses"] {
            let response = mock_app()
                .oneshot(
                    Request::post(path)
                        .header(CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"model":"fixture-model","input":"hello"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let response = mock_app()
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"model":"fixture-model","input":"__fixture:rate-limit__"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[RETRY_AFTER], "1");
    }

    #[tokio::test]
    async fn proxy_injects_configured_key_and_preserves_streaming_response() {
        // 使用本地上游断言：测试 credential 注入和 SSE body/header 转发，不读取真实 .env。
        let observed = Arc::new(Mutex::new(None));
        let upstream = Router::new()
            .route(
                "/v1/responses",
                post({
                    let observed = observed.clone();
                    move |headers: HeaderMap, body: Bytes| {
                        let observed = observed.clone();
                        async move {
                            *observed.lock().unwrap() = Some((
                                headers[AUTHORIZATION].to_str().unwrap().to_owned(),
                                body,
                            ));
                            let mut response = Response::new(Body::from(
                                "event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
                            ));
                            response.headers_mut().insert(
                                CONTENT_TYPE,
                                HeaderValue::from_static("text/event-stream"),
                            );
                            response.headers_mut().insert(
                                HeaderName::from_static("x-request-id"),
                                HeaderValue::from_static("upstream-fixture"),
                            );
                            response
                        }
                    }
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let app = router(ServerConfig {
            listen: DEFAULT_LISTEN.parse().unwrap(),
            mode: Mode::Proxy(ProxyConfig {
                api_base: Url::parse(&format!("http://{address}/v1/")).unwrap(),
                api_key: SecretString::from("fixture-secret"),
                default_model: Some("fixture-default-model".to_owned()),
            }),
            timeout: Duration::from_secs(1),
        })
        .unwrap();

        let response = app
            .oneshot(
                Request::post("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"model":"fixture-model","input":"hello","stream":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-request-id"], "upstream-fixture");
        assert!(
            std::str::from_utf8(&to_bytes(response.into_body(), 4096).await.unwrap())
                .unwrap()
                .contains("response.completed")
        );
        assert_eq!(
            observed.lock().unwrap().take(),
            Some((
                "Bearer fixture-secret".to_owned(),
                Bytes::from_static(br#"{"model":"fixture-model","input":"hello","stream":true}"#),
            ))
        );
        server.abort();
    }

    #[test]
    fn proxy_default_model_only_fills_a_missing_model() {
        let body = apply_default_model(
            Bytes::from_static(br#"{"input":"hello"}"#),
            Some("fixture-default-model"),
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["model"],
            "fixture-default-model"
        );

        let body = apply_default_model(
            Bytes::from_static(br#"{"model":"caller-model","input":"hello"}"#),
            Some("fixture-default-model"),
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["model"],
            "caller-model"
        );
    }
}
