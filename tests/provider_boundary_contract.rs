//! Verifies Provider adapter header isolation, capability ceilings, error classification, and SSE terminals.

use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
};
use openbridge::{
    core::{ApiProtocol, ReasoningOutput},
    credential::{CredentialMetadata, CredentialSource, CredentialStoreBuilder},
    provider::{
        CredentialKind, ProviderAdapter, ProviderKind, RetryHint, StreamEventStatus,
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
    // Build terminal wire accepted only by another discriminator or not configured.
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

    // Verify that each Provider accepts only its compile-time terminal discriminator and vocabulary.
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
fn longcat_contract_exposes_only_the_verified_native_surface() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::LongCat);
    let contract = adapter.contract();

    assert_eq!(contract.kind(), ProviderKind::LongCat);
    assert!(contract.capabilities().chat_completions.enabled);
    assert!(contract.capabilities().responses.enabled);
    assert!(contract.capabilities().chat_completions.streaming);
    assert!(contract.capabilities().responses.streaming);
    assert!(
        contract
            .capabilities()
            .chat_completions
            .function_tools
            .is_some()
    );
    assert!(contract.capabilities().responses.function_tools.is_some());
    assert!(
        !contract
            .capabilities()
            .chat_completions
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(
        !contract
            .capabilities()
            .responses
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(
        contract
            .capabilities()
            .chat_completions
            .image_input
            .is_none()
    );
    assert!(contract.capabilities().responses.image_input.is_none());
    assert!(
        contract
            .capabilities()
            .chat_completions
            .structured_outputs
            .is_none()
    );
    assert!(
        contract
            .capabilities()
            .responses
            .structured_outputs
            .is_none()
    );
}

#[test]
fn deepseek_and_mimo_contracts_expose_their_declared_native_protocols() {
    let deepseek = ProviderAdapter::for_kind(ProviderKind::DeepSeek);
    let mimo = ProviderAdapter::for_kind(ProviderKind::MiMo);

    assert!(deepseek.contract().capabilities().chat_completions.enabled);
    assert!(deepseek.contract().capabilities().responses.enabled);
    assert!(mimo.contract().capabilities().chat_completions.enabled);
    assert!(mimo.contract().capabilities().responses.enabled);
}

#[test]
fn deepseek_and_mimo_reasoning_output_types_are_explicit() {
    let deepseek = ProviderAdapter::for_kind(ProviderKind::DeepSeek)
        .contract()
        .capabilities();
    assert_eq!(
        deepseek.chat_completions.reasoning_output,
        ReasoningOutput::PlainText
    );
    assert_eq!(
        deepseek.responses.reasoning_output,
        ReasoningOutput::Unknown
    );

    let mimo = ProviderAdapter::for_kind(ProviderKind::MiMo)
        .contract()
        .capabilities();
    assert_eq!(
        mimo.chat_completions.reasoning_output,
        ReasoningOutput::PlainText
    );
    assert_eq!(mimo.responses.reasoning_output, ReasoningOutput::PlainText);
}

#[test]
fn mimo_contract_declares_tools_images_and_plain_text_reasoning_without_state() {
    let capabilities = ProviderAdapter::for_kind(ProviderKind::MiMo)
        .contract()
        .capabilities();

    let chat = capabilities.chat_completions;
    assert!(
        chat.function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(chat.image_input.is_some());
    assert!(chat.structured_outputs.is_some());
    assert!(!chat.store);
    assert_eq!(chat.reasoning_output, ReasoningOutput::PlainText);

    let responses = capabilities.responses;
    assert!(
        responses
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(responses.image_input.is_some());
    assert!(responses.structured_outputs.is_some());
    assert!(!responses.store);
    assert!(!responses.previous_response_id);
    assert!(!responses.background);
    assert_eq!(responses.reasoning_output, ReasoningOutput::PlainText);
}

#[test]
fn openrouter_contract_exposes_stateless_chat_and_responses_surfaces() {
    let adapter = ProviderAdapter::for_kind(ProviderKind::OpenRouter);
    let contract = adapter.contract();

    assert_eq!(contract.kind(), ProviderKind::OpenRouter);
    assert!(contract.capabilities().chat_completions.enabled);
    assert!(contract.capabilities().chat_completions.streaming);
    assert!(
        contract
            .capabilities()
            .chat_completions
            .function_tools
            .is_some()
    );
    assert!(contract.capabilities().responses.enabled);
    assert!(contract.capabilities().responses.streaming);
    assert!(contract.capabilities().responses.function_tools.is_some());
    assert!(!contract.capabilities().responses.store);
    assert!(!contract.capabilities().responses.previous_response_id);
    assert!(!contract.capabilities().responses.background);
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
