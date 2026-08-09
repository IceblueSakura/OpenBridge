//! Verifies Provider adapter header isolation, capability ceilings, error classification, and SSE terminals.

use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
};
use openbridge::{
    core::{
        ApiProtocol, ImageDetail, ImageDetailPolicy, ImageMediaType, ImageSourceCapabilities,
        JsonSchemaSupport, ReasoningOutput, StructuredOutputProfile, ToolChoiceMode,
    },
    credential::{CredentialMetadata, CredentialSource, CredentialStoreBuilder},
    provider::{
        CredentialKind, ProviderAdapter, ProviderKind, RetryHint, StreamEventStatus,
        UpstreamErrorKind,
    },
    providers::compiled_config,
    registry::UpstreamApiCapabilities,
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
        let chat = capabilities
            .chat_completions
            .expect("a dual-protocol Provider must expose Chat Completions");
        let responses = capabilities
            .responses
            .expect("a dual-protocol Provider must expose Responses");
        assert!(chat.streaming, "{kind:?}");
        assert!(responses.streaming, "{kind:?}");
        assert!(chat.function_tools.is_some(), "{kind:?}");
        assert!(responses.function_tools.is_some(), "{kind:?}");
        assert!(!chat.store, "{kind:?}");
        assert!(!responses.supports_store(), "{kind:?}");
        assert!(!responses.supports_previous_response_id(), "{kind:?}");
        assert!(!responses.background, "{kind:?}");
    }

    // Preserve the capability differences that affect admission and bridge eligibility.
    let longcat = ProviderAdapter::for_kind(ProviderKind::LongCat)
        .contract()
        .capabilities();
    let longcat_chat = longcat
        .chat_completions
        .expect("LongCat must expose Chat Completions");
    let longcat_responses = longcat.responses.expect("LongCat must expose Responses");
    assert!(
        !longcat_chat
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(
        !longcat_responses
            .function_tools
            .is_some_and(|profile| profile.parallel_calls)
    );
    assert!(longcat_chat.image_input.is_none());
    assert!(longcat_responses.image_input.is_none());
    assert!(longcat_chat.structured_outputs.is_none());
    assert!(longcat_responses.structured_outputs.is_none());

    let deepseek = ProviderAdapter::for_kind(ProviderKind::DeepSeek)
        .contract()
        .capabilities();
    let deepseek_chat = deepseek
        .chat_completions
        .expect("DeepSeek must expose Chat Completions");
    let deepseek_responses = deepseek.responses.expect("DeepSeek must expose Responses");
    for profile in [
        deepseek_chat.structured_outputs.unwrap(),
        deepseek_responses.structured_outputs.unwrap(),
    ] {
        assert_eq!(profile, StructuredOutputProfile::JsonObject);
    }

    let mimo = ProviderAdapter::for_kind(ProviderKind::MiMo)
        .contract()
        .capabilities();
    let mimo_chat = mimo
        .chat_completions
        .expect("MiMo must expose Chat Completions");
    let mimo_responses = mimo.responses.expect("MiMo must expose Responses");
    for profile in [
        mimo_chat.function_tools.unwrap(),
        mimo_responses.function_tools.unwrap(),
    ] {
        assert_eq!(profile.choice_modes, &[ToolChoiceMode::Auto]);
        assert!(!profile.parallel_calls);
        assert!(profile.strict_schema);
    }
    assert!(mimo_chat.image_input.is_some());
    assert!(mimo_responses.image_input.is_some());
    assert!(
        mimo_chat.audio.is_some(),
        "MiMo must retain its non-empty multi-task Provider audio ceiling"
    );
    for profile in [
        mimo_chat.structured_outputs.unwrap(),
        mimo_responses.structured_outputs.unwrap(),
    ] {
        assert_eq!(profile, StructuredOutputProfile::JsonObject);
    }
    assert_eq!(mimo_chat.reasoning_output, ReasoningOutput::PlainText);
    assert_eq!(mimo_responses.reasoning_output, ReasoningOutput::PlainText);

    // Keep generic Provider Chat surfaces from inheriting MiMo's independently typed audio set.
    for kind in [
        ProviderKind::OpenAi,
        ProviderKind::Bailian,
        ProviderKind::DeepSeek,
        ProviderKind::LongCat,
        ProviderKind::OpenRouter,
        ProviderKind::Nvidia,
        ProviderKind::KimiCn,
    ] {
        assert!(
            kind.contract()
                .capabilities()
                .chat_completions
                .expect("listed Provider must expose Chat Completions")
                .audio
                .is_none(),
            "{kind:?} must not inherit an unverified Provider audio ceiling"
        );
    }
}

