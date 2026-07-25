use bytes::Bytes;
use http::Method;
use openbridge::{
    core::{Protocol, ValidatedRequest},
    provider::{ProviderAdapter, ProviderKind, RequestAdapter},
};

#[test]
fn native_chat_adapter_builds_only_relative_upstream_request_parts() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let body = Bytes::from_static(br#"{"model":"code-primary","messages":[]}"#);
    let request = ValidatedRequest::new(Protocol::ChatCompletions, body.clone());

    let upstream = adapter.encode_request(&request, "upstream-model").unwrap();

    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/v1/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["messages"], serde_json::json!([]));
    assert!(upstream.relative_uri().scheme().is_none());
    assert!(upstream.relative_uri().authority().is_none());
}
