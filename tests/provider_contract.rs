//! Verifies Provider adapter relative URIs, actual models, and protocol request shapes.

use std::sync::OnceLock;

use bytes::Bytes;
use http::Method;
use openbridge::{
    config::parse_bootstrap_config,
    core::{ApiProtocol, ApiRequest, OperationKind},
    provider::{
        AdapterError, CredentialKind, GenerationProviderAdapter, PreparedUpstreamRequest,
        ProviderKind, ProviderOperationAdapter,
    },
    providers::build_compiled_registry,
    registry::{RuntimeRegistry, UpstreamApi},
};

fn registry() -> &'static RuntimeRegistry {
    static REGISTRY: OnceLock<RuntimeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .expect("checked-in bootstrap must parse");
        build_compiled_registry(bootstrap).expect("compiled Provider catalog must remain valid")
    })
}

fn generation_route(
    provider: ProviderKind,
    protocol: ApiProtocol,
    upstream_model: &str,
) -> (GenerationProviderAdapter, &'static UpstreamApi) {
    let adapter = match provider
        .definition()
        .operation_adapter(protocol.operation())
        .expect("test Provider declares the Generation operation")
    {
        ProviderOperationAdapter::Generation(adapter) => adapter,
        ProviderOperationAdapter::Embeddings(_) => {
            panic!("Generation operation selected an Embeddings adapter")
        }
    };
    let upstream_api = registry()
        .upstream_target_ids()
        .filter_map(|id| registry().upstream_target(id))
        .filter(|target| target.kind() == provider)
        .flat_map(|target| target.upstream_apis().map(|(_, api)| api))
        .find(|api| {
            api.operation() == protocol.operation() && api.upstream_model() == upstream_model
        })
        .unwrap_or_else(|| panic!("missing {provider:?} {protocol:?} route for {upstream_model}"));
    (adapter, upstream_api)
}

fn prepare_request(
    provider: ProviderKind,
    protocol: ApiProtocol,
    request: &ApiRequest,
    upstream_model: &str,
) -> Result<PreparedUpstreamRequest, AdapterError> {
    let (adapter, upstream_api) = generation_route(provider, protocol, upstream_model);
    adapter.prepare_routed_request(request, upstream_api)
}

#[test]
fn provider_definition_selects_one_closed_operation_before_request_preparation() {
    let operation = ProviderKind::OpenAi
        .definition()
        .operation_adapter(OperationKind::Responses)
        .expect("OpenAI declares Responses");
    let ProviderOperationAdapter::Generation(adapter) = operation else {
        panic!("Responses must select a Generation adapter");
    };
    assert_eq!(adapter.capabilities().operation(), OperationKind::Responses);
    let chat = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"public","messages":[]}"#),
    );
    let (_, responses_api) =
        generation_route(ProviderKind::OpenAi, ApiProtocol::Responses, "gpt-5.6-sol");
    let (_, chat_api) = generation_route(
        ProviderKind::OpenAi,
        ApiProtocol::ChatCompletions,
        "gpt-5.6-sol",
    );

    assert!(matches!(
        adapter.prepare_routed_request(&chat, responses_api),
        Err(AdapterError::UnsupportedProtocol)
    ));
    let responses = ApiRequest::new(
        ApiProtocol::Responses,
        Bytes::from_static(br#"{"model":"public","input":"hello"}"#),
    );
    assert!(matches!(
        adapter.prepare_routed_request(&responses, chat_api),
        Err(AdapterError::UnsupportedProtocol)
    ));
    assert!(
        ProviderKind::ChatGpt
            .definition()
            .operation_adapter(OperationKind::ChatCompletions)
            .is_none()
    );
}

#[test]
fn chatgpt_provider_uses_the_fixed_responses_path_and_oauth_credential() {
    // Verify the fixed credential type and actual adapter behavior at the Provider boundary.
    let contract = ProviderKind::ChatGpt.contract();
    assert_eq!(
        contract.credential_kinds(),
        [CredentialKind::OAuth2BearerAccessToken]
    );
    // Bind Responses to the fixed backend path and reject Chat Completions.
    let (adapter, responses_api) =
        generation_route(ProviderKind::ChatGpt, ApiProtocol::Responses, "gpt-5.6-sol");
    let responses = ApiRequest::new(
        ApiProtocol::Responses,
        Bytes::from_static(br#"{"model":"public","input":"hello","stream":true}"#),
    );
    let upstream = adapter
        .prepare_routed_request(&responses, responses_api)
        .unwrap();
    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/responses");

    let chat = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"public","messages":[]}"#),
    );
    assert!(matches!(
        adapter.prepare_routed_request(&chat, responses_api),
        Err(AdapterError::UnsupportedProtocol)
    ));
}

