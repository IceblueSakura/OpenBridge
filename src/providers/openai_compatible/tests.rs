//! Focused tests for the OpenAI-compatible facade and cross-policy assembly.

use bytes::Bytes;
use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{CONTENT_TYPE, USER_AGENT},
};
use serde_json::json;

use crate::{
    core::{
        ApiProtocol, ApiRequest, ProviderChatCompletionsCapabilities, ProviderResponsesCapabilities,
    },
    provider::{AdapterError, ProviderKind, ProviderRequestHeaders, SafeHeaders},
};

use super::{OpenAiCompatibleAdapter, OpenAiCompatibleApiSurface, OpenAiCompatibleEndpoint};

fn transform_headers(
    downstream: &HeaderMap,
    upstream: &mut SafeHeaders,
) -> Result<(), AdapterError> {
    // Forward the selected downstream metadata for the synthetic Provider policy.
    let source = HeaderName::from_static("x-source-name");
    let target = HeaderName::from_static("x-target-name");
    if let Some(value) = downstream.get(source) {
        upstream.insert(target, value.clone())?;
    }
    if let Some(value) = downstream.get(USER_AGENT) {
        upstream.insert(USER_AGENT, value.clone())?;
    }

    // Remove the shared JSON header to exercise hook deletion independently.
    upstream.remove(CONTENT_TYPE);
    Ok(())
}

fn remove_chat_output_limit(
    _protocol: ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    document.remove("max_completion_tokens");
    Ok(())
}

#[test]
fn provider_hook_and_fixed_headers_apply_in_deterministic_order() {
    const FIXED_HEADERS: &[crate::provider::StaticRequestHeader] =
        &[crate::provider::StaticRequestHeader::new(
            "x-provider-fixed",
            "fixed-value",
        )];
    const REQUEST_HEADERS: ProviderRequestHeaders = ProviderRequestHeaders::new()
        .with_user_agent("fixed-provider-client/1.0")
        .with_headers(FIXED_HEADERS);

    // Configure one synthetic adapter and conflicting downstream identity.
    let adapter = OpenAiCompatibleAdapter::new(
        ProviderKind::OpenAi,
        OpenAiCompatibleApiSurface::new(
            Some(OpenAiCompatibleEndpoint::new(
                "/chat",
                ProviderChatCompletionsCapabilities::default(),
            )),
            Some(OpenAiCompatibleEndpoint::new(
                "/responses",
                ProviderResponsesCapabilities::default(),
            )),
            None,
        ),
        "/models",
        transform_headers,
    )
    .with_request_headers(REQUEST_HEADERS);
    let mut downstream = HeaderMap::new();
    downstream.insert(
        HeaderName::from_static("x-source-name"),
        HeaderValue::from_static("transformed-value"),
    );
    downstream.insert(
        USER_AGENT,
        HeaderValue::from_static("downstream-client/1.0"),
    );

    // Run the hook before the fixed profile, matching production request assembly.
    let mut upstream = adapter.prepare_headers().unwrap();
    adapter
        .apply_request_header_hook(&downstream, &mut upstream)
        .unwrap();
    adapter
        .apply_configured_request_headers(&mut upstream)
        .unwrap();

    // Verify deletion, fixed-header precedence, and downstream transformation together.
    assert!(upstream.get(CONTENT_TYPE).is_none());
    assert_eq!(
        upstream.get(USER_AGENT).unwrap(),
        "fixed-provider-client/1.0"
    );
    assert_eq!(
        upstream
            .get(HeaderName::from_static("x-provider-fixed"))
            .unwrap(),
        "fixed-value"
    );
    assert_eq!(
        upstream
            .get(HeaderName::from_static("x-target-name"))
            .unwrap(),
        "transformed-value"
    );
}

#[test]
fn probe_body_hook_cannot_silently_remove_an_output_limit() {
    let adapter = OpenAiCompatibleAdapter::new(
        ProviderKind::OpenAi,
        OpenAiCompatibleApiSurface::new(
            Some(OpenAiCompatibleEndpoint::new(
                "/chat",
                ProviderChatCompletionsCapabilities::default(),
            )),
            None,
            None,
        ),
        "/models",
        transform_headers,
    )
    .with_request_body_hook(remove_chat_output_limit);
    let bounded = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from(
                json!({"model": "candidate", "messages": [], "stream": true, "max_completion_tokens": 16})
                    .to_string(),
            ),
        );

    assert!(matches!(
        adapter.prepare_probe_request(
            ApiProtocol::ChatCompletions,
            "/chat",
            &bounded,
            "candidate",
            true,
        ),
        Err(AdapterError::InvalidRequestBody)
    ));
}

#[test]
fn provider_model_list_profiles_bind_paths_and_response_envelopes() {
    // Keep the generic OpenAI-compatible path and data envelope as the default profile.
    let openai = crate::provider::ProviderAdapter::for_kind(ProviderKind::OpenAi);
    assert_eq!(
        openai
            .prepare_model_list_request()
            .unwrap()
            .relative_uri()
            .to_string(),
        "/v1/models"
    );
    assert_eq!(
        openai.model_list_ids(&json!({"data": [{"id": "gpt-5.6-sol"}]})),
        Some(vec!["gpt-5.6-sol".to_owned()])
    );

    // Bind LongCat's OpenAI-compatible model-list endpoint under its /openai/v1 prefix.
    let longcat = crate::provider::ProviderAdapter::for_kind(ProviderKind::LongCat);
    assert_eq!(
        longcat
            .prepare_model_list_request()
            .unwrap()
            .relative_uri()
            .to_string(),
        "/openai/v1/models"
    );

    // Bind ChatGPT's client-version query and parse its Codex manifest envelope.
    let chatgpt = crate::provider::ProviderAdapter::for_kind(ProviderKind::ChatGpt);
    assert_eq!(
        chatgpt
            .prepare_model_list_request()
            .unwrap()
            .relative_uri()
            .to_string(),
        "/models?client_version=0.146.0"
    );
    assert_eq!(
        chatgpt.model_list_ids(&json!({
            "models": [
                {"slug": "gpt-5.6-sol"},
                {"slug": "gpt-5.6-luna"}
            ]
        })),
        Some(vec!["gpt-5.6-sol".to_owned(), "gpt-5.6-luna".to_owned()])
    );
    assert!(
        chatgpt
            .model_list_ids(&json!({"data": [{"id": "gpt-5.6-sol"}]}))
            .is_none()
    );
}
