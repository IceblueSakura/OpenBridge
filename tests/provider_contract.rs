//! Verifies Provider adapter relative URIs, actual models, and protocol request shapes.

use bytes::Bytes;
use http::Method;
use openbridge::{
    core::{ApiProtocol, ApiRequest, ReasoningOutput},
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
fn nvidia_bailian_and_kimi_adapters_bind_their_confirmed_api_surfaces() {
    // Verify each fixed API-key Provider contract exposes only its confirmed API surfaces.
    for provider in [
        ProviderKind::Nvidia,
        ProviderKind::Bailian,
        ProviderKind::KimiCn,
    ] {
        let contract = provider.contract();
        assert_eq!(contract.credential_kinds(), [CredentialKind::ApiKey]);
        assert!(contract.capabilities().chat_completions.enabled);
        assert!(contract.capabilities().chat_completions.streaming);
        assert_eq!(
            contract.capabilities().responses.enabled,
            provider == ProviderKind::Bailian
        );
        assert_eq!(
            contract.capabilities().responses.streaming,
            provider == ProviderKind::Bailian
        );
        if provider == ProviderKind::Bailian {
            assert_eq!(
                contract.capabilities().responses.reasoning_output,
                ReasoningOutput::Summary
            );
        }
        assert_eq!(
            contract.capabilities().chat_completions.reasoning_output,
            if provider == ProviderKind::Nvidia {
                ReasoningOutput::Unknown
            } else {
                ReasoningOutput::PlainText
            }
        );
        assert_eq!(
            contract.capabilities().embeddings.enabled,
            provider == ProviderKind::Bailian
        );

        // Build a relative OpenAI-compatible request without selecting any endpoint or credential.
        let adapter = ProviderAdapter::for_kind(provider);
        let request = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"public","messages":[]}"#),
        );
        let upstream = adapter.prepare_request(&request, "upstream-model").unwrap();
        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(
            upstream.relative_uri().to_string(),
            if provider == ProviderKind::KimiCn {
                "/v1/chat/completions"
            } else {
                "/chat/completions"
            }
        );
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
fn reasoning_chat_profiles_emit_provider_official_switches() {
    // Convert every model-level value into the switch shape documented by each Chat API.
    for (provider, upstream_model, expected_field, levels) in [
        (
            ProviderKind::Bailian,
            "qwen3.7-max",
            "enable_thinking",
            &["none", "minimal", "low", "medium", "high", "xhigh", "max"][..],
        ),
        (
            ProviderKind::LongCat,
            "LongCat-2.0",
            "thinking",
            &["none", "high"][..],
        ),
        (
            ProviderKind::MiMo,
            "mimo-v2.5-pro",
            "thinking",
            &["none", "low", "medium", "high"][..],
        ),
    ] {
        for level in levels {
            let enabled = *level != "none";
            let request = ApiRequest::new(
                ApiProtocol::ChatCompletions,
                Bytes::from(format!(
                    r#"{{"model":"public","messages":[],"reasoning_effort":"{level}"}}"#
                )),
            );
            let upstream = ProviderAdapter::for_kind(provider)
                .prepare_request(&request, upstream_model)
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();

            assert!(body.get("reasoning_effort").is_none());
            if expected_field == "enable_thinking" {
                assert_eq!(body[expected_field], enabled);
            } else {
                assert_eq!(
                    body[expected_field]["type"],
                    if enabled { "enabled" } else { "disabled" }
                );
            }
        }
    }

    // Keep Bailian's model-specific toggle from changing DeepSeek or GLM effort semantics.
    let request = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"public","messages":[],"reasoning_effort":"high"}"#),
    );
    let upstream = ProviderAdapter::for_kind(ProviderKind::Bailian)
        .prepare_request(&request, "deepseek-v4-pro")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["reasoning_effort"], "high");
    assert!(body.get("enable_thinking").is_none());
}

#[test]
fn native_responses_preserve_every_documented_reasoning_level() {
    // Preserve exact values on the Native Responses protocols where the upstream documents effort levels.
    for (provider, upstream_model, levels, expected_path) in [
        (
            ProviderKind::Bailian,
            "qwen3.7-plus",
            &["none", "minimal", "low", "medium", "high", "xhigh", "max"][..],
            "/responses",
        ),
        (
            ProviderKind::MiMo,
            "mimo-v2.5",
            &["none", "low", "medium", "high"][..],
            "/v1/responses",
        ),
    ] {
        for level in levels {
            let request = ApiRequest::new(
                ApiProtocol::Responses,
                Bytes::from(format!(
                    r#"{{"model":"public","input":"hello","reasoning":{{"effort":"{level}"}}}}"#
                )),
            );
            let upstream = ProviderAdapter::for_kind(provider)
                .prepare_request(&request, upstream_model)
                .unwrap();
            assert_eq!(upstream.relative_uri().to_string(), expected_path);
            let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
            assert_eq!(body["reasoning"]["effort"], *level);
        }
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
