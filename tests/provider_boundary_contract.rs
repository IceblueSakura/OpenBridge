use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
};
use openbridge::{
    core::{ApiCapabilities, ApiProtocol, EndpointCapabilities, ResponsesCapabilities},
    credential::{CredentialMetadata, CredentialSource, CredentialStoreBuilder},
    provider::{
        AdapterError, CredentialKind, ProviderAdapter, ProviderKind, RetryHint, StreamEventStatus,
        UpstreamErrorKind,
    },
    transport::sse::SseDecoder,
};
use secrecy::SecretString;

#[test]
fn openai_adapter_keeps_safe_and_sensitive_headers_separate() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let mut credentials = CredentialStoreBuilder::new();
    credentials
        .insert_upstream_member(
            ProviderKind::OpenAi,
            "openai-primary",
            "openai-primary#1",
            SecretString::from("credential-test-value".to_owned()),
            CredentialMetadata::upstream(CredentialKind::ApiKey, CredentialSource::Programmatic),
        )
        .unwrap();
    let credentials = credentials.build();
    let credential = credentials
        .upstream_pool(
            ProviderKind::OpenAi,
            "openai-primary",
            CredentialKind::ApiKey,
        )
        .unwrap()
        .remove(0);

    let safe = adapter.prepare_headers().unwrap();
    let sensitive = adapter.prepare_auth_headers(&credential).unwrap();

    assert_eq!(safe.get(CONTENT_TYPE).unwrap(), "application/json");
    assert!(safe.get(AUTHORIZATION).is_none());
    assert!(sensitive.contains(AUTHORIZATION));
    assert_eq!(credential.member_id(), "openai-primary#1");
    assert!(!format!("{credential:?} {sensitive:?}").contains("credential-test-value"));
}

#[test]
fn provider_request_header_hooks_apply_trusted_regular_header_policy() {
    let mut downstream = HeaderMap::new();
    downstream.insert(
        USER_AGENT,
        HeaderValue::from_static("openbridge-contract-client/1.0"),
    );
    downstream.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer downstream-only"),
    );

    for kind in [ProviderKind::OpenAi, ProviderKind::LongCat] {
        let adapter = ProviderAdapter::for_kind(kind);
        let mut safe = adapter.prepare_headers().unwrap();

        adapter
            .apply_request_header_hook(&downstream, &mut safe)
            .unwrap();

        assert_eq!(
            safe.get(USER_AGENT).unwrap(),
            "openbridge-contract-client/1.0"
        );
        assert!(safe.get(AUTHORIZATION).is_none());
    }
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
fn openrouter_responses_classifies_data_only_openai_terminal() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenRouter);
    let mut decoder = SseDecoder::new(256);
    let completed = decoder
        .push(
            b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
        )
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let failed = decoder
        .push(b"data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let unconfigured_done = decoder
        .push(b"data: {\"type\":\"response.done\",\"response\":{\"status\":\"completed\"}}\n\n")
        .unwrap()
        .remove(0);

    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, completed)
            .unwrap()
            .status(),
        StreamEventStatus::Completed
    );
    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, failed)
            .unwrap()
            .status(),
        StreamEventStatus::Failed
    );
    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, unconfigured_done)
            .unwrap()
            .status(),
        StreamEventStatus::Continue
    );
}

#[test]
fn longcat_responses_classifies_data_only_type_terminal() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::LongCat);
    let mut decoder = SseDecoder::new(256);
    let completed = decoder
        .push(
            b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
        )
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let failed = decoder
        .push(b"data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n")
        .unwrap()
        .remove(0);

    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, completed)
            .unwrap()
            .status(),
        StreamEventStatus::Completed
    );
    assert_eq!(
        adapter
            .classify_sse_event(ApiProtocol::Responses, failed)
            .unwrap()
            .status(),
        StreamEventStatus::Failed
    );
}

#[test]
fn openai_event_profiles_fail_closed_on_conflicting_terminal_discriminators() {
    let openai = ProviderAdapter::for_kind(ProviderKind::OpenAi);
    let mut decoder = SseDecoder::new(256);
    let event_completed_data_failed = decoder
        .push(
            b"event: response.completed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n",
        )
        .unwrap()
        .remove(0);
    let longcat = ProviderAdapter::for_kind(ProviderKind::LongCat);
    let mut decoder = SseDecoder::new(256);
    let event_failed_data_completed = decoder
        .push(
            b"event: response.failed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
        )
        .unwrap()
        .remove(0);

    assert_eq!(
        openai
            .classify_sse_event(ApiProtocol::Responses, event_completed_data_failed)
            .unwrap()
            .status(),
        StreamEventStatus::Failed
    );
    assert_eq!(
        longcat
            .classify_sse_event(ApiProtocol::Responses, event_failed_data_completed)
            .unwrap()
            .status(),
        StreamEventStatus::Failed
    );
}

