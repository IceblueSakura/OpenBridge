//! 共享上游 HTTP transport。
//!
//! client 复用连接池、禁用重定向，并只接受 adapter 生成的相对 URI 与配置验证过的 endpoint base。
//! response body 以流形式交给 ingress；不预读或缓冲 token，因此下游取消会 drop 上游流。

use std::{fmt, time::Duration};

use axum::body::Body;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Method, StatusCode, Uri};
use thiserror::Error;
use url::Url;

use crate::{provider::PreparedUpstreamRequest, registry::UpstreamTarget};

/// 上游 transport 在构造 client、发送请求或读取超时边界时报告的错误。
#[derive(Debug, Error)]
pub enum TransportError {
    /// reqwest client 无法按 bootstrap 策略创建。
    #[error("failed to construct the upstream HTTP client")]
    ClientBuild(#[source] reqwest::Error),
    /// 上游请求在发送或接收过程中失败。
    #[error("upstream request failed")]
    Request(#[source] reqwest::Error),
    /// 上游请求超过 target 的超时时间。
    #[error("upstream request timed out")]
    Timeout,
    /// adapter 生成了带 authority、scheme 或非法 path 的 URI。
    #[error("provider adapter produced an invalid relative upstream target")]
    InvalidTarget,
}

/// ingress 与真实 HTTP client/测试 transport 之间的最小发送契约。
pub trait UpstreamTransport: Send + Sync {
    /// 将 adapter 请求发送到指定 target，并保留流式响应 body。
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>>;
}

/// 复用连接池的共享上游 HTTP client。
pub struct UpstreamClient {
    client: reqwest::Client,
}

impl UpstreamClient {
    /// 按 bootstrap 策略创建一个禁用重定向的上游 client。
    pub fn new(
        connect_timeout: Duration,
        pool_idle_timeout: Duration,
        pool_max_idle_per_host: usize,
    ) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .pool_idle_timeout(Some(pool_idle_timeout))
            .pool_max_idle_per_host(pool_max_idle_per_host)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(TransportError::ClientBuild)?;
        Ok(Self { client })
    }

    /// 将相对 URI 与 Upstream Target endpoint base 合成为唯一 egress URL。
    ///
    /// 这里再次拒绝 scheme/authority/path 不合法的 URI，即使 adapter 是编译期代码；配置
    /// allowlist 与该检查共同避免未来 adapter 修改意外扩大 SSRF 出站面。
    pub async fn send(
        &self,
        target: &UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> Result<UpstreamResponse, TransportError> {
        // 将 adapter 相对 URI 与已校验 endpoint base 合成为受信 URL。
        let url = resolve_upstream_url(target.endpoint_base(), request.relative_uri())?;
        // 交给共享 client 发送，并保留流式响应 body。
        self.send_request(UpstreamRequest::new(
            url,
            request.method().clone(),
            headers,
            request.body().clone(),
            target.request_timeout(),
        ))
        .await
    }

    /// 用共享 client 发送已绑定 URL 的请求，并保留响应 stream body。
    async fn send_request(
        &self,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, TransportError> {
        // 应用 target 超时和连接池 client，发送不跟随重定向的请求。
        let response = self
            .client
            .request(request.method, request.url)
            .headers(request.headers)
            .body(request.body)
            .timeout(request.timeout)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    TransportError::Timeout
                } else {
                    TransportError::Request(error)
                }
            })?;
        // 复制 status/header，并将 response stream 交给上层消费。
        let status = response.status();
        let headers = response.headers().clone();
        let body = Body::from_stream(response.bytes_stream());
        Ok(UpstreamResponse {
            status,
            headers,
            body,
        })
    }
}

