//! Shared upstream HTTP transport.
//!
//! The client reuses a connection pool, disables redirects, and accepts only adapter-generated
//! relative URIs with configuration-validated endpoint bases. Response bodies are streamed to
//! ingress without pre-reading or buffering tokens, so downstream cancellation drops the upstream stream.

use std::{fmt, time::Duration};

use axum::body::Body;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Method, StatusCode, Uri};
use url::Url;

use crate::{provider::PreparedUpstreamRequest, registry::UpstreamTarget};

mod error;

pub use error::TransportError;

/// Minimal send contract between ingress and the real HTTP client/test transport.
pub trait UpstreamTransport: Send + Sync {
    /// Sends an adapter request to the target and preserves the streaming response body.
    fn send<'a>(
        &'a self,
        target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>>;
}

/// Shared upstream HTTP client with a reused connection pool.
pub struct UpstreamClient {
    client: reqwest::Client,
}

impl UpstreamClient {
    /// Creates an upstream client with redirects disabled according to bootstrap policy.
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

    /// Combines a relative URI with the Upstream Target endpoint base into the single egress URL.
    ///
    /// Rejects invalid URI scheme/authority/path again even though the adapter is compile-time code.
    /// The configuration allowlist and this check together prevent future adapter changes from
    /// expanding the SSRF egress surface.
    pub async fn send(
        &self,
        target: &UpstreamTarget,
        request: PreparedUpstreamRequest,
        headers: HeaderMap,
    ) -> Result<UpstreamResponse, TransportError> {
        // Combine the adapter relative URI with the validated endpoint base into a trusted URL.
        let url = resolve_upstream_url(target.endpoint_base(), request.relative_uri())?;
        // Send through the shared client and preserve the streaming response body.
        self.send_request(UpstreamRequest::new(
            url,
            request.method().clone(),
            headers,
            request.body().clone(),
            target.request_timeout(),
        ))
        .await
    }

    /// Sends a request with a bound URL through the shared client and preserves the response stream body.
    async fn send_request(
        &self,
        request: UpstreamRequest,
    ) -> Result<UpstreamResponse, TransportError> {
        // Apply the target timeout and pooled client, sending without following redirects.
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
        // Copy status/headers and pass the response stream to the caller.
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

/// Validates an adapter relative URI and safely joins it to a validated endpoint base.
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
    /// Creates an internal request value with bound URL, method, headers, body, and timeout.
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

/// Upstream response status, initial headers, and streaming body.
pub struct UpstreamResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl UpstreamResponse {
    /// Creates a response for tests or transport boundaries.
    pub fn new(status: StatusCode, headers: HeaderMap, body: Body) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Returns the HTTP status.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns upstream response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Consumes the response and returns its streaming body.
    pub fn into_body(self) -> Body {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        future::pending,
        net::SocketAddr,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::{ConnectInfo, Request, State},
        response::Response,
        routing::{any, get},
    };
    use bytes::Bytes;
    use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header::LOCATION};
    use tokio::net::TcpListener;
    use url::Url;

    use super::{TransportError, UpstreamClient, UpstreamRequest, resolve_upstream_url};

    type ObservedRequest = Arc<Mutex<Option<(Method, String, Bytes)>>>;

    async fn capture_request(
        State(observed): State<ObservedRequest>,
        request: Request,
    ) -> Response {
        // Capture method and one synthetic header before consuming the request body.
        let method = request.method().clone();
        let marker = request
            .headers()
            .get("x-openbridge-test")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let body = to_bytes(request.into_body(), 1024).await.unwrap();
        *observed.lock().unwrap() = Some((method, marker, body));

        // Return non-default status, metadata, and body for transport preservation checks.
        Response::builder()
            .status(StatusCode::CREATED)
            .header("x-upstream-test", "preserved")
            .body(Body::from("response-body"))
            .unwrap()
    }

    async fn never_respond() -> &'static str {
        pending::<()>().await;
        "unreachable"
    }

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