#[test]
fn responses_terminal_discriminators_reject_unconfigured_wire_shapes() {
    // 构造只应由另一 discriminator 接受或未配置的 terminal wire。
    let mut decoder = SseDecoder::new(256);
    let data_type_completed = decoder
        .push(b"data: {\"type\":\"response.completed\"}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let openrouter_event_field_completed = decoder
        .push(b"event: response.completed\ndata: {}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let event_field_completed = decoder
        .push(b"event: response.completed\ndata: {}\n\n")
        .unwrap()
        .remove(0);
    let mut decoder = SseDecoder::new(256);
    let open_responses_done = decoder
        .push(b"data: {\"type\":\"response.done\",\"response\":{\"status\":\"completed\"}}\n\n")
        .unwrap()
        .remove(0);

    // 验证每个 Provider 只接受编译期绑定的 terminal discriminator 与词汇。
    assert_eq!(
        ProviderAdapter::for_kind(ProviderKind::OpenAi)
            .classify_sse_event(ApiProtocol::Responses, data_type_completed)
            .unwrap()
            .status(),
        StreamEventStatus::Continue
    );
    assert_eq!(
        ProviderAdapter::for_kind(ProviderKind::LongCat)
            .classify_sse_event(ApiProtocol::Responses, event_field_completed)
            .unwrap()
            .status(),
        StreamEventStatus::Continue
    );
    assert_eq!(
        ProviderAdapter::for_kind(ProviderKind::LongCat)
            .classify_sse_event(ApiProtocol::Responses, open_responses_done)
            .unwrap()
            .status(),
        StreamEventStatus::Continue
    );
    assert_eq!(
        ProviderAdapter::for_kind(ProviderKind::OpenRouter)
            .classify_sse_event(ApiProtocol::Responses, openrouter_event_field_completed)
            .unwrap()
            .status(),
        StreamEventStatus::Continue
    );
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
fn provider_definition_is_the_single_contract_and_adapter_source() {
    for (kind, endpoint_profile) in [
        (ProviderKind::OpenAi, "public-api"),
        (ProviderKind::LongCat, "longcat-openai"),
        (ProviderKind::DeepSeek, "deepseek-openai"),
        (ProviderKind::MiMo, "mimo-openai"),
        (ProviderKind::OpenRouter, "openrouter-chat"),
    ] {
        let definition = kind.definition();
        let contract = definition.contract();
        let adapter = definition.adapter();

        assert_eq!(definition.kind(), kind);
        assert_eq!(contract.kind(), kind);
        assert!(std::ptr::eq(adapter.contract(), contract));
        assert!(contract.endpoint_profiles().contains(&endpoint_profile));
    }
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
fn deepseek_and_mimo_contracts_expose_only_declared_native_protocols() {
    let deepseek = ProviderAdapter::for_kind(ProviderKind::DeepSeek);
    let mimo = ProviderAdapter::for_kind(ProviderKind::MiMo);

    assert!(deepseek.contract().capabilities().chat_completions.enabled);
    assert!(!deepseek.contract().capabilities().responses.enabled);
    assert!(mimo.contract().capabilities().chat_completions.enabled);
    assert!(mimo.contract().capabilities().responses.enabled);
}

#[test]
fn openrouter_contract_exposes_stateless_chat_and_responses_surfaces() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenRouter);
    let contract = adapter.contract();

    assert_eq!(contract.kind(), ProviderKind::OpenRouter);
    assert!(contract.capabilities().chat_completions.enabled);
    assert!(contract.capabilities().chat_completions.streaming);
    assert!(contract.capabilities().chat_completions.function_calling);
    assert!(contract.capabilities().responses.enabled);
    assert!(contract.capabilities().responses.streaming);
    assert!(contract.capabilities().responses.function_calling);
    assert!(!contract.capabilities().responses.store);
    assert!(!contract.capabilities().responses.previous_response_id);
    assert!(!contract.capabilities().responses.background);
    assert!(contract.endpoint_profiles().contains(&"openrouter-chat"));
    assert!(
        contract
            .endpoint_profiles()
            .contains(&"openrouter-responses")
    );
}

#[test]
fn openrouter_authentication_is_bound_to_its_own_credential() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenRouter);
    let mut credentials = CredentialStoreBuilder::new();
    credentials
        .insert_upstream_member(
            ProviderKind::OpenRouter,
            "openrouter-primary",
            "openrouter-primary#1",
            SecretString::from("openrouter-test-value".to_owned()),
            CredentialMetadata::upstream(CredentialKind::ApiKey, CredentialSource::Programmatic),
        )
        .unwrap();
    let credentials = credentials.build();
    let credential = credentials
        .upstream_pool(
            ProviderKind::OpenRouter,
            "openrouter-primary",
            CredentialKind::ApiKey,
        )
        .unwrap()
        .remove(0);

    let headers = adapter.prepare_auth_headers(&credential).unwrap();

    assert!(headers.contains(AUTHORIZATION));
    assert!(!format!("{credential:?} {headers:?}").contains("openrouter-test-value"));
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
