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
        ProviderOperationAdapter::Embeddings(_)
        | ProviderOperationAdapter::ImagesGenerations(_) => {
            panic!("Generation operation selected a non-Generation adapter")
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
fn zhipu_adapter_uses_fixed_protocol_specific_api_paths() {
    for (protocol, body, expected_path) in [
        (
            ApiProtocol::ChatCompletions,
            Bytes::from_static(br#"{"model":"glm","messages":[]}"#),
            "/api/paas/v4/chat/completions",
        ),
        (
            ApiProtocol::Responses,
            Bytes::from_static(br#"{"model":"glm","input":"hello"}"#),
            "/api/v1/responses",
        ),
    ] {
        let request = ApiRequest::new(protocol, body);
        let upstream =
            prepare_request(ProviderKind::ZhipuCn, protocol, &request, "glm-5.3").unwrap();

        assert_eq!(upstream.method(), Method::POST);
        assert_eq!(upstream.relative_uri().to_string(), expected_path);
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(body["model"], "glm-5.3");
    }
}
