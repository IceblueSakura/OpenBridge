//! Verifies Provider adapter relative URIs, actual models, and protocol request shapes.

use bytes::Bytes;
use http::Method;
use openbridge::{
    core::{ApiProtocol, ApiRequest},
    provider::{AdapterError, CredentialKind, ProviderAdapter, ProviderKind},
};

#[test]
fn chatgpt_provider_uses_the_fixed_responses_path_and_oauth_credential() {
    // Verify the independent Provider contract exposes only the ChatGPT Responses surface.
    let contract = ProviderKind::ChatGpt.contract();
    assert_eq!(
        contract.credential_kinds(),
        [CredentialKind::OAuth2BearerAccessToken]
    );
    assert!(!contract.capabilities().chat_completions.enabled);
    assert!(contract.capabilities().responses.enabled);
    assert!(contract.capabilities().responses.streaming);
    assert!(!contract.capabilities().embeddings.enabled);

    // Verify the adapter binds Responses to the fixed backend path and rejects Chat Completions.
    let adapter = ProviderAdapter::for_kind(ProviderKind::ChatGpt);
    let responses = ApiRequest::new(
        ApiProtocol::Responses,
        Bytes::from_static(br#"{"model":"public","input":"hello","stream":true}"#),
    );
    let upstream = adapter.prepare_request(&responses, "gpt-5.6-sol").unwrap();
    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/responses");

    let chat = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"public","messages":[]}"#),
    );
    assert!(matches!(
        adapter.prepare_request(&chat, "gpt-5.6-sol"),
        Err(AdapterError::UnsupportedProtocol)
    ));
}

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
fn nvidia_and_bailian_adapters_bind_only_the_confirmed_chat_surface() {
    // Verify both unbound Provider contracts expose only the confirmed basic Chat surface.
    for provider in [ProviderKind::Nvidia, ProviderKind::Bailian] {
        let contract = provider.contract();
        assert_eq!(contract.credential_kinds(), [CredentialKind::ApiKey]);
        assert!(contract.capabilities().chat_completions.enabled);
        assert!(contract.capabilities().chat_completions.streaming);
        assert!(!contract.capabilities().responses.enabled);
        assert!(!contract.capabilities().embeddings.enabled);

        // Build a relative OpenAI-compatible request without selecting any endpoint or credential.
        let adapter = ProviderAdapter::for_kind(provider);
        let request = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"public","messages":[]}"#),
        );
        let upstream = adapter.prepare_request(&request, "upstream-model").unwrap();
        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(upstream.relative_uri().to_string(), "/chat/completions");
        assert!(upstream.relative_uri().scheme().is_none());
        assert!(upstream.relative_uri().authority().is_none());
    }
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

#[test]
fn deepseek_adapter_encodes_chat_and_responses() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::DeepSeek);

    for (protocol, body, expected_path) in [
        (
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"deepseek-public","messages":[]}"#),
            "/chat/completions",
        ),
        (
            ApiProtocol::Responses,
            Bytes::from_static(br#"{"model":"deepseek-public","input":"hello"}"#),
            "/responses",
        ),
    ] {
        let request = ApiRequest::new(protocol, body);
        let upstream = adapter
            .prepare_request(&request, "deepseek-v4-flash")
            .unwrap();

        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(upstream.relative_uri().to_string(), expected_path);
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["model"], "deepseek-v4-flash");
    }
}

#[test]
fn mimo_adapter_encodes_chat_and_responses() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::MiMo);

    for (protocol, body, expected_path) in [
        (
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"mimo-public","messages":[]}"#),
            "/v1/chat/completions",
        ),
        (
            ApiProtocol::Responses,
            Bytes::from_static(br#"{"model":"mimo-public","input":"hello"}"#),
            "/v1/responses",
        ),
    ] {
        let request = ApiRequest::new(protocol, body);
        let upstream = adapter.prepare_request(&request, "mimo-v2.5-pro").unwrap();

        assert_eq!(upstream.relative_uri().to_string(), expected_path);
    }
}

#[test]
fn openrouter_adapter_supports_chat_and_responses() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenRouter);
    let chat = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"deepseek-v4-flash","messages":[]}"#),
    );

    let upstream = adapter
        .prepare_request(&chat, "deepseek/deepseek-v4-flash")
        .unwrap();
    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4-flash");

    let responses = ApiRequest::new(
        ApiProtocol::Responses,
        Bytes::from_static(br#"{"model":"deepseek-v4-flash","input":"hello"}"#),
    );
    let upstream = adapter
        .prepare_request(&responses, "deepseek/deepseek-v4-flash")
        .unwrap();
    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/responses");
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4-flash");
}
