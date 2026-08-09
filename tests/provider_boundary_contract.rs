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
fn api_key_adapters_keep_safe_and_sensitive_headers_separate() {
    // Exercise distinct Provider pools through the same credential-isolation boundary.
    for (kind, pool_id, secret) in [
        (ProviderKind::OpenAi, "openai-primary", "openai-test-value"),
        (
            ProviderKind::OpenRouter,
            "openrouter-primary",
            "openrouter-test-value",
        ),
    ] {
        let adapter = ProviderAdapter::for_kind(kind);
        let mut credentials = CredentialStoreBuilder::new();
        credentials
            .insert_upstream_member(
                kind,
                pool_id,
                format!("{pool_id}#1"),
                SecretString::from(secret.to_owned()),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::Programmatic,
                ),
            )
            .unwrap();
        let credentials = credentials.build();
        let credential = credentials
            .upstream_pool(kind, pool_id, CredentialKind::ApiKey)
            .unwrap()
            .remove(0);

        let safe = adapter.prepare_headers().unwrap();
        let sensitive = adapter.prepare_auth_headers(&credential).unwrap();
        assert_eq!(
            safe.get(CONTENT_TYPE).unwrap(),
            "application/json",
            "{kind:?}"
        );
        assert!(safe.get(AUTHORIZATION).is_none(), "{kind:?}");
        assert!(sensitive.contains(AUTHORIZATION), "{kind:?}");
        assert!(
            !format!("{credential:?} {sensitive:?}").contains(secret),
            "{kind:?} leaked its credential"
        );
    }
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
fn provider_capability_ceilings_preserve_verified_feature_differences() {
    // Require the three dual-protocol Providers to share only their verified baseline.
    for kind in [
        ProviderKind::LongCat,
        ProviderKind::MiMo,
        ProviderKind::OpenRouter,
    ] {
        let capabilities = ProviderAdapter::for_kind(kind).contract().capabilities();
        let chat = capabilities.chat_completions;
        let responses = capabilities.responses;
        assert!(chat.enabled && chat.streaming, "{kind:?}");
        assert!(responses.enabled && responses.streaming, "{kind:?}");
        assert!(chat.function_tools.is_some(), "{kind:?}");
        assert!(responses.function_tools.is_some(), "{kind:?}");
        assert!(!chat.store, "{kind:?}");
        assert!(!responses.store, "{kind:?}");
        assert!(!responses.previous_response_id, "{kind:?}");
        assert!(!responses.background, "{kind:?}");
    }

    // Preserve the capability differences that affect admission and bridge eligibility.
    let longcat = ProviderAdapter::for_kind(ProviderKind::LongCat)
        .contract()
        .capabilities();
    assert!(
        !longcat
            .chat_completions
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(
        !longcat
            .responses
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(longcat.chat_completions.image_input.is_none());
    assert!(longcat.responses.image_input.is_none());
    assert!(longcat.chat_completions.structured_outputs.is_none());
    assert!(longcat.responses.structured_outputs.is_none());

    let mimo = ProviderAdapter::for_kind(ProviderKind::MiMo)
        .contract()
        .capabilities();
    assert!(
        mimo.chat_completions
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(
        mimo.responses
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(mimo.chat_completions.image_input.is_some());
    assert!(mimo.responses.image_input.is_some());
    assert!(mimo.chat_completions.structured_outputs.is_some());
    assert!(mimo.responses.structured_outputs.is_some());
    assert_eq!(
        mimo.chat_completions.reasoning_output,
        ReasoningOutput::PlainText
    );
    assert_eq!(mimo.responses.reasoning_output, ReasoningOutput::PlainText);
}
