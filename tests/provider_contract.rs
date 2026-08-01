use bytes::Bytes;
use http::Method;
use openbridge::{
    core::{ApiProtocol, ApiRequest},
    provider::{ProviderAdapter, ProviderKind},
};

#[test]
fn native_chat_adapter_builds_only_relative_upstream_request_parts() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let body = Bytes::from_static(br#"{"model":"code-primary","messages":[]}"#);
    let request = ApiRequest::new(ApiProtocol::ChatCompletions, body.clone());

    let upstream = adapter.prepare_request(&request, "upstream-model").unwrap();

    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/v1/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["messages"], serde_json::json!([]));
    assert!(upstream.relative_uri().scheme().is_none());
    assert!(upstream.relative_uri().authority().is_none());
}

#[test]
fn longcat_adapter_directly_encodes_chat_and_responses() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::LongCat);

    for (protocol, body, expected_path) in [
        (
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"LongCat-2.0","messages":[]}"#),
            "/openai/v1/chat/completions",
        ),
        (
            ApiProtocol::Responses,
            Bytes::from_static(br#"{"model":"LongCat-2.0","input":"hello"}"#),
            "/openai/v1/responses",
        ),
    ] {
        let request = ApiRequest::new(protocol, body);
        let upstream = adapter.prepare_request(&request, "LongCat-2.0").unwrap();

        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(upstream.relative_uri().to_string(), expected_path);
        assert!(upstream.relative_uri().scheme().is_none());
        assert!(upstream.relative_uri().authority().is_none());
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["model"], "LongCat-2.0");
    }
}
