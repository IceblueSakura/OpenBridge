//! Lowest-owner ingress checks; no real Provider I/O.
use super::http::{authenticate, handle};
use super::*;
use crate::{provider::CredentialBindingId, topology::EndpointId};
use axum::{
    Router,
    http::{HeaderMap, Request as HttpRequest, StatusCode},
    middleware,
};
use std::time::Duration;
use tower::ServiceExt;
pub(super) const KEY: &str = "synthetic-gateway-client-token-0001";
pub(super) fn gateway(limits: Limits) -> Gateway {
    Gateway::new(
        crate::topology::catalog::default_topology().unwrap(),
        vec![Entry {
            model: "deepseek-flash".into(),
            protocol: Profile::Chat,
            endpoint: EndpointId::new("deepseek-chat").unwrap(),
        }],
        BTreeMap::from([(
            CredentialBindingId::new("deepseek-api-key").unwrap(),
            Arc::new(SecretMaterial::new("synthetic-upstream-credential-0001").unwrap()),
        )]),
        SecretMaterial::new(KEY).unwrap(),
        limits,
        None,
    )
    .unwrap()
}
#[tokio::test]
async fn diagnostics_only_record_authenticated_requests_and_never_expose_headers() {
    let path = std::env::temp_dir().join(format!(
        "openbridge-auth-diag-{}-{}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let gate = gateway(Limits::default())
        .with_probe_diagnostics(&path)
        .unwrap();
    for authenticated in [false, true] {
        let mut request = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header(
                "x-openbridge-probe-id",
                "00000000000000000000000000000001:1",
            )
            .header("content-type", "application/json");
        if authenticated {
            request = request.header("authorization", format!("Bearer {KEY}"));
        }
        let response = gate
            .router()
            .oneshot(
                request
                    .body(axum::body::Body::from("invalid json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), if authenticated { 400 } else { 401 });
    }
    gate.flush_probe_diagnostics().await;
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(!text.contains(KEY));
    assert!(!text.contains("invalid json"));
    assert!(text.contains("admission"));
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn authentication_runs_before_any_body_poll() {
    let gate = gateway(Limits::default());
    let app = Router::new()
        .fallback(|| async {
            panic!("unauthenticated handler ran");
            #[allow(unreachable_code)]
            StatusCode::OK
        })
        .layer(middleware::from_fn_with_state(
            gate.state.clone(),
            authenticate,
        ));
    let forbidden = futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<bytes::Bytes, std::io::Error>>> {
            panic!("unauthenticated body was polled")
        },
    );
    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/test")
                .body(axum::body::Body::from_stream(forbidden))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(response.headers()["www-authenticate"], "Bearer");
}
#[tokio::test]
async fn identity_budget_and_scope_are_owned_by_trusted_ingress() {
    let gate = gateway(Limits {
        default_output_tokens: 32,
        max_output_tokens: 64,
        ..Limits::default()
    });
    let (_, request) = admission::prepare(
        &gate.state,
        Profile::Chat,
        br#"{"model":"deepseek-flash","messages":[{"role":"user","content":"hello"}]}"#,
    )
    .unwrap();
    assert_eq!(request.task.semantic.controls().max_output_tokens, Some(32));
    for wire in [br#"{"model":"deepseek-flash","model":"missing","messages":[]}"#.as_slice(),br#"{"model":"deepseek-flash","messages":[{"role":"user","content":"hello"}],"max_completion_tokens":65}"#,br#"{"model":"deepseek-flash","messages":[{"role":"user","content":"hello"}],"upstream_url":"https://untrusted.invalid"}"#] {assert_eq!(admission::prepare(&gate.state,Profile::Chat,wire).err().unwrap().status,400);}
    // Public identity resolves before task contents, not by guessing a Provider.
    assert_eq!(
        admission::prepare(
            &gate.state,
            Profile::Chat,
            br#"{"model":"missing","messages":null}"#
        )
        .err()
        .unwrap()
        .status,
        404
    );
    let mut bad_headers = HeaderMap::new();
    bad_headers.insert("content-type", "application/json".parse().unwrap());
    bad_headers.insert("content-encoding", "gzip".parse().unwrap());
    assert_eq!(
        admission::headers(&bad_headers, 100).unwrap_err().status,
        415
    );
    let chunks = futures_util::stream::iter([
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"123")),
        Ok(bytes::Bytes::from_static(b"456")),
    ]);
    assert_eq!(
        admission::collect(axum::body::Body::from_stream(chunks), 5)
            .await
            .unwrap_err()
            .status,
        413
    );
}
#[tokio::test]
async fn concurrency_and_collection_timeout_fail_before_upstream() {
    let gate = gateway(Limits {
        concurrency: 1,
        body_timeout: Duration::from_millis(20),
        ..Limits::default()
    });
    let held = gate.state.permits.clone().acquire_owned().await.unwrap();
    let request = || {
        HttpRequest::builder()
            .header("content-type", "application/json")
            .body(axum::body::Body::from_stream(
                futures_util::stream::pending::<Result<bytes::Bytes, std::io::Error>>(),
            ))
            .unwrap()
    };
    assert_eq!(
        handle(gate.state.clone(), Profile::Chat, request())
            .await
            .unwrap_err()
            .status,
        429
    );
    drop(held);
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(1),
            handle(gate.state.clone(), Profile::Chat, request())
        )
        .await
        .unwrap()
        .unwrap_err()
        .status,
        408
    );
    assert_eq!(gate.state.permits.available_permits(), 1);
    gate.shutdown();
    assert_eq!(
        handle(gate.state.clone(), Profile::Chat, request())
            .await
            .unwrap_err()
            .status,
        503
    );
}
#[test]
fn entries_cannot_escape_compiled_route_or_credential_ownership() {
    let topo = crate::topology::catalog::default_topology().unwrap();
    let result = Gateway::new(
        topo,
        vec![Entry {
            model: "deepseek-flash".into(),
            protocol: Profile::Chat,
            endpoint: EndpointId::new("xiaomi-chat").unwrap(),
        }],
        BTreeMap::new(),
        SecretMaterial::new(KEY).unwrap(),
        Limits::default(),
        None,
    );
    assert!(matches!(result, Err(StartupError::Binding)));
    let gate = gateway(Limits::default());
    let scope = gate
        .state
        .entries
        .values()
        .next()
        .unwrap()
        .client
        .adaptation
        .scope
        .clone();
    let other = Gateway::new(
        crate::topology::catalog::default_topology().unwrap(),
        vec![Entry {
            model: "deepseek-flash".into(),
            protocol: Profile::Chat,
            endpoint: EndpointId::new("deepseek-chat").unwrap(),
        }],
        BTreeMap::from([(
            CredentialBindingId::new("deepseek-api-key").unwrap(),
            Arc::new(SecretMaterial::new("different-synthetic-upstream-key").unwrap()),
        )]),
        SecretMaterial::new(KEY).unwrap(),
        Limits::default(),
        None,
    )
    .unwrap();
    assert_ne!(
        scope,
        other
            .state
            .entries
            .values()
            .next()
            .unwrap()
            .client
            .adaptation
            .scope
    );
}
