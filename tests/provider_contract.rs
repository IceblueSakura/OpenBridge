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

    let upstream = adapter.encode_request(&request).unwrap();

    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/v1/chat/completions");
    assert_eq!(upstream.body(), &body);
    assert!(upstream.relative_uri().scheme().is_none());
    assert!(upstream.relative_uri().authority().is_none());
}
