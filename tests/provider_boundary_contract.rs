use http::{
    StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use openbridge::{
    core::{ApiCapabilities, ApiProtocol, EndpointCapabilities, ResponsesCapabilities},
    provider::{
        AdapterError, CredentialValue, ProviderAdapter, ProviderKind, RetryHint, StreamEventStatus,
        UpstreamErrorKind,
    },
    transport::sse::SseDecoder,
};
use secrecy::SecretString;

#[test]
fn openai_adapter_keeps_safe_and_sensitive_headers_separate() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let credential = CredentialValue::new(
        ProviderKind::OpenAi,
        "openai-primary",
        "version-1",
        SecretString::from("credential-test-value".to_owned()),
    );

    let safe = adapter.prepare_headers().unwrap();
    let sensitive = adapter.prepare_auth_headers(&credential).unwrap();

    assert_eq!(safe.get(CONTENT_TYPE).unwrap(), "application/json");
    assert!(safe.get(AUTHORIZATION).is_none());
    assert!(sensitive.contains(AUTHORIZATION));
    assert_eq!(credential.binding_id(), "openai-primary");
    assert_eq!(credential.secret_version(), "version-1");
    assert!(!format!("{credential:?} {sensitive:?}").contains("credential-test-value"));
}

#[test]
fn response_adapter_classifies_protocol_specific_terminal_events() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let mut decoder = SseDecoder::new(256);
    let responses_event = decoder
        .push(b"event: response.completed\ndata: {}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let chat_event = decoder.push(b"data: [DONE]\n\n").unwrap().remove(0);
    let mut decoder = SseDecoder::new(256);
    let failed_event = decoder
        .push(b"event: response.failed\ndata: {}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let unknown_event = decoder
        .push(b"event: provider.extension\ndata: {\"value\":1}\n\n")
        .unwrap()
        .remove(0);

    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, responses_event)
            .unwrap()
            .status(),
        StreamEventStatus::Completed
    );
    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::ChatCompletions, chat_event)
            .unwrap()
            .status(),
        StreamEventStatus::Completed
    );
    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, failed_event)
            .unwrap()
            .status(),
        StreamEventStatus::Failed
    );
    let decoded_unknown = adapter
        .classify_sse_event(ApiProtocol::Responses, unknown_event)
        .unwrap();
    assert_eq!(decoded_unknown.status(), StreamEventStatus::Continue);
    assert_eq!(decoded_unknown.event().event(), Some("provider.extension"));
}

#[test]
fn error_adapter_returns_safe_coarse_retry_guidance() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);

    let rate_limit = adapter.classify_status(StatusCode::TOO_MANY_REQUESTS);
    let authentication = adapter.classify_status(StatusCode::UNAUTHORIZED);

    assert_eq!(rate_limit.kind(), UpstreamErrorKind::RateLimited);
    assert_eq!(rate_limit.retry_hint(), RetryHint::BeforeFirstEvent);
    assert_eq!(authentication.kind(), UpstreamErrorKind::Authentication);
    assert_eq!(authentication.retry_hint(), RetryHint::Never);
}

#[test]
fn provider_descriptor_is_compile_time_metadata() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let contract = adapter.contract();

    assert_eq!(contract.kind(), ProviderKind::OpenAi);
    assert!(contract.capabilities().chat_completions.enabled);
    assert!(contract.endpoint_profiles().contains(&"public-api"));
}

#[test]
fn longcat_contract_exposes_only_the_verified_native_surface() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::LongCat);
    let contract = adapter.contract();

    assert_eq!(contract.kind(), ProviderKind::LongCat);
    assert!(contract.capabilities().chat_completions.enabled);
    assert!(contract.capabilities().responses.enabled);
    assert!(contract.capabilities().chat_completions.streaming);
    assert!(contract.capabilities().responses.streaming);
    assert!(contract.capabilities().chat_completions.function_calling);
    assert!(contract.capabilities().responses.function_calling);
    assert!(!contract.capabilities().chat_completions.parallel_tool_calls);
    assert!(!contract.capabilities().responses.parallel_tool_calls);
    assert!(!contract.capabilities().chat_completions.image_input);
    assert!(!contract.capabilities().responses.image_input);
    assert!(!contract.capabilities().chat_completions.structured_outputs);
    assert!(!contract.capabilities().responses.structured_outputs);
    assert!(contract.endpoint_profiles().contains(&"longcat-openai"));
}

#[test]
fn capability_adapter_rejects_feature_elevation_before_egress() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let supported = ApiCapabilities {
        chat_completions: EndpointCapabilities {
            enabled: true,
            ..EndpointCapabilities::default()
        },
        ..ApiCapabilities::default()
    };
    let elevated = ApiCapabilities {
        responses: ResponsesCapabilities {
            background: true,
            ..ResponsesCapabilities::default()
        },
        ..ApiCapabilities::default()
    };

    adapter.validate_capabilities(supported).unwrap();
    assert!(matches!(
        adapter.validate_capabilities(elevated).unwrap_err(),
        AdapterError::UnsupportedCapabilities
    ));
}
