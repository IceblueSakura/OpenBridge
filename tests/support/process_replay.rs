//! canonical corpus 与真实 loopback HTTP SUT 的最小回放 runner。
//!
//! runner 只在测试进程内创建显式 loopback 地址，生产 registry 仍保持 HTTPS-only；它不读取
//! `.env`、不调用真实 Provider，也不把认证 header 或业务正文写入 observation。

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::post,
};
use futures_util::future::BoxFuture;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use openbridge::{
    ingress::{GatewayState, build_router},
    provider::PreparedUpstreamRequest,
    registry::UpstreamTarget,
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

/// 一次 loopback replay 的安全摘要。
pub struct ReplayObservation {
    /// OpenBridge 最终返回的 HTTP status。
    pub status: StatusCode,
    /// OpenBridge 最终保留的安全 `Retry-After`。
    pub retry_after: Option<String>,
    /// Mock upstream 实际收到的 attempt 数量。
    pub upstream_attempts: usize,
    /// 每次上游 JSON request 是否匹配 canonical expectation。
    pub upstream_request_matches: Vec<bool>,
    /// 下游 JSON body 是否匹配 canonical expectation。
    pub downstream_body_matches: bool,
}

#[derive(Clone)]
struct MockUpstreamState {
    expected_request: Value,
    response_body: Bytes,
    observations: Arc<Mutex<Vec<bool>>>,
}

struct LoopbackReplayTransport {
    base_url: String,
    client: reqwest::Client,
}

impl UpstreamTransport for LoopbackReplayTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        // 固定测试 runner 创建的 loopback origin，并保留 adapter 生成的相对 path。
        let url = format!("{}{}", self.base_url, request.relative_uri());
        let method = request.method().clone();
        let body = request.body().clone();
        let client = self.client.clone();

        // 经过真实 HTTP client/socket 发出请求，并保留 streaming body 边界。
        Box::pin(async move {
            let response = client
                .request(method, url)
                .headers(headers)
                .body(body)
                .send()
                .await
                .map_err(TransportError::Request)?;
            let status = response.status();
            let headers = response.headers().clone();
            Ok(UpstreamResponse::new(
                status,
                headers,
                Body::from_stream(response.bytes_stream()),
            ))
        })
    }
}

/// 回放一个 canonical Responses 429 case，并返回不含正文与凭证的验证摘要。
pub async fn replay_rate_limit_case(case_id: &str) -> ReplayObservation {
    // 从固定 corpus 目录加载四份 canonical wire artifact。
    assert_eq!(case_id, "responses_native.rate_limit.non_stream");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/native")
        .join(case_id);
    let client_request = read_json(root.join("client-request.json"));
    let expected_upstream = read_json(root.join("expected-upstream-request.json"));
    let upstream_body = std::fs::read(root.join("upstream-response.json"))
        .expect("canonical upstream response must be readable");
    let expected_client = read_json(root.join("expected-client-response.json"));

    // 启动记录实际 HTTP request 的 mock upstream listener。
    let observations = Arc::new(Mutex::new(Vec::new()));
    let upstream_state = MockUpstreamState {
        expected_request: expected_upstream,
        response_body: Bytes::from(upstream_body),
        observations: observations.clone(),
    };
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock upstream must bind loopback");
    let upstream_address = upstream_listener
        .local_addr()
        .expect("mock upstream address must exist");
    let upstream_task = spawn_server(
        upstream_listener,
        Router::new()
            .route("/v1/responses", post(mock_rate_limit))
            .with_state(upstream_state),
    );

    // 用生产 Router 与 adapter 启动 SUT，但仅在测试 transport 内替换为 loopback origin。
    let registry = openbridge::registry::build_registry(
        super::bootstrap(super::BOOTSTRAP),
        super::definition("process-replay", "public-model", "upstream-model"),
    )
    .expect("replay registry must be valid");
    let transport = LoopbackReplayTransport {
        base_url: format!("http://{upstream_address}"),
        client: reqwest::Client::new(),
    };
    let (users, credentials) = super::users_and_credentials(
        "downstream-token-0000000000000000",
        &registry,
        "synthetic-upstream-token",
    );
    let state = GatewayState::new(Arc::new(registry), Arc::new(transport), users, credentials);
    let gateway_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("OpenBridge replay listener must bind loopback");
    let gateway_address = gateway_listener
        .local_addr()
        .expect("OpenBridge replay address must exist");
    let gateway_task = spawn_server(gateway_listener, build_router(state));

    // 经真实下游 HTTP client 发送 canonical request，并读取最终安全响应。
    let response = reqwest::Client::new()
        .post(format!("http://{gateway_address}/v1/responses"))
        .header(CONTENT_TYPE, "application/json")
        .bearer_auth("downstream-token-0000000000000000")
        .body(serde_json::to_vec(&client_request).expect("canonical request must encode"))
        .send()
        .await
        .expect("OpenBridge replay request must complete");
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .bytes()
        .await
        .expect("OpenBridge replay response body must be readable");
    let body: Value =
        serde_json::from_slice(&body).expect("OpenBridge replay response must be JSON");

    // 停止两个 listener，并只返回不含敏感内容的比较摘要。
    gateway_task.abort();
    upstream_task.abort();
    let upstream_request_matches = observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    ReplayObservation {
        status,
        retry_after,
        upstream_attempts: upstream_request_matches.len(),
        upstream_request_matches,
        downstream_body_matches: body == expected_client,
    }
}

async fn mock_rate_limit(
    State(state): State<MockUpstreamState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 比较请求 JSON，但 observation 只保留布尔结果而不回显 body 或认证 header。
    let matches =
        serde_json::from_slice::<Value>(&body).is_ok_and(|value| value == state.expected_request);
    state
        .observations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(matches);

    // 为每个 attempt 返回同一 canonical 429 与 Retry-After。
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header(CONTENT_TYPE, "application/json")
        .header("retry-after", "1")
        .body(Body::from(state.response_body))
        .expect("canonical mock response must build")
}

fn read_json(path: std::path::PathBuf) -> Value {
    // 读取并解析 canonical JSON artifact，错误直接绑定到测试 fixture。
    let bytes = std::fs::read(path).expect("canonical JSON artifact must be readable");
    serde_json::from_slice(&bytes).expect("canonical JSON artifact must be valid")
}

fn spawn_server(listener: TcpListener, router: Router) -> JoinHandle<()> {
    // 在独立任务中运行 loopback server，由测试结束路径显式 abort。
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("loopback replay server must remain valid");
    })
}