    #[tokio::test]
    async fn send_request_preserves_method_headers_body_and_response_metadata() {
        // Start a loopback endpoint that records the complete request boundary.
        let observed = Arc::new(Mutex::new(None));
        let app = Router::new()
            .route("/capture", any(capture_request))
            .with_state(observed.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // Send through the real pooled client with explicit method, header, body, and timeout.
        let client =
            UpstreamClient::new(Duration::from_secs(2), Duration::from_secs(30), 4).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openbridge-test",
            HeaderValue::from_static("request-marker"),
        );
        let request = UpstreamRequest::new(
            Url::parse(&format!("http://{address}/capture")).unwrap(),
            Method::POST,
            headers,
            Bytes::from_static(b"request-body"),
            Duration::from_secs(2),
        );
        let response = client.send_request(request).await.unwrap();

        // Verify both directions without exposing or buffering any production credential.
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["x-upstream-test"], "preserved");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            Bytes::from_static(b"response-body")
        );
        let observed = observed.lock().unwrap().take().unwrap();
        assert_eq!(observed.0, Method::POST);
        assert_eq!(observed.1, "request-marker");
        assert_eq!(observed.2, Bytes::from_static(b"request-body"));
        server.abort();
    }

    #[tokio::test]
    async fn send_request_does_not_follow_redirects() {
        // Count destination requests so a followed redirect cannot pass unnoticed.
        let destinations = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route(
                "/redirect",
                get(|| async {
                    Response::builder()
                        .status(StatusCode::FOUND)
                        .header(LOCATION, "/destination")
                        .body(Body::empty())
                        .unwrap()
                }),
            )
            .route(
                "/destination",
                get(|State(destinations): State<Arc<AtomicUsize>>| async move {
                    destinations.fetch_add(1, Ordering::Relaxed);
                    "followed"
                }),
            )
            .with_state(destinations.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // Preserve the redirect response instead of expanding trusted egress with a second request.
        let client =
            UpstreamClient::new(Duration::from_secs(2), Duration::from_secs(30), 4).unwrap();
        let request = UpstreamRequest::new(
            Url::parse(&format!("http://{address}/redirect")).unwrap(),
            Method::GET,
            HeaderMap::new(),
            Bytes::new(),
            Duration::from_secs(2),
        );
        let response = client.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers()[LOCATION], "/destination");
        assert_eq!(destinations.load(Ordering::Relaxed), 0);
        server.abort();
    }

    #[tokio::test]
    async fn send_request_classifies_the_target_timeout() {
        // Keep the loopback handler pending so only the request timeout can complete the call.
        let app = Router::new().route("/hang", get(never_respond));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client =
            UpstreamClient::new(Duration::from_secs(2), Duration::from_secs(30), 4).unwrap();
        let request = UpstreamRequest::new(
            Url::parse(&format!("http://{address}/hang")).unwrap(),
            Method::GET,
            HeaderMap::new(),
            Bytes::new(),
            Duration::from_millis(50),
        );

        let error = client
            .send_request(request)
            .await
            .err()
            .expect("the pending endpoint must reach the target timeout");

        assert!(matches!(error, TransportError::Timeout));
        server.abort();
    }

    #[tokio::test]
    async fn send_request_classifies_non_timeout_client_failures() {
        // Build a request whose unsupported scheme fails inside the HTTP client without network I/O.
        let client =
            UpstreamClient::new(Duration::from_secs(2), Duration::from_secs(30), 4).unwrap();
        let request = UpstreamRequest::new(
            Url::parse("ftp://127.0.0.1/resource").unwrap(),
            Method::GET,
            HeaderMap::new(),
            Bytes::new(),
            Duration::from_secs(2),
        );

        // Keep non-timeout client failures distinct from the target timeout classification.
        let error = client
            .send_request(request)
            .await
            .err()
            .expect("reqwest must reject a non-HTTP URL scheme");

        assert!(matches!(error, TransportError::Request(_)));
    }
}