#[test]
fn structured_output_provider_ceilings_and_checked_in_targets_match_the_exact_matrix() {
    let object = StructuredOutputProfile::JsonObject;
    let combined_strict =
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);

    // Lock operation presence and the exact Structured Output ceiling for every Provider family.
    for (kind, expected_chat, expected_responses) in [
        (ProviderKind::Bailian, Some(Some(object)), Some(None)),
        (ProviderKind::ChatGpt, None, Some(Some(combined_strict))),
        (
            ProviderKind::DeepSeek,
            Some(Some(object)),
            Some(Some(object)),
        ),
        (ProviderKind::KimiCn, Some(None), None),
        (ProviderKind::LongCat, Some(None), Some(None)),
        (ProviderKind::MiMo, Some(Some(object)), Some(Some(object))),
        (ProviderKind::Nvidia, Some(None), None),
        (
            ProviderKind::OpenAi,
            Some(Some(combined_strict)),
            Some(Some(combined_strict)),
        ),
        (
            ProviderKind::OpenRouter,
            Some(Some(object)),
            Some(Some(object)),
        ),
    ] {
        let capabilities = kind.contract().capabilities();
        assert_eq!(
            capabilities
                .chat_completions
                .map(|profile| profile.structured_outputs),
            expected_chat,
            "{kind:?} Chat"
        );
        assert_eq!(
            capabilities
                .responses
                .map(|profile| profile.structured_outputs),
            expected_responses,
            "{kind:?} Responses"
        );
    }

    // Lock every checked-in generation Target operation without inheriting its Provider ceiling.
    let config = compiled_config();
    let mut generation_operations = 0;
    for target in &config.upstream_targets {
        for api in &target.upstream_apis {
            let actual = match &api.capabilities {
                UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                    capabilities.structured_outputs
                }
                UpstreamApiCapabilities::Responses(capabilities) => capabilities.structured_outputs,
                UpstreamApiCapabilities::Embeddings(_) => continue,
            };
            generation_operations += 1;
            let key = format!("{}:{:?}", target.id, api.capabilities.operation());
            let expected = match key.as_str() {
                "bailian-deepseek-v4-pro:ChatCompletions"
                | "bailian-deepseek-v4-flash:ChatCompletions"
                | "deepseek-v4-pro:ChatCompletions"
                | "deepseek-v4-flash:ChatCompletions"
                | "deepseek-v4-flash:Responses"
                | "mimo-v2-5-pro:ChatCompletions"
                | "mimo-v2-5-pro:Responses"
                | "mimo-v2-5:ChatCompletions"
                | "mimo-v2-5:Responses"
                | "openrouter-deepseek-v4-flash:ChatCompletions"
                | "openrouter-deepseek-v4-flash:Responses" => Some(object),
                "chatgpt-gpt-5-5:Responses"
                | "chatgpt-gpt-5-6-luna:Responses"
                | "chatgpt-gpt-5-6-terra:Responses"
                | "chatgpt-gpt-5-6-sol:Responses" => Some(combined_strict),
                "bailian-glm-5-2:ChatCompletions"
                | "bailian-qwen3-7-plus:ChatCompletions"
                | "bailian-qwen3-7-plus:Responses"
                | "bailian-qwen3-7-max:ChatCompletions"
                | "bailian-qwen3-7-max:Responses"
                | "bailian-qwen3-8-max:ChatCompletions"
                | "bailian-qwen3-8-max:Responses"
                | "bailian-qwen-image-3-0:ChatCompletions"
                | "bailian-qwen-image-3-0-pro:ChatCompletions"
                | "bailian-qwen3-5-livetranslate-flash-realtime:ChatCompletions"
                | "bailian-qwen3-6-27b:ChatCompletions"
                | "chatgpt-gpt-5-3-codex-spark:Responses"
                | "kimi-cn-kimi-k3:ChatCompletions"
                | "longcat-2:ChatCompletions"
                | "longcat-2:Responses"
                | "mimo-v2-5-asr:ChatCompletions"
                | "mimo-v2-5-tts:ChatCompletions"
                | "mimo-v2-5-tts-voicedesign:ChatCompletions"
                | "mimo-v2-5-tts-voiceclone:ChatCompletions"
                | "nvidia-minimax-m3:ChatCompletions"
                | "openai-main:ChatCompletions"
                | "openai-main:Responses"
                | "openai-gpt-5-6-terra:ChatCompletions"
                | "openai-gpt-5-6-terra:Responses"
                | "openai-gpt-5-6-luna:ChatCompletions"
                | "openai-gpt-5-6-luna:Responses"
                | "openai-gpt-5-5:ChatCompletions"
                | "openai-gpt-5-5:Responses"
                | "openrouter-minimax-m3:ChatCompletions"
                | "openrouter-minimax-m3:Responses" => None,
                unexpected => panic!("unreviewed checked-in generation operation {unexpected}"),
            };
            assert_eq!(actual, expected, "{key}");
        }
    }
    assert_eq!(generation_operations, 45);
}

