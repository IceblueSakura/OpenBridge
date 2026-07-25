use http::{
    StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use openbridge::{
    core::{CapabilitySet, Protocol, ProtocolCapabilities, ResponsesCapabilities},
    provider::{
        AuthAdapter, CapabilityAdapter, CredentialLease, ErrorAdapter, EventDisposition,
        HeaderAdapter, ProviderAdapter, ProviderErrorClass, ProviderFailure, ProviderKind,
        ResponseAdapter, RetryHint,
    },
    transport::sse::SseDecoder,
};
use secrecy::SecretString;

#[test]
fn openai_adapter_keeps_safe_and_sensitive_headers_separate() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let lease = CredentialLease::new(
        ProviderKind::OpenAi,
        "openai-primary",
        "version-1",
        SecretString::from("credential-test-value".to_owned()),
    );

    let safe = adapter.build_headers().unwrap();
    let sensitive = adapter.build_auth_headers(&lease).unwrap();

    assert_eq!(safe.get(CONTENT_TYPE).unwrap(), "application/json");
    assert!(safe.get(AUTHORIZATION).is_none());
    assert!(sensitive.contains(AUTHORIZATION));
    assert_eq!(lease.binding_id(), "openai-primary");
    assert_eq!(lease.secret_version(), "version-1");
    assert!(!format!("{lease:?} {sensitive:?}").contains("credential-test-value"));
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
            .decode_event(Protocol::Responses, responses_event)
            .unwrap()
            .disposition(),
        EventDisposition::Completed
    );
    assert_eq!(
        adapter
            .decode_event(Protocol::ChatCompletions, chat_event)
            .unwrap()
            .disposition(),
        EventDisposition::Completed
    );
    assert_eq!(
        adapter
            .decode_event(Protocol::Responses, failed_event)
            .unwrap()
            .disposition(),
        EventDisposition::Failed
    );
    let decoded_unknown = adapter
        .decode_event(Protocol::Responses, unknown_event)
        .unwrap();
    assert_eq!(decoded_unknown.disposition(), EventDisposition::Continue);
    assert_eq!(decoded_unknown.event().event(), Some("provider.extension"));
}

#[test]
fn error_adapter_returns_safe_coarse_retry_guidance() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);

    let rate_limit = adapter.classify_status(StatusCode::TOO_MANY_REQUESTS);
    let authentication = adapter.classify_status(StatusCode::UNAUTHORIZED);

    assert_eq!(rate_limit.class(), ProviderErrorClass::RateLimited);
    assert_eq!(rate_limit.retry_hint(), RetryHint::BeforeFirstEvent);
    assert_eq!(authentication.class(), ProviderErrorClass::Authentication);
    assert_eq!(authentication.retry_hint(), RetryHint::Never);
}

#[test]
fn provider_descriptor_is_compile_time_metadata() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let descriptor = adapter.descriptor();

    assert_eq!(descriptor.kind(), ProviderKind::OpenAi);
    assert!(descriptor.capabilities().chat_completions.enabled);
    assert!(descriptor.endpoint_profiles().contains(&"public-api"));
}

#[test]
fn meituan_descriptor_exposes_only_the_verified_longcat_native_surface() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::Meituan);
    let descriptor = adapter.descriptor();

    assert_eq!(descriptor.kind(), ProviderKind::Meituan);
    assert!(descriptor.capabilities().chat_completions.enabled);
    assert!(descriptor.capabilities().responses.enabled);
    assert!(descriptor.capabilities().chat_completions.streaming);
    assert!(descriptor.capabilities().responses.streaming);
    assert!(descriptor.capabilities().chat_completions.function_calling);
    assert!(descriptor.capabilities().responses.function_calling);
    assert!(
        !descriptor
            .capabilities()
            .chat_completions
            .parallel_tool_calls
    );
    assert!(!descriptor.capabilities().responses.parallel_tool_calls);
    assert!(!descriptor.capabilities().chat_completions.image_input);
    assert!(!descriptor.capabilities().responses.image_input);
    assert!(
        !descriptor
            .capabilities()
            .chat_completions
            .structured_outputs
    );
    assert!(!descriptor.capabilities().responses.structured_outputs);
    assert!(descriptor.endpoint_profiles().contains(&"longcat-openai"));
}

#[test]
fn capability_adapter_rejects_feature_elevation_before_egress() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let supported = CapabilitySet {
        chat_completions: ProtocolCapabilities {
            enabled: true,
            ..ProtocolCapabilities::default()
        },
        ..CapabilitySet::default()
    };
    let elevated = CapabilitySet {
        responses: ResponsesCapabilities {
            background: true,
            ..ResponsesCapabilities::default()
        },
        ..CapabilitySet::default()
    };

    adapter.validate_capabilities(supported).unwrap();
    assert!(matches!(
        adapter.validate_capabilities(elevated).unwrap_err(),
        ProviderFailure::UnsupportedCapabilities
    ));
}