#[test]
fn native_chat_adapter_builds_only_relative_upstream_request_parts() {
    let (adapter, upstream_api) = generation_route(
        ProviderKind::OpenAi,
        ApiProtocol::ChatCompletions,
        "gpt-5.6-sol",
    );
    let body = Bytes::from_static(br#"{"model":"code-primary","messages":[]}"#);
    let request = ApiRequest::new(ApiProtocol::ChatCompletions, body.clone());

    let upstream = adapter
        .prepare_routed_request(&request, upstream_api)
        .unwrap();

    assert_eq!(upstream.method(), Method::POST);
    assert_eq!(upstream.relative_uri().to_string(), "/v1/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["model"], "gpt-5.6-sol");
    assert_eq!(body["messages"], serde_json::json!([]));
    assert!(upstream.relative_uri().scheme().is_none());
    assert!(upstream.relative_uri().authority().is_none());
}

#[test]
fn openai_compatible_adapters_build_relative_protocol_requests() {
    let cases = [
        (
            ProviderKind::Nvidia,
            ApiProtocol::ChatCompletions,
            "/chat/completions",
            "minimaxai/minimax-m3",
        ),
        (
            ProviderKind::Bailian,
            ApiProtocol::ChatCompletions,
            "/chat/completions",
            "qwen3.7-max",
        ),
        (
            ProviderKind::Bailian,
            ApiProtocol::Responses,
            "/responses",
            "qwen3.7-max",
        ),
        (
            ProviderKind::KimiCn,
            ApiProtocol::ChatCompletions,
            "/v1/chat/completions",
            "kimi-k3",
        ),
        (
            ProviderKind::MiMo,
            ApiProtocol::ChatCompletions,
            "/v1/chat/completions",
            "mimo-v2.5",
        ),
        (
            ProviderKind::MiMo,
            ApiProtocol::Responses,
            "/v1/responses",
            "mimo-v2.5",
        ),
        (
            ProviderKind::OpenRouter,
            ApiProtocol::ChatCompletions,
            "/chat/completions",
            "minimax/minimax-m3",
        ),
        (
            ProviderKind::OpenRouter,
            ApiProtocol::Responses,
            "/responses",
            "minimax/minimax-m3",
        ),
    ];

    // Exercise every supported protocol through the Provider wire adapter.
    for (provider, protocol, expected_path, upstream_model) in cases {
        let body = match protocol {
            ApiProtocol::ChatCompletions => {
                Bytes::from_static(br#"{"model":"public","messages":[]}"#)
            }
            ApiProtocol::Responses => Bytes::from_static(br#"{"model":"public","input":"hello"}"#),
        };
        let request = ApiRequest::new(protocol, body);
        let upstream = prepare_request(provider, protocol, &request, upstream_model).unwrap();
        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(
            upstream.relative_uri().to_string(),
            expected_path,
            "{provider:?}"
        );
        assert!(upstream.relative_uri().scheme().is_none());
        assert!(upstream.relative_uri().authority().is_none());
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["model"], upstream_model, "{provider:?}");
    }
}

#[test]
fn longcat_adapter_directly_encodes_chat_and_responses() {
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
        let upstream =
            prepare_request(ProviderKind::LongCat, protocol, &request, "LongCat-2.0").unwrap();

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
        let upstream = prepare_request(
            ProviderKind::DeepSeek,
            protocol,
            &request,
            "deepseek-v4-flash",
        )
        .unwrap();

        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(upstream.relative_uri().to_string(), expected_path);
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["model"], "deepseek-v4-flash");
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
            ProviderKind::Bailian,
            "qwen3.8-max",
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
            let upstream = prepare_request(
                provider,
                ApiProtocol::ChatCompletions,
                &request,
                upstream_model,
            )
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
    let upstream = prepare_request(
        ProviderKind::Bailian,
        ApiProtocol::ChatCompletions,
        &request,
        "deepseek-v4-pro",
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["reasoning_effort"], "high");
    assert!(body.get("enable_thinking").is_none());
}

#[test]
fn bailian_deepseek_none_uses_boolean_switch_without_collapsing_other_efforts() {
    // Convert only the confirmed off level for both fixed Bailian DeepSeek deployments.
    for upstream_model in ["deepseek-v4-pro", "deepseek-v4-flash-0731"] {
        let request = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"public","messages":[],"reasoning_effort":"none"}"#),
        );
        let upstream = prepare_request(
            ProviderKind::Bailian,
            ApiProtocol::ChatCompletions,
            &request,
            upstream_model,
        )
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert!(body.get("reasoning_effort").is_none(), "{upstream_model}");
        assert_eq!(body["enable_thinking"], false, "{upstream_model}");
    }

    // Preserve the multi-level effort vocabulary instead of collapsing enabled levels to a boolean.
    for upstream_model in ["deepseek-v4-pro", "deepseek-v4-flash-0731"] {
        let request = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"public","messages":[],"reasoning_effort":"high"}"#),
        );
        let upstream = prepare_request(
            ProviderKind::Bailian,
            ApiProtocol::ChatCompletions,
            &request,
            upstream_model,
        )
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["reasoning_effort"], "high", "{upstream_model}");
        assert!(body.get("enable_thinking").is_none(), "{upstream_model}");
    }

    // Leave GLM's confirmed standard off value on its documented effort wire.
    let request = ApiRequest::new(
        ApiProtocol::ChatCompletions,
        Bytes::from_static(br#"{"model":"public","messages":[],"reasoning_effort":"none"}"#),
    );
    let upstream = prepare_request(
        ProviderKind::Bailian,
        ApiProtocol::ChatCompletions,
        &request,
        "glm-5.2",
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
    assert_eq!(body["reasoning_effort"], "none");
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
            ProviderKind::Bailian,
            "qwen3.8-max",
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
            let upstream =
                prepare_request(provider, ApiProtocol::Responses, &request, upstream_model)
                    .unwrap();
            assert_eq!(upstream.relative_uri().to_string(), expected_path);
            let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
            assert_eq!(body["reasoning"]["effort"], *level);
        }
    }
}