#[test]
fn image_provider_ceilings_and_checked_in_targets_keep_separate_source_evidence() {
    // Verify each image-capable Provider ceiling owns complete URL and inline payloads.
    for (kind, max_parts, media_types, expected_detail, expected_inline_limits) in [
        (
            ProviderKind::MiMo,
            64,
            &[
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Gif,
                ImageMediaType::Webp,
                ImageMediaType::Bmp,
            ][..],
            ImageDetailPolicy::OmittedOnly { default: None },
            (
                50 * 1024 * 1024,
                38 * 1024 * 1024,
                50 * 1024 * 1024,
                38 * 1024 * 1024,
            ),
        ),
        (
            ProviderKind::OpenAi,
            500,
            &[
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Gif,
                ImageMediaType::Webp,
            ][..],
            ImageDetailPolicy::Explicit(openbridge::core::ImageDetailProfile::new(
                Some(ImageDetail::Auto),
                &[
                    ImageDetail::Auto,
                    ImageDetail::Low,
                    ImageDetail::High,
                    ImageDetail::Original,
                ],
            )),
            (
                20 * 1024 * 1024,
                15 * 1024 * 1024,
                50 * 1024 * 1024,
                38 * 1024 * 1024,
            ),
        ),
    ] {
        let capabilities = ProviderAdapter::for_kind(kind).contract().capabilities();
        for image in [
            capabilities
                .chat_completions
                .expect("image Provider must expose Chat Completions")
                .image_input
                .expect("image Provider Chat ceiling must be typed"),
            capabilities
                .responses
                .expect("image Provider must expose Responses")
                .image_input
                .expect("image Provider Responses ceiling must be typed"),
        ] {
            assert_eq!(image.max_parts(), max_parts, "{kind:?}");
            assert_eq!(image.detail_policy(), expected_detail, "{kind:?}");
            let ImageSourceCapabilities::RemoteUrlAndDataUrl { remote, data } = image.sources()
            else {
                panic!("{kind:?} must expose only the complete URL + data URL source union");
            };
            assert_eq!(remote.max_url_length(), 8_192, "{kind:?}");
            assert_eq!(data.media_types(), media_types, "{kind:?}");
            let limits = data.limits();
            assert_eq!(
                (
                    limits.max_inline_encoded_bytes(),
                    limits.max_inline_decoded_bytes(),
                    limits.max_total_inline_encoded_bytes(),
                    limits.max_total_inline_decoded_bytes(),
                ),
                expected_inline_limits,
                "{kind:?}"
            );
        }
    }

    // Keep Provider ceilings from automatically opening unverified checked-in executable targets.
    let config = compiled_config();
    for target in &config.upstream_targets {
        let expects_image = target.id == "mimo-v2-5";
        let is_scoped_target = target.id.starts_with("mimo-") || target.id.starts_with("openai-");
        if !is_scoped_target {
            continue;
        }
        for api in &target.upstream_apis {
            let image_input = match &api.capabilities {
                UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities.image_input,
                UpstreamApiCapabilities::Responses(capabilities) => capabilities.image_input,
                UpstreamApiCapabilities::Embeddings(_) => continue,
            };
            assert_eq!(
                image_input.is_some(),
                expects_image,
                "{} {:?}",
                target.id,
                api.capabilities.operation()
            );
        }
    }
}

#[test]
fn responses_provider_state_ceilings_preserve_the_verified_family_matrix() {
    // Lock every Responses-capable Provider to its independently typed storage/continuation ceiling.
    for (kind, supports_store, supports_continuation) in [
        (ProviderKind::ChatGpt, false, false),
        (ProviderKind::OpenAi, true, true),
        (ProviderKind::LongCat, false, false),
        (ProviderKind::DeepSeek, false, false),
        (ProviderKind::MiMo, false, false),
        (ProviderKind::OpenRouter, false, false),
        (ProviderKind::Bailian, false, false),
    ] {
        let responses = kind
            .contract()
            .capabilities()
            .responses
            .expect("listed Provider must expose Responses");
        assert_eq!(responses.supports_store(), supports_store, "{kind:?}");
        assert_eq!(
            responses.supports_previous_response_id(),
            supports_continuation,
            "{kind:?}"
        );
    }

    // Keep Chat-only Providers from acquiring a placeholder Responses profile.
    for kind in [ProviderKind::Nvidia, ProviderKind::KimiCn] {
        assert!(
            kind.contract().capabilities().responses.is_none(),
            "{kind:?}"
        );
    }
}