/// 校验 adapter 相对 URI，并将其安全拼接到已验证的 endpoint base。
fn resolve_upstream_url(endpoint_base: &Url, relative_uri: &Uri) -> Result<Url, TransportError> {
    if relative_uri.scheme().is_some()
        || relative_uri.authority().is_some()
        || !relative_uri.path().starts_with('/')
    {
        return Err(TransportError::InvalidTarget);
    }

    let prefix = endpoint_base.path().trim_end_matches('/');
    let mut url = endpoint_base.clone();
    url.set_path(&format!("{}{}", prefix, relative_uri.path()));
    url.set_query(relative_uri.query());
    Ok(url)
}

impl UpstreamTransport for UpstreamClient {
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        Box::pin(async move { UpstreamClient::send(self, target, request, headers).await })
    }
}

struct UpstreamRequest {
    url: Url,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
    timeout: Duration,
}

impl UpstreamRequest {
    /// 创建已绑定 URL、method、header、body 和 timeout 的内部请求值对象。
    fn new(url: Url, method: Method, headers: HeaderMap, body: Bytes, timeout: Duration) -> Self {
        Self {
            url,
            method,
            headers,
            body,
            timeout,
        }
    }
}

impl fmt::Debug for UpstreamRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamRequest")
            .field("method", &self.method)
            .field("origin", &self.url.origin().ascii_serialization())
            .field("headers", &"[REDACTED]")
            .field("body", &"[OMITTED]")
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// 上游响应的状态、前置头和流式 body。
pub struct UpstreamResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl UpstreamResponse {
    /// 创建一个用于测试或 transport 边界的响应。
    pub fn new(status: StatusCode, headers: HeaderMap, body: Body) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// 返回 HTTP status。
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// 返回上游响应头。
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// 消费响应并取得其流式 body。
    pub fn into_body(self) -> Body {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        net::SocketAddr,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use axum::{
        Router,
        body::to_bytes,
        extract::{ConnectInfo, State},
        routing::get,
    };
    use bytes::Bytes;
    use http::{HeaderMap, Method, StatusCode, Uri};
    use tokio::net::TcpListener;
    use url::Url;

    use super::{UpstreamClient, UpstreamRequest, resolve_upstream_url};

    #[test]
    fn endpoint_base_prefix_is_preserved_when_building_adapter_target() {
        let endpoint_base = Url::parse("https://provider.example/openai/").unwrap();
        let target = Uri::from_static("/v1/responses");

        let url = resolve_upstream_url(&endpoint_base, &target).unwrap();

        assert_eq!(url.as_str(), "https://provider.example/openai/v1/responses");
    }

    #[test]
    fn endpoint_base_rejects_adapter_targets_with_an_authority() {
        let endpoint_base = Url::parse("https://provider.example/openai/").unwrap();
        let target = "https://attacker.invalid/v1/responses"
            .parse::<Uri>()
            .unwrap();

        assert!(resolve_upstream_url(&endpoint_base, &target).is_err());
    }

    #[tokio::test]
    async fn shared_client_reuses_one_tcp_connection_for_sequential_requests() {
        let peers = Arc::new(Mutex::new(HashSet::<SocketAddr>::new()));
        let app = Router::new()
            .route(
                "/probe",
                get(
                    |ConnectInfo(peer): ConnectInfo<SocketAddr>,
                     State(peers): State<Arc<Mutex<HashSet<SocketAddr>>>>| async move {
                        peers.lock().unwrap().insert(peer);
                        "ok"
                    },
                ),
            )
            .with_state(peers.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let client =
            UpstreamClient::new(Duration::from_secs(2), Duration::from_secs(30), 4).unwrap();
        let url = Url::parse(&format!("http://{address}/probe")).unwrap();

        for _ in 0..2 {
            let request = UpstreamRequest::new(
                url.clone(),
                Method::GET,
                HeaderMap::new(),
                Bytes::new(),
                Duration::from_secs(2),
            );
            let response = client.send_request(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                to_bytes(response.into_body(), 16).await.unwrap(),
                Bytes::from_static(b"ok")
            );
        }

        assert_eq!(peers.lock().unwrap().len(), 1);
        server.abort();
    }
}
