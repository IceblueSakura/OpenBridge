use std::{fmt, time::Duration};

use axum::body::Body;
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use thiserror::Error;
use url::Url;

use crate::{config::ResolvedDeployment, provider::UpstreamRequestParts};

#[derive(Debug, Error)]
pub enum UpstreamError {
    #[error("failed to construct the upstream HTTP client")]
    ClientBuild(#[source] reqwest::Error),
    #[error("upstream request failed")]
    Request(#[source] reqwest::Error),
    #[error("provider adapter produced an invalid relative upstream target")]
    InvalidTarget,
}

pub struct UpstreamClient {
    client: reqwest::Client,
}

impl UpstreamClient {
    pub fn new(
        connect_timeout: Duration,
        pool_idle_timeout: Duration,
        pool_max_idle_per_host: usize,
    ) -> Result<Self, UpstreamError> {
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .pool_idle_timeout(Some(pool_idle_timeout))
            .pool_max_idle_per_host(pool_max_idle_per_host)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(UpstreamError::ClientBuild)?;
        Ok(Self { client })
    }

    pub async fn send(
        &self,
        deployment: &ResolvedDeployment,
        request: UpstreamRequestParts,
        headers: HeaderMap,
    ) -> Result<UpstreamResponse, UpstreamError> {
        if request.relative_uri().scheme().is_some()
            || request.relative_uri().authority().is_some()
            || !request.relative_uri().path().starts_with('/')
        {
            return Err(UpstreamError::InvalidTarget);
        }
        let url = deployment
            .origin()
            .join(&request.relative_uri().to_string())
            .map_err(|_| UpstreamError::InvalidTarget)?;
        self.send_request(UpstreamRequest::new(
            url,
            request.method().clone(),
            headers,
            request.body().clone(),
            deployment.request_timeout(),
        ))
        .await
    }

    async fn send_request(
        &self,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, UpstreamError> {
        let response = self
            .client
            .request(request.method, request.url)
            .headers(request.headers)
            .body(request.body)
            .timeout(request.timeout)
            .send()
            .await
            .map_err(UpstreamError::Request)?;
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

struct UpstreamRequest {
    url: Url,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
    timeout: Duration,
}

impl UpstreamRequest {
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

pub struct UpstreamResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl UpstreamResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

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
    use http::{HeaderMap, Method, StatusCode};
    use tokio::net::TcpListener;
    use url::Url;

    use super::{UpstreamClient, UpstreamRequest};

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
