//! Verifies Public Model capability preflight and ordered Route planning.

mod support;

use openbridge::{
    core::{
        ALL_TOOL_CHOICE_MODES, ApiProtocol, ExecutableResponsesState, FunctionToolCapabilities,
        ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageMediaType,
        ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonSchemaSupport, OperationKind, ReasoningOutput, RemoteImageInputLimits,
        ResponsesAffinity, StorageSupport, StructuredOutputProfile,
    },
    pipeline::RequestPlanningError,
    provider::{CredentialKind, ProviderAdapter, ProviderKind},
    providers::compiled_config,
    registry::{
        CredentialPoolConfig, IgnorableGenerationParameter, ModelContextLength,
        NonStreamingConversion, ProviderInstanceConfig, ReasoningLevel, ReasoningLevelMapping,
        ReasoningLevelPolicy, ReasoningProfile, RegistryConfig, RegistryError, RouteConfig,
        RouteMode, RuntimeRegistry, UpstreamApiCapabilities, UpstreamStreamingPolicy,
        build_registry,
    },
};
use serde_json::{Value, json};

const TINY_IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[ImageMediaType::Png];
const JPEG_IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[ImageMediaType::Jpeg];
const PNG_AND_JPEG_IMAGE_MEDIA_TYPES: &[ImageMediaType] =
    &[ImageMediaType::Png, ImageMediaType::Jpeg];
const LOW_IMAGE_DETAIL: &[ImageDetail] = &[ImageDetail::Low];
const HIGH_IMAGE_DETAIL: &[ImageDetail] = &[ImageDetail::High];
const LOW_AND_HIGH_IMAGE_DETAILS: &[ImageDetail] = &[ImageDetail::Low, ImageDetail::High];
const TINY_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    2,
    ImageSourceCapabilities::DataUrl(InlineImageInputProfile::new(
        TINY_IMAGE_MEDIA_TYPES,
        InlineImageInputLimits::new(4, 3, 4, 3),
    )),
    ImageDetailPolicy::OmittedOnly {
        default: Some(ImageDetail::Auto),
    },
);

fn base_definition() -> RegistryConfig {
    support::definition("routing-test", "public-model", "upstream-model")
}

fn prompt_cache_definition() -> RegistryConfig {
    // Rebind the synthetic fixture to the probed LongCat dual-protocol Provider contract.
    let mut definition = base_definition();
    definition.provider_instances[0].kind = ProviderKind::LongCat;
    definition.provider_instances[0].base_url = "https://api.longcat.chat".to_owned();
    definition.credential_pools[0].provider = ProviderKind::LongCat;
    let provider = ProviderAdapter::for_kind(ProviderKind::LongCat);
    let capabilities = provider.contract().capabilities();
    let target = &mut definition.upstream_targets[0];
    target.provider_model = ProviderKind::LongCat.routing_model_id(&target.canonical_model);
    target.upstream_apis[0].capabilities = UpstreamApiCapabilities::ChatCompletions(
        capabilities
            .chat_completions
            .expect("LongCat must expose Chat Completions")
            .to_executable(None),
    );
    target.upstream_apis[1].capabilities = UpstreamApiCapabilities::Responses(
        capabilities
            .responses
            .expect("LongCat must expose Responses")
            .to_executable(ExecutableResponsesState::new(
                StorageSupport::Unsupported,
                ResponsesAffinity::TargetBound,
            )),
    );
    definition
}

fn omitted_auto_detail() -> ImageDetailPolicy {
    ImageDetailPolicy::OmittedOnly {
        default: Some(ImageDetail::Auto),
    }
}

fn explicit_auto_detail(allowed: &'static [ImageDetail]) -> ImageDetailPolicy {
    ImageDetailPolicy::Explicit(ImageDetailProfile::new(Some(ImageDetail::Auto), allowed))
}

fn remote_image_input(
    max_parts: u32,
    max_url_length: u32,
    detail: ImageDetailPolicy,
) -> ImageInputCapabilities {
    ImageInputCapabilities::new(
        max_parts,
        ImageSourceCapabilities::RemoteUrl(RemoteImageInputLimits::new(max_url_length)),
        detail,
    )
}

fn data_image_input(
    max_parts: u32,
    media_types: &'static [ImageMediaType],
    limits: InlineImageInputLimits,
    detail: ImageDetailPolicy,
) -> ImageInputCapabilities {
    ImageInputCapabilities::new(
        max_parts,
        ImageSourceCapabilities::DataUrl(InlineImageInputProfile::new(media_types, limits)),
        detail,
    )
}

fn remote_and_data_image_input(
    max_parts: u32,
    max_url_length: u32,
    media_types: &'static [ImageMediaType],
    limits: InlineImageInputLimits,
    detail: ImageDetailPolicy,
) -> ImageInputCapabilities {
    ImageInputCapabilities::new(
        max_parts,
        ImageSourceCapabilities::RemoteUrlAndDataUrl {
            remote: RemoteImageInputLimits::new(max_url_length),
            data: InlineImageInputProfile::new(media_types, limits),
        },
        detail,
    )
}

fn set_image_input(
    definition: &mut RegistryConfig,
    target_index: usize,
    operation: OperationKind,
    image_input: ImageInputCapabilities,
) {
    set_target_image_input(
        &mut definition.upstream_targets[target_index],
        operation,
        image_input,
    );
}

fn add_chat_image_candidate(
    definition: &mut RegistryConfig,
    target_id: &str,
    route_id: &str,
    image_input: ImageInputCapabilities,
) {
    // Clone and narrow one executable Chat target to the requested image profile.
    let mut target = definition.upstream_targets[0].clone();
    target.id = target_id.to_owned();
    set_target_image_input(&mut target, OperationKind::ChatCompletions, image_input);

    // Attach the target and its Native Route to the fixed Public Model candidate order.
    definition.upstream_targets.push(target);
    definition.routes.push(RouteConfig {
        id: route_id.to_owned(),
        upstream_target: target_id.to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::ChatCompletions,
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes.push(route_id.to_owned());
}

fn add_mimo_chat_image_candidate(
    definition: &mut RegistryConfig,
    target_id: &str,
    route_id: &str,
    image_input: ImageInputCapabilities,
) {
    // Register one synthetic MiMo deployment and credential boundary for the second Route.
    definition.provider_instances.push(ProviderInstanceConfig {
        id: "mimo-test".to_owned(),
        kind: ProviderKind::MiMo,
        base_url: "https://api.xiaomimimo.com".to_owned(),
    });
    definition.credential_pools.push(CredentialPoolConfig {
        id: "mimo-test".to_owned(),
        provider: ProviderKind::MiMo,
        kind: CredentialKind::ApiKey,
    });

    // Rebind a Chat-only candidate to the MiMo Provider and its independently checked ceiling.
    let mut target = definition.upstream_targets[0].clone();
    target.id = target_id.to_owned();
    target.provider_instance = "mimo-test".to_owned();
    target.provider_model = ProviderKind::MiMo.routing_model_id(&target.canonical_model);
    target.credential_pool = "mimo-test".to_owned();
    target
        .upstream_apis
        .retain(|api| api.capabilities.operation() == OperationKind::ChatCompletions);
    target.upstream_apis[0].capabilities = UpstreamApiCapabilities::ChatCompletions(
        ProviderAdapter::for_kind(ProviderKind::MiMo)
            .contract()
            .capabilities()
            .chat_completions
            .expect("MiMo must expose Chat Completions")
            .to_executable(None),
    );
    set_target_image_input(&mut target, OperationKind::ChatCompletions, image_input);

    // Attach the rebound target and its Native Route to the fixed Public Model candidate order.
    definition.upstream_targets.push(target);
    definition.routes.push(RouteConfig {
        id: route_id.to_owned(),
        upstream_target: target_id.to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::ChatCompletions,
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes.push(route_id.to_owned());
}

fn set_target_image_input(
    target: &mut openbridge::registry::UpstreamTargetConfig,
    operation: OperationKind,
    image_input: ImageInputCapabilities,
) {
    // Resolve the exact generation operation that owns the image profile.
    let upstream_api = target
        .upstream_apis
        .iter_mut()
        .find(|api| api.capabilities.operation() == operation)
        .expect("test target must expose the selected generation operation");

    // Apply the profile without allowing an Embeddings operation to absorb generation state.
    match &mut upstream_api.capabilities {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => {
            capabilities.image_input = Some(image_input);
        }
        UpstreamApiCapabilities::Responses(capabilities) => {
            capabilities.image_input = Some(image_input);
        }
        UpstreamApiCapabilities::Embeddings(_) => panic!("image input requires generation"),
    }
}

fn public_chat_image(definition: RegistryConfig) -> Value {
    // Compile the fixed interface before reading its downstream-safe JSON projection.
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    info["interfaces"]["chat_completions"]["multimodal_input"]["image"].clone()
}

fn all_function_tools() -> FunctionToolCapabilities {
    FunctionToolCapabilities {
        choice_modes: ALL_TOOL_CHOICE_MODES,
        parallel_calls: false,
        strict_schema: false,
    }
}

const COMMON_TOOL_CHOICE_MODES: &[openbridge::core::ToolChoiceMode] = &[
    openbridge::core::ToolChoiceMode::None,
    openbridge::core::ToolChoiceMode::Auto,
];

#[test]
fn prompt_cache_key_is_forwarded_to_every_native_candidate_and_empty_include_is_removed() {
    let mut definition = prompt_cache_definition();

    // Add a second fixed Native Responses candidate with the same exact forwarding contract.
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "longcat-fallback".to_owned();
    definition.upstream_targets.push(fallback);
    definition.routes.push(RouteConfig {
        id: "fallback-responses".to_owned(),
        upstream_target: "longcat-fallback".to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: OperationKind::Responses,
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec![
        "public-responses".to_owned(),
        "fallback-responses".to_owned(),
    ];
    let registry = build_test_registry(definition);

    // Publish forwarding as a request parameter without claiming any unsupported include value.
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    let interface = &info["interfaces"]["responses"];
    assert!(
        interface["supported_parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "prompt_cache_key")
    );
    assert_eq!(interface["response_includes"], json!([]));
    assert!(interface.get("prompt_caching").is_none());

    // Build each fallback independently from the same canonical request option.
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "include": [],
        "prompt_cache_key": "cache-test"
    }))
    .unwrap();
    let plan = support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap();
    assert_eq!(plan.candidates().len(), 2);
    for candidate in plan.candidates() {
        let upstream: Value = serde_json::from_slice(candidate.request().body()).unwrap();
        assert_eq!(upstream["prompt_cache_key"], "cache-test");
        assert!(upstream.get("include").is_none());
    }

    // Keep an unsupported known output projection behind the typed Public Model preflight gate.
    let unsupported = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "include": ["reasoning.encrypted_content"]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, unsupported.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn planning_preserves_canonical_reasoning_levels_for_every_candidate() {
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::XHigh]);
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
        upstream_api.model_rules.reasoning_level_mappings = vec![ReasoningLevelMapping {
            downstream: ReasoningLevel::XHigh,
            upstream: "max".to_owned(),
        }];
    }
    let mut unmapped = definition.upstream_targets[0].clone();
    unmapped.id = "openai-unmapped".to_owned();
    for upstream_api in &mut unmapped.upstream_apis {
        upstream_api.model_rules.reasoning_level_mappings.clear();
    }
    definition.upstream_targets.push(unmapped);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "unmapped-chat".to_owned(),
        upstream_target: "openai-unmapped".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "unmapped-responses".to_owned(),
        upstream_target: "openai-unmapped".to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: ApiProtocol::Responses.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0].routes = vec![
        "public-chat".to_owned(),
        "unmapped-chat".to_owned(),
        "public-responses".to_owned(),
        "unmapped-responses".to_owned(),
    ];
    let registry = build_test_registry(definition);

    // Verify that planning preserves the canonical Responses level for every candidate.
    let request = json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "xhigh"}
    });
    let prepared = support::prepare(
        &registry,
        ApiProtocol::Responses,
        serde_json::to_vec(&request).unwrap().into(),
    )
    .unwrap();
    let mapped: Value = serde_json::from_slice(prepared.request().body()).unwrap();
    assert_eq!(mapped["reasoning"]["effort"], "xhigh");
    assert_eq!(prepared.candidates().len(), 2);
    let unmapped: Value =
        serde_json::from_slice(prepared.candidates()[1].request().body()).unwrap();
    assert_eq!(unmapped["reasoning"]["effort"], "xhigh");

    // Responses does not accept Chat's top-level reasoning_effort alias.
    let nonstandard = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning_effort": "xhigh"
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, nonstandard.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    // Verify that planning also preserves the canonical Chat level for every candidate.
    let chat = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "reasoning_effort": "xhigh"
    }))
    .unwrap();
    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, chat.into()).unwrap();
    let mapped: Value = serde_json::from_slice(prepared.candidates()[0].request().body()).unwrap();
    let unmapped: Value =
        serde_json::from_slice(prepared.candidates()[1].request().body()).unwrap();
    assert_eq!(mapped["reasoning_effort"], "xhigh");
    assert_eq!(unmapped["reasoning_effort"], "xhigh");

    // Provider-private target values must not expand the public level set available downstream.
    let unsupported = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "max"}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, unsupported.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));
}

#[test]
fn clamp_positive_floor_normalizes_sparse_reasoning_before_candidate_expansion() {
    // Configure one sparse Public Model contract and a second fallback target for both protocols.
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::Medium, ReasoningLevel::High]);
    definition.public_models[0].reasoning_level_policy = ReasoningLevelPolicy::ClampPositiveFloor;
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "openai-fallback".to_owned();
    definition.upstream_targets.push(fallback);
    for (id, operation) in [
        ("fallback-chat", OperationKind::ChatCompletions),
        ("fallback-responses", OperationKind::Responses),
    ] {
        definition.routes.push(RouteConfig {
            id: id.to_owned(),
            upstream_target: "openai-fallback".to_owned(),
            upstream_operation: operation,
            downstream_operation: operation,
            mode: RouteMode::Native,
        });
        definition.public_models[0].routes.push(id.to_owned());
    }
    let registry = build_test_registry(definition);

    // Normalize every positive edge case once and give every fallback candidate the same body.
    for (protocol, request, pointer, expected) in [
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "reasoning_effort": "minimal"}),
            "/reasoning_effort",
            "medium",
        ),
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "reasoning_effort": "low"}),
            "/reasoning_effort",
            "medium",
        ),
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "reasoning_effort": "xhigh"}),
            "/reasoning_effort",
            "high",
        ),
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "reasoning_effort": "max"}),
            "/reasoning_effort",
            "high",
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "reasoning": {"effort": "minimal"}}),
            "/reasoning/effort",
            "medium",
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "reasoning": {"effort": "low"}}),
            "/reasoning/effort",
            "medium",
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "reasoning": {"effort": "xhigh"}}),
            "/reasoning/effort",
            "high",
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "reasoning": {"effort": "max"}}),
            "/reasoning/effort",
            "high",
        ),
    ] {
        let prepared = support::prepare(
            &registry,
            protocol,
            serde_json::to_vec(&request).unwrap().into(),
        )
        .unwrap();
        assert_eq!(prepared.candidates().len(), 2, "{protocol:?}");
        for candidate in prepared.candidates() {
            let upstream: Value = serde_json::from_slice(candidate.request().body()).unwrap();
            let upstream_pointer = match candidate.request().protocol() {
                ApiProtocol::ChatCompletions => "/reasoning_effort",
                ApiProtocol::Responses => "/reasoning/effort",
            };
            assert_eq!(
                upstream.pointer(upstream_pointer).and_then(Value::as_str),
                Some(expected),
                "{protocol:?} {}",
                request
                    .pointer(pointer)
                    .and_then(Value::as_str)
                    .expect("test request must contain an effort")
            );
        }
    }

    // Keep `none`, unknown values, and an unspecified Responses object outside positive clamping.
    for (protocol, request) in [
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "reasoning_effort": "none"}),
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "reasoning": {"effort": "none"}}),
        ),
    ] {
        assert!(matches!(
            support::prepare(
                &registry,
                protocol,
                serde_json::to_vec(&request).unwrap().into()
            )
            .unwrap_err(),
            RequestPlanningError::ReasoningLevelUnsupported
        ));
    }
    let unknown = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "reasoning_effort": "future"
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, unknown.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));
    let unspecified = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {}
    }))
    .unwrap();
    let prepared = support::prepare(&registry, ApiProtocol::Responses, unspecified.into()).unwrap();
    let upstream: Value = serde_json::from_slice(prepared.request().body()).unwrap();
    assert_eq!(upstream["reasoning"], json!({}));

    // Publish executable levels separately from the complete positive input vocabulary.
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    for interface in ["chat_completions", "responses"] {
        let reasoning = &info["interfaces"][interface]["reasoning"];
        assert_eq!(
            reasoning["levels"],
            json!(["medium", "high"]),
            "{interface}"
        );
        assert_eq!(
            reasoning["accepted_levels"],
            json!(["minimal", "low", "medium", "high", "xhigh", "max"]),
            "{interface}"
        );
        assert_eq!(
            reasoning["input_policy"], "clamp_positive_floor",
            "{interface}"
        );
    }
}

#[test]
fn native_routes_accept_declared_none_and_max_reasoning_levels() {
    // A new canonical level must preserve stable wire round trips.
    assert_eq!(
        ReasoningLevel::from_wire("none"),
        Some(ReasoningLevel::None)
    );
    assert_eq!(ReasoningLevel::None.as_wire(), "none");
    assert_eq!(ReasoningLevel::from_wire("max"), Some(ReasoningLevel::Max));
    assert_eq!(ReasoningLevel::Max.as_wire(), "max");

    // After explicit model declaration, Chat and Responses requests preserve their respective levels.
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::None, ReasoningLevel::Max]);
    let registry = build_test_registry(definition);
    for (protocol, request, pointer, expected) in [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "reasoning_effort": "max"
            }),
            "/reasoning_effort",
            "max",
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "hello",
                "reasoning": {"effort": "none"}
            }),
            "/reasoning/effort",
            "none",
        ),
    ] {
        let prepared = support::prepare(
            &registry,
            protocol,
            serde_json::to_vec(&request).unwrap().into(),
        )
        .unwrap();
        let upstream: Value = serde_json::from_slice(prepared.request().body()).unwrap();
        assert_eq!(
            upstream.pointer(pointer).and_then(Value::as_str),
            Some(expected)
        );
    }
}

#[test]
fn request_preflight_rejects_conflicting_reasoning_configuration_sources() {
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).reasoning =
        ReasoningProfile::supported([ReasoningLevel::Low, ReasoningLevel::High]);
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "reasoning": {"effort": "low"},
        "reasoning_effort": "high"
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::InvalidReasoningConfiguration
    ));
}

#[test]
fn bridged_reasoning_requires_a_readable_upstream_output_capability() {
    fn definition(reasoning_output: ReasoningOutput) -> RegistryConfig {
        let mut definition = base_definition();
        support::generation_profile_mut(&mut definition.models[0]).reasoning =
            ReasoningProfile::supported([ReasoningLevel::High]);

        definition.credential_pools[0].id = "deepseek-primary".to_owned();
        definition.credential_pools[0].provider = openbridge::provider::ProviderKind::DeepSeek;
        let instance = &mut definition.provider_instances[0];
        instance.id = "deepseek-test".to_owned();
        instance.kind = openbridge::provider::ProviderKind::DeepSeek;
        instance.base_url = "https://api.deepseek.com".to_owned();
        let target = &mut definition.upstream_targets[0];
        target.provider_instance = "deepseek-test".to_owned();
        target.provider_model = "deepseek/test-model".to_owned();
        target.credential_pool = "deepseek-primary".to_owned();
        target.upstream_apis.truncate(1);
        if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut target.upstream_apis[0].capabilities
        {
            capabilities.reasoning_output = reasoning_output;
        }
        definition.routes = vec![RouteConfig {
            id: "responses-via-chat".to_owned(),
            upstream_target: "openai-main".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: ApiProtocol::Responses.operation(),
            mode: RouteMode::Bridged,
        }];
        definition.public_models[0].routes = vec!["responses-via-chat".to_owned()];
        definition
    }

    let request = serde_json::to_vec(&serde_json::json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "high"}
    }))
    .unwrap();
    let unknown = build_test_registry(definition(ReasoningOutput::Unknown));
    assert!(matches!(
        support::prepare(&unknown, ApiProtocol::Responses, request.clone().into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    let readable = build_test_registry(definition(ReasoningOutput::PlainText));
    let plan = support::prepare(&readable, ApiProtocol::Responses, request.into())
        .expect("plain-text reasoning should remain bridgeable");
    assert!(plan.candidates()[0].bridge().is_some());
}

#[test]
fn bridged_responses_never_exposes_native_continuation_state() {
    // Enable continuation on the sibling Native Responses API, then expose only a Chat-to-Responses Bridge.
    let mut definition = base_definition();
    let UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("second synthetic API must be Responses");
    };
    capabilities.state = ExecutableResponsesState::new(
        StorageSupport::Unsupported,
        ResponsesAffinity::TargetBoundContinuation,
    );
    definition.routes = vec![RouteConfig {
        id: "responses-via-chat".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::Responses,
        mode: RouteMode::Bridged,
    }];
    definition.public_models[0].routes = vec!["responses-via-chat".to_owned()];
    let registry = build_test_registry(definition);

    // Keep continuation out of both the public Bridge contract and pre-egress planning.
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["responses"]["state"]["previous_response_id"],
        "unsupported"
    );
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "previous_response_id": "resp_test"
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

fn build_test_registry(definition: RegistryConfig) -> RuntimeRegistry {
    build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap()
}

#[test]
fn native_routing_preserves_original_request_for_the_provider_adapter() {
    let registry = build_test_registry(base_definition());
    let original = json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "hello"}],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "stream": true
    });

    let prepared = support::prepare(
        &registry,
        ApiProtocol::ChatCompletions,
        serde_json::to_vec(&original).unwrap().into(),
    )
    .unwrap();
    let preserved: Value = serde_json::from_slice(prepared.request().body()).unwrap();

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(preserved, original);
}

#[test]
fn generation_request_analysis_rejects_unknown_top_level_fields_for_both_paths() {
    // Keep one Native Chat Route and replace Responses coverage with the reverse Bridge.
    let mut definition = base_definition();
    definition.routes.retain(|route| {
        route.downstream_operation == OperationKind::ChatCompletions
            && route.upstream_operation == OperationKind::ChatCompletions
    });
    definition.routes.push(RouteConfig {
        id: "responses-via-chat".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::Responses,
        mode: RouteMode::Bridged,
    });
    definition.public_models[0].routes =
        vec!["public-chat".to_owned(), "responses-via-chat".to_owned()];
    let registry = build_test_registry(definition);

    // Unknown fields must be classified before either Native preservation or Bridge validation.
    for (protocol, request) in [
        (
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": [], "future_parameter": null}),
        ),
        (
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "hello", "future_parameter": 1}),
        ),
    ] {
        let error = support::prepare(
            &registry,
            protocol,
            serde_json::to_vec(&request).unwrap().into(),
        )
        .expect_err("unknown top-level field must fail before Route preparation");
        assert_eq!(
            error.to_string(),
            "request contains unknown top-level parameter future_parameter"
        );
    }
}

#[test]
fn generation_interfaces_exclude_parameters_owned_only_by_another_source_protocol() {
    // Declare one Chat-only canonical parameter across otherwise symmetric Native APIs.
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).supported_parameters =
        vec!["max_tokens".to_owned()];
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();

    // Advertise the parameter only where request analysis recognizes the same source wire field.
    let chat_parameters = info["interfaces"]["chat_completions"]["supported_parameters"]
        .as_array()
        .unwrap();
    let responses_parameters = info["interfaces"]["responses"]["supported_parameters"]
        .as_array()
        .unwrap();
    assert!(chat_parameters.iter().any(|value| value == "max_tokens"));
    assert!(
        !responses_parameters
            .iter()
            .any(|value| value == "max_tokens")
    );
    let response_request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "max_tokens": 16
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(
            &registry,
            ApiProtocol::Responses,
            response_request.into()
        )
        .unwrap_err(),
        RequestPlanningError::UnknownParameter(parameter) if parameter == "max_tokens"
    ));
}

#[test]
fn candidate_parameter_ignores_apply_before_bridge_without_mutating_fallbacks() {
    // Compile one Responses Bridge whose Chat API accepts temperature only as an ignored hint.
    let mut bridged = base_definition();
    support::generation_profile_mut(&mut bridged.models[0]).supported_parameters =
        vec!["temperature".to_owned()];
    bridged.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];
    bridged.routes = vec![RouteConfig {
        id: "responses-via-chat".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::Responses,
        mode: RouteMode::Bridged,
    }];
    bridged.public_models[0].routes = vec!["responses-via-chat".to_owned()];
    let registry = build_test_registry(bridged);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "temperature": 0.2
    }))
    .unwrap();
    let plan = support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap();
    let upstream: Value = serde_json::from_slice(plan.request().body()).unwrap();
    assert!(
        upstream.get("temperature").is_none(),
        "ignored hint must be removed before Responses-to-Chat conversion"
    );

    // Give the preferred Native API an ignore rule while its fallback supports the same field.
    let mut fallback = base_definition();
    support::generation_profile_mut(&mut fallback.models[0]).supported_parameters =
        vec!["temperature".to_owned()];
    fallback.upstream_targets[0].upstream_apis[0]
        .model_rules
        .ignored_parameters = vec![IgnorableGenerationParameter::Temperature];
    let mut supporting_target = fallback.upstream_targets[0].clone();
    supporting_target.id = "openai-temperature-fallback".to_owned();
    supporting_target.upstream_apis[0]
        .model_rules
        .ignored_parameters
        .clear();
    fallback.upstream_targets.push(supporting_target);
    fallback.routes.retain(|route| {
        route.downstream_operation == OperationKind::ChatCompletions
            && route.upstream_operation == OperationKind::ChatCompletions
    });
    fallback.routes.push(RouteConfig {
        id: "temperature-fallback-chat".to_owned(),
        upstream_target: "openai-temperature-fallback".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::ChatCompletions,
        mode: RouteMode::Native,
    });
    fallback.public_models[0].routes = vec![
        "public-chat".to_owned(),
        "temperature-fallback-chat".to_owned(),
    ];
    let registry = build_test_registry(fallback);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "temperature": 0.2
    }))
    .unwrap();
    let plan = support::prepare(&registry, ApiProtocol::ChatCompletions, request.into()).unwrap();
    let preferred: Value = serde_json::from_slice(plan.candidates()[0].request().body()).unwrap();
    let fallback: Value = serde_json::from_slice(plan.candidates()[1].request().body()).unwrap();
    assert!(preferred.get("temperature").is_none());
    assert_eq!(fallback["temperature"], 0.2);
}

#[test]
fn public_model_preflight_rejects_unknown_models() {
    let registry = build_test_registry(base_definition());
    let body = serde_json::to_vec(&json!({"model": "missing", "messages": []})).unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnknownModel
    ));
}

#[test]
fn request_analysis_rejects_invalid_json_and_non_object_documents() {
    let registry = build_test_registry(base_definition());

    // Reject malformed JSON and valid JSON values that are not request objects.
    for body in [b"{".as_slice(), b"[]".as_slice(), b"null".as_slice()] {
        assert!(matches!(
            support::prepare(
                &registry,
                ApiProtocol::ChatCompletions,
                body.to_vec().into()
            )
            .unwrap_err(),
            RequestPlanningError::InvalidJson
        ));
    }
}

#[test]
fn request_analysis_rejects_missing_or_empty_model_values() {
    let registry = build_test_registry(base_definition());

    // Require a non-empty string model before Public Model lookup begins.
    for request in [json!({}), json!({"model": ""}), json!({"model": null})] {
        let body = serde_json::to_vec(&request).unwrap();
        assert!(matches!(
            support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
            RequestPlanningError::MissingModel
        ));
    }
}

#[test]
fn public_model_preflight_rejects_streaming_when_the_fixed_interface_disables_it() {
    // Compile a Chat interface whose only executable candidate disables streaming.
    let mut definition = base_definition();
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.streaming = false;
    }
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "stream": true
    }))
    .unwrap();

    // Reject the request at fixed-interface preflight before selecting egress.
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::StreamingUnsupported
    ));
}

#[test]
fn public_model_preflight_rejects_non_streaming_when_conversion_is_disabled() {
    // Compile a streaming-only Chat API whose trusted non-streaming conversion switch is off.
    let mut definition = base_definition();
    definition.upstream_targets[0].upstream_apis[0].streaming_policy =
        UpstreamStreamingPolicy::Required {
            non_streaming: NonStreamingConversion::Disabled,
        };
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "openai-non-streaming-fallback".to_owned();
    fallback.upstream_apis[0].streaming_policy = UpstreamStreamingPolicy::Optional;
    definition.upstream_targets.push(fallback);
    definition.routes.push(RouteConfig {
        id: "non-streaming-fallback-chat".to_owned(),
        upstream_target: "openai-non-streaming-fallback".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: OperationKind::ChatCompletions,
        mode: RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("non-streaming-fallback-chat".to_owned());
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["non_streaming"],
        "unsupported"
    );

    // Reject before egress instead of skipping the preferred source for a stronger fallback Route.
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": []
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::NonStreamingUnsupported
    ));
}

#[test]
fn public_model_preflight_rejects_capabilities_not_guaranteed_by_its_contract() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = None;
    }
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn public_model_capability_rejection_does_not_select_a_stronger_later_route() {
    // Build a preferred Chat Route with weaker capability and a later Route supporting tools.
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = None;
    }
    let mut stronger = definition.upstream_targets[0].clone();
    stronger.id = "openai-stronger".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut stronger.upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(all_function_tools());
    }
    definition.upstream_targets.push(stronger);
    definition.routes.push(RouteConfig {
        id: "stronger-chat".to_owned(),
        upstream_target: "openai-stronger".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec!["public-chat".to_owned(), "stronger-chat".to_owned()];
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();

    // The Public Model fixed intersection does not support tools; a stronger later Route cannot change eligibility.
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn public_model_preflight_gates_output_parallel_image_and_reasoning_requirements() {
    let mut definition = base_definition();
    support::generation_profile_mut(&mut definition.models[0]).context_length =
        ModelContextLength::new(None, None, Some(32));
    let registry = build_test_registry(definition);

    let too_large = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 33
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, too_large.into()).unwrap_err(),
        RequestPlanningError::OutputLimitExceeded
    ));

    let parallel = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "parallel_tool_calls": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, parallel.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let image = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{
            "role": "user",
            "content": [{"type": "image_url", "image_url": {"url": "https://example.invalid/image.png"}}]
        }]
    }))
        .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, image.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let reasoning = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": {"effort": "low"}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, reasoning.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));

    let invalid_shape = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "reasoning": false
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, invalid_shape.into()).unwrap_err(),
        RequestPlanningError::ReasoningLevelUnsupported
    ));
}

#[test]
fn native_image_preflight_accepts_the_minimum_legal_remote_and_data_wires() {
    // Compile one profile whose source budgets admit exactly the minimum canonical wire examples.
    let image_input = remote_and_data_image_input(
        1,
        "https://a".len() as u32,
        TINY_IMAGE_MEDIA_TYPES,
        InlineImageInputLimits::new(4, 1, 4, 1),
        omitted_auto_detail(),
    );
    let mut definition = base_definition();
    set_image_input(
        &mut definition,
        0,
        OperationKind::ChatCompletions,
        image_input,
    );
    set_image_input(&mut definition, 0, OperationKind::Responses, image_input);
    let registry = build_test_registry(definition);

    // Admit the shortest explicit HTTPS host and one canonical one-byte Base64 payload.
    for (protocol, request) in [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": [{
                    "type": "image_url",
                    "image_url": {"url": "https://a"}
                }]}]
            }),
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": [{"role": "user", "content": [{
                    "type": "input_image",
                    "image_url": "data:image/png;base64,AA=="
                }]}]
            }),
        ),
    ] {
        let body = serde_json::to_vec(&request).unwrap();
        support::prepare(&registry, protocol, body.into())
            .expect("minimum legal image wire must pass the compiled profile");
    }
}

#[test]
fn public_image_projection_preserves_exact_source_specific_payloads() {
    let cases = [
        (
            remote_image_input(3, 1_024, omitted_auto_detail()),
            json!({
                "sources": ["remote_url"],
                "media_types": [],
                "detail": {"default": "auto", "allowed": []},
                "limits": {
                    "max_parts": 3,
                    "max_url_length": 1_024,
                    "max_inline_encoded_bytes": 0,
                    "max_inline_decoded_bytes": 0,
                    "max_total_inline_encoded_bytes": 0,
                    "max_total_inline_decoded_bytes": 0
                }
            }),
        ),
        (
            data_image_input(
                2,
                TINY_IMAGE_MEDIA_TYPES,
                InlineImageInputLimits::new(8, 6, 16, 12),
                explicit_auto_detail(LOW_AND_HIGH_IMAGE_DETAILS),
            ),
            json!({
                "sources": ["data_url"],
                "media_types": ["image/png"],
                "detail": {"default": "auto", "allowed": ["low", "high"]},
                "limits": {
                    "max_parts": 2,
                    "max_url_length": 0,
                    "max_inline_encoded_bytes": 8,
                    "max_inline_decoded_bytes": 6,
                    "max_total_inline_encoded_bytes": 16,
                    "max_total_inline_decoded_bytes": 12
                }
            }),
        ),
        (
            remote_and_data_image_input(
                2,
                1_024,
                TINY_IMAGE_MEDIA_TYPES,
                InlineImageInputLimits::new(8, 6, 16, 12),
                explicit_auto_detail(LOW_AND_HIGH_IMAGE_DETAILS),
            ),
            json!({
                "sources": ["remote_url", "data_url"],
                "media_types": ["image/png"],
                "detail": {"default": "auto", "allowed": ["low", "high"]},
                "limits": {
                    "max_parts": 2,
                    "max_url_length": 1_024,
                    "max_inline_encoded_bytes": 8,
                    "max_inline_decoded_bytes": 6,
                    "max_total_inline_encoded_bytes": 16,
                    "max_total_inline_decoded_bytes": 12
                }
            }),
        ),
    ];

    // Compile each closed source variant through the OpenAI Provider ceiling.
    for (image_input, expected) in cases {
        let mut definition = base_definition();
        set_image_input(
            &mut definition,
            0,
            OperationKind::ChatCompletions,
            image_input,
        );

        // Compare the complete existing flat JSON projection, including zero-only derived fields.
        assert_eq!(public_chat_image(definition), expected);
    }
}

#[test]
fn public_image_intersection_closes_or_downgrades_disjoint_data_sources() {
    // Intersect two data-only profiles whose MIME sets are disjoint.
    let mut data_only = base_definition();
    set_image_input(
        &mut data_only,
        0,
        OperationKind::ChatCompletions,
        data_image_input(
            2,
            TINY_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(8, 6, 16, 12),
            omitted_auto_detail(),
        ),
    );
    add_chat_image_candidate(
        &mut data_only,
        "jpeg-data-target",
        "jpeg-data-chat",
        data_image_input(
            2,
            JPEG_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(8, 6, 16, 12),
            omitted_auto_detail(),
        ),
    );
    assert!(public_chat_image(data_only).is_null());

    // Preserve the complete remote payload when the same disjoint MIME sets occur under Both.
    let mut both = base_definition();
    set_image_input(
        &mut both,
        0,
        OperationKind::ChatCompletions,
        remote_and_data_image_input(
            3,
            1_024,
            TINY_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(8, 6, 16, 12),
            omitted_auto_detail(),
        ),
    );
    add_chat_image_candidate(
        &mut both,
        "jpeg-both-target",
        "jpeg-both-chat",
        remote_and_data_image_input(
            2,
            900,
            JPEG_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(8, 6, 16, 12),
            omitted_auto_detail(),
        ),
    );
    assert_eq!(
        public_chat_image(both),
        json!({
            "sources": ["remote_url"],
            "media_types": [],
            "detail": {"default": "auto", "allowed": []},
            "limits": {
                "max_parts": 2,
                "max_url_length": 900,
                "max_inline_encoded_bytes": 0,
                "max_inline_decoded_bytes": 0,
                "max_total_inline_encoded_bytes": 0,
                "max_total_inline_decoded_bytes": 0
            }
        })
    );
}

#[test]
fn public_image_detail_intersection_keeps_omission_separate_from_explicit_values() {
    // Close the image interface when omission would have different behavior across fallback Routes.
    let mut mismatched_default = base_definition();
    set_image_input(
        &mut mismatched_default,
        0,
        OperationKind::ChatCompletions,
        remote_image_input(2, 1_024, omitted_auto_detail()),
    );
    add_mimo_chat_image_candidate(
        &mut mismatched_default,
        "unknown-default-target",
        "unknown-default-chat",
        remote_image_input(2, 1_024, ImageDetailPolicy::OmittedOnly { default: None }),
    );
    assert!(public_chat_image(mismatched_default).is_null());

    // Downgrade disjoint explicit domains to omission-only while retaining the shared default.
    let mut disjoint_explicit = base_definition();
    set_image_input(
        &mut disjoint_explicit,
        0,
        OperationKind::ChatCompletions,
        remote_image_input(2, 1_024, explicit_auto_detail(LOW_IMAGE_DETAIL)),
    );
    add_chat_image_candidate(
        &mut disjoint_explicit,
        "high-detail-target",
        "high-detail-chat",
        remote_image_input(2, 1_024, explicit_auto_detail(HIGH_IMAGE_DETAIL)),
    );
    let registry = build_test_registry(disjoint_explicit);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["multimodal_input"]["image"]["detail"],
        json!({"default": "auto", "allowed": []})
    );

    // Admit omission but reject a value accepted by only one candidate through the same compiled contract.
    let omitted = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [{
            "type": "image_url",
            "image_url": {"url": "https://example.invalid/image.png"}
        }]}]
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::ChatCompletions, omitted.into()).is_ok());
    let explicit = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [{
            "type": "image_url",
            "image_url": {"url": "https://example.invalid/image.png", "detail": "low"}
        }]}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, explicit.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn public_image_intersection_clamps_cross_minima_to_reachable_inline_totals() {
    // Combine minima taken from opposite profiles so raw independent minima would be unreachable.
    let mut definition = base_definition();
    set_image_input(
        &mut definition,
        0,
        OperationKind::ChatCompletions,
        data_image_input(
            1,
            PNG_AND_JPEG_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(100, 75, 100, 75),
            omitted_auto_detail(),
        ),
    );
    add_chat_image_candidate(
        &mut definition,
        "small-item-target",
        "small-item-chat",
        data_image_input(
            25,
            TINY_IMAGE_MEDIA_TYPES,
            InlineImageInputLimits::new(4, 3, 100, 75),
            omitted_auto_detail(),
        ),
    );
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["multimodal_input"]["image"],
        json!({
            "sources": ["data_url"],
            "media_types": ["image/png"],
            "detail": {"default": "auto", "allowed": []},
            "limits": {
                "max_parts": 1,
                "max_url_length": 0,
                "max_inline_encoded_bytes": 4,
                "max_inline_decoded_bytes": 3,
                "max_total_inline_encoded_bytes": 4,
                "max_total_inline_decoded_bytes": 3
            }
        })
    );

    // Confirm preflight consumes the clamped contract rather than either unaggregated candidate.
    let accepted = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [{
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,AAAA"}
        }]}]
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::ChatCompletions, accepted.into()).is_ok());
    let above_clamp = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [{
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,AAAAAAAA"}
        }]}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, above_clamp.into()).unwrap_err(),
        RequestPlanningError::MultimodalInputLimitExceeded
    ));
}

#[test]
fn native_image_preflight_enforces_per_part_and_cumulative_inline_byte_limits() {
    // Compile one deliberately small typed image profile for both protocol-native interfaces.
    let mut definition = base_definition();
    for upstream_api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut upstream_api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.image_input = Some(TINY_IMAGE_INPUT);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.image_input = Some(TINY_IMAGE_INPUT);
            }
            UpstreamApiCapabilities::Embeddings(_) => unreachable!("generation fixture"),
        }
    }
    let registry = build_test_registry(definition);

    // Accept one four-character payload whose canonical decoded size is exactly three bytes.
    let accepted = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [{
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,AAAA"}
        }]}]
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::ChatCompletions, accepted.into()).is_ok());

    // Reject either one oversized part or two individually valid parts exceeding the cumulative ceiling.
    for content in [
        json!([{
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,AAAAAAAA"}
        }]),
        json!([
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
        ]),
    ] {
        let body = serde_json::to_vec(&json!({
            "model": "public-model",
            "messages": [{"role": "user", "content": content}]
        }))
        .unwrap();
        assert!(matches!(
            support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap_err(),
            RequestPlanningError::MultimodalInputLimitExceeded
        ));
    }
}

#[test]
fn public_model_output_limit_uses_the_most_restrictive_route() {
    let mut definition = base_definition();
    let mut limited = definition.upstream_targets[0].clone();
    limited.id = "openai-limited".to_owned();
    limited.upstream_apis[0].upstream_model = "limited-upstream-model".to_owned();
    limited.upstream_apis[0].model_rules.context_length =
        ModelContextLength::new(None, None, Some(4_096));
    definition.upstream_targets.push(limited);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "limited-chat".to_owned(),
        upstream_target: "openai-limited".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0].routes = vec!["limited-chat".to_owned(), "public-chat".to_owned()];
    let registry = build_test_registry(definition);
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "max_completion_tokens": 4097
    }))
    .unwrap();

    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, request.into()).unwrap_err(),
        RequestPlanningError::OutputLimitExceeded
    ));
}

#[test]
fn public_model_interfaces_scope_capabilities_and_detect_strict_functions() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.store = true;
    }
    let registry = build_test_registry(definition);

    let chat_store = serde_json::to_vec(&json!({
        "model": "public-model", "messages": [], "store": true
    }))
    .unwrap();
    assert!(support::prepare(&registry, ApiProtocol::ChatCompletions, chat_store.into()).is_ok());

    let responses_store = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "store": true
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, responses_store.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let unmodeled_tool = serde_json::to_vec(&json!({
        "model": "public-model", "input": "hello", "tools": [{"type": "future_tool"}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, unmodeled_tool.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let strict_function = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe", "strict": true}}]
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(
            &registry,
            ApiProtocol::ChatCompletions,
            strict_function.into()
        )
        .unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn absent_operation_is_omitted_from_the_public_model_and_planner() {
    let mut definition = base_definition();
    definition.upstream_targets[0]
        .upstream_apis
        .retain(|api| api.capabilities.operation() != OperationKind::Responses);
    definition
        .routes
        .retain(|route| route.id != "public-responses");
    definition.public_models[0]
        .routes
        .retain(|route| route != "public-responses");
    let registry = build_test_registry(definition);

    // Project only the operation backed by a present Target API and Route.
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert!(info["interfaces"]["chat_completions"].is_object());
    assert!(info["interfaces"]["responses"].is_null());

    // Reject the absent operation before request planning can select an upstream candidate.
    let request = serde_json::to_vec(&json!({"model": "public-model", "input": "hello"})).unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap_err(),
        RequestPlanningError::UnsupportedProtocol
    ));
}

#[test]
fn disjoint_structured_output_routes_compile_to_unsupported_contract() {
    // Narrow both original Native operations to the independently valid JSON Object profile.
    let mut definition = base_definition();
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
            }
            UpstreamApiCapabilities::Embeddings(_) => {}
        }
    }

    // Clone both operations under one second Target and narrow them to non-strict JSON Schema.
    let mut schema_target = definition.upstream_targets[0].clone();
    schema_target.id = "openai-schema-only".to_owned();
    for api in &mut schema_target.upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::NonStrictOnly,
                ));
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(StructuredOutputProfile::JsonSchema(
                    JsonSchemaSupport::NonStrictOnly,
                ));
            }
            UpstreamApiCapabilities::Embeddings(_) => {}
        }
    }
    definition.upstream_targets.push(schema_target);
    definition.routes.extend([
        RouteConfig {
            id: "schema-only-chat".to_owned(),
            upstream_target: "openai-schema-only".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "schema-only-responses".to_owned(),
            upstream_target: "openai-schema-only".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::Responses,
            mode: RouteMode::Native,
        },
    ]);
    definition.public_models[0].routes.extend([
        "schema-only-chat".to_owned(),
        "schema-only-responses".to_owned(),
    ]);

    // Compile both operations and require one closed unsupported projection with no control fields.
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    let expected = json!({
        "support": "unsupported",
        "modes": [],
        "strict_schema": "unsupported"
    });
    for protocol in ["chat_completions", "responses"] {
        let interface = &info["interfaces"][protocol];
        assert_eq!(interface["structured_outputs"], expected, "{protocol}");
        let parameters = interface["supported_parameters"].as_array().unwrap();
        for parameter in ["response_format", "text", "structured_outputs"] {
            assert!(
                !parameters.iter().any(|value| value == parameter),
                "{protocol} must remove {parameter}"
            );
        }
    }

    // Reject both Structured Output modes for both protocols without producing any RoutePlan.
    let requests = [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {"type": "json_object"}
            }),
        ),
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {"name": "answer", "schema": {"type": "object"}}
                }
            }),
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_object"}}
            }),
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {
                    "type": "json_schema",
                    "name": "answer",
                    "schema": {"type": "object"}
                }}
            }),
        ),
    ];
    for (protocol, request) in requests {
        let body = serde_json::to_vec(&request).unwrap();
        assert!(matches!(
            support::prepare(&registry, protocol, body.into()),
            Err(RequestPlanningError::UnsupportedCapabilities)
        ));
    }
}

#[test]
fn structured_output_schema_and_strict_elevation_fail_at_the_provider_boundary() {
    let cases = [
        (
            "schema-mode",
            StructuredOutputProfile::JsonSchema(JsonSchemaSupport::NonStrictOnly),
        ),
        (
            "strict-schema",
            StructuredOutputProfile::JsonSchema(JsonSchemaSupport::StrictSupported),
        ),
    ];

    // Elevate one checked-in JSON Object Target beyond Bailian's operation ceiling.
    for (case, elevated_profile) in cases {
        let mut definition = compiled_config();
        let target = definition
            .upstream_targets
            .iter_mut()
            .find(|target| target.id == "bailian-deepseek-v4-pro")
            .expect("the checked-in Bailian DeepSeek target must exist");
        let capabilities = target
            .upstream_apis
            .iter_mut()
            .find_map(|api| match &mut api.capabilities {
                UpstreamApiCapabilities::ChatCompletions(capabilities) => Some(capabilities),
                UpstreamApiCapabilities::Responses(_) | UpstreamApiCapabilities::Embeddings(_) => {
                    None
                }
            })
            .expect("the Bailian DeepSeek target must expose Chat Completions");
        capabilities.structured_outputs = Some(elevated_profile);

        // Reject both schema mode and strict-schema elevation before compiling the registry.
        assert!(
            matches!(
                build_registry(support::bootstrap(support::BOOTSTRAP), definition),
                Err(RegistryError::CapabilityElevation {
                    upstream_operation: OperationKind::ChatCompletions,
                    ..
                })
            ),
            "{case}"
        );
    }
}

#[test]
fn fine_grained_generation_capabilities_intersect_without_capability_routing() {
    let mut definition = base_definition();
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(FunctionToolCapabilities {
            choice_modes: ALL_TOOL_CHOICE_MODES,
            parallel_calls: true,
            strict_schema: true,
        });
        capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObjectAndJsonSchema(
            JsonSchemaSupport::StrictSupported,
        ));
    }
    let mut weaker = definition.upstream_targets[0].clone();
    weaker.id = "openai-weaker".to_owned();
    if let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut weaker.upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(FunctionToolCapabilities {
            choice_modes: COMMON_TOOL_CHOICE_MODES,
            parallel_calls: false,
            strict_schema: false,
        });
        capabilities.structured_outputs = Some(StructuredOutputProfile::JsonObject);
    }
    definition.upstream_targets.push(weaker);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "weaker-chat".to_owned(),
        upstream_target: "openai-weaker".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .extend(["weaker-chat".to_owned()]);
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["tools"]["tool_choice_modes"],
        json!(["none", "auto"])
    );
    assert_eq!(
        info["interfaces"]["chat_completions"]["structured_outputs"]["modes"],
        json!(["json_object"])
    );
    assert_eq!(
        info["interfaces"]["chat_completions"]["structured_outputs"],
        json!({
            "support": "supported",
            "modes": ["json_object"],
            "strict_schema": "unsupported"
        })
    );
    assert_eq!(
        info["interfaces"]["chat_completions"]["tools"]["parallel_calls"],
        "unsupported"
    );

    let unsupported = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "response_format": {"type": "json_schema", "json_schema": {"name": "answer", "schema": {"type": "object"}}}
    }))
    .unwrap();
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::ChatCompletions, unsupported.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));

    let supported = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}],
        "tool_choice": "auto"
    }))
    .unwrap();
    let plan = support::prepare(&registry, ApiProtocol::ChatCompletions, supported.into()).unwrap();
    assert_eq!(plan.candidates().len(), 2);
    assert_eq!(plan.candidates()[0].route_id(), "public-chat");
    assert_eq!(plan.candidates()[1].route_id(), "weaker-chat");
}

#[test]
fn combined_structured_output_intersection_keeps_mode_order_and_downgrades_strict() {
    let strict =
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);
    let non_strict =
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::NonStrictOnly);

    // Give the original Native operations the complete strict-capable combined profile.
    let mut definition = base_definition();
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(strict);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(strict);
            }
            UpstreamApiCapabilities::Embeddings(_) => {}
        }
    }

    // Add a second Native Target that keeps both modes but accepts non-strict schemas only.
    let mut weaker = definition.upstream_targets[0].clone();
    weaker.id = "openai-non-strict".to_owned();
    for api in &mut weaker.upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(non_strict);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(non_strict);
            }
            UpstreamApiCapabilities::Embeddings(_) => {}
        }
    }
    definition.upstream_targets.push(weaker);
    definition.routes.extend([
        RouteConfig {
            id: "non-strict-chat".to_owned(),
            upstream_target: "openai-non-strict".to_owned(),
            upstream_operation: OperationKind::ChatCompletions,
            downstream_operation: OperationKind::ChatCompletions,
            mode: RouteMode::Native,
        },
        RouteConfig {
            id: "non-strict-responses".to_owned(),
            upstream_target: "openai-non-strict".to_owned(),
            upstream_operation: OperationKind::Responses,
            downstream_operation: OperationKind::Responses,
            mode: RouteMode::Native,
        },
    ]);
    definition.public_models[0].routes.extend([
        "non-strict-chat".to_owned(),
        "non-strict-responses".to_owned(),
    ]);

    // Project one stable combined profile while conservatively dropping strict-schema support.
    let registry = build_test_registry(definition);
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    let expected = json!({
        "support": "supported",
        "modes": ["json_object", "json_schema"],
        "strict_schema": "unsupported"
    });
    for protocol in ["chat_completions", "responses"] {
        assert_eq!(info["interfaces"][protocol]["structured_outputs"], expected);
    }

    // Admit non-strict schema requests to both candidates and reject strict requests globally.
    let requests = [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {"name": "answer", "schema": {"type": "object"}}
                }
            }),
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "answer",
                        "schema": {"type": "object"},
                        "strict": true
                    }
                }
            }),
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {
                    "type": "json_schema",
                    "name": "answer",
                    "schema": {"type": "object"}
                }}
            }),
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {
                    "type": "json_schema",
                    "name": "answer",
                    "schema": {"type": "object"},
                    "strict": true
                }}
            }),
        ),
    ];
    for (protocol, non_strict_request, strict_request) in requests {
        let non_strict_body = serde_json::to_vec(&non_strict_request).unwrap();
        let plan = support::prepare(&registry, protocol, non_strict_body.into()).unwrap();
        assert_eq!(plan.candidates().len(), 2, "{protocol:?}");

        let strict_body = serde_json::to_vec(&strict_request).unwrap();
        assert!(matches!(
            support::prepare(&registry, protocol, strict_body.into()),
            Err(RequestPlanningError::UnsupportedCapabilities)
        ));
    }
}

#[test]
fn structured_output_analysis_and_preflight_cover_each_public_request_variant() {
    let strict =
        StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);

    // Compile both protocol interfaces with the complete strict-capable profile.
    let mut definition = base_definition();
    for api in &mut definition.upstream_targets[0].upstream_apis {
        match &mut api.capabilities {
            UpstreamApiCapabilities::ChatCompletions(capabilities) => {
                capabilities.structured_outputs = Some(strict);
            }
            UpstreamApiCapabilities::Responses(capabilities) => {
                capabilities.structured_outputs = Some(strict);
            }
            UpstreamApiCapabilities::Embeddings(_) => {}
        }
    }
    let registry = build_test_registry(definition);

    // Exercise absent, plain text, Object, Schema strictness, and unknown variants per protocol.
    let cases = [
        (
            "chat-absent",
            ApiProtocol::ChatCompletions,
            json!({"model": "public-model", "messages": []}),
            true,
        ),
        (
            "chat-plain-text",
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {"type": "text"}
            }),
            true,
        ),
        (
            "chat-object",
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {"type": "json_object"}
            }),
            true,
        ),
        (
            "chat-schema-non-strict",
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {"type": "json_schema", "json_schema": {"name": "answer"}}
            }),
            true,
        ),
        (
            "chat-schema-strict",
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {"name": "answer", "strict": true}
                }
            }),
            true,
        ),
        (
            "chat-unknown",
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [],
                "response_format": {"type": "future_json"}
            }),
            false,
        ),
        (
            "responses-absent",
            ApiProtocol::Responses,
            json!({"model": "public-model", "input": "answer"}),
            true,
        ),
        (
            "responses-plain-text",
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "text"}}
            }),
            true,
        ),
        (
            "responses-object",
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_object"}}
            }),
            true,
        ),
        (
            "responses-schema-non-strict",
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_schema", "name": "answer"}}
            }),
            true,
        ),
        (
            "responses-schema-strict",
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "json_schema", "name": "answer", "strict": true}}
            }),
            true,
        ),
        (
            "responses-unknown",
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": "answer",
                "text": {"format": {"type": "future_json"}}
            }),
            false,
        ),
    ];
    for (case, protocol, request, accepted) in cases {
        let body = serde_json::to_vec(&request).unwrap();
        let result = support::prepare(&registry, protocol, body.into());
        if accepted {
            assert_eq!(result.unwrap().candidates().len(), 1, "{case}");
        } else {
            assert!(
                matches!(result, Err(RequestPlanningError::UnsupportedCapabilities)),
                "{case}: {result:?}"
            );
        }
    }
}

#[test]
fn route_plan_preserves_configured_order_after_public_model_preflight() {
    let mut definition = base_definition();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = None;
    }
    let mut tools = definition.upstream_targets[0].clone();
    tools.id = "openai-tools".to_owned();
    tools.upstream_apis[0].upstream_model = "tool-capable-model".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut tools.upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(all_function_tools());
    }
    definition.upstream_targets.push(tools);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "tools-chat".to_owned(),
        upstream_target: "openai-tools".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("tools-chat".to_owned());
    let registry = build_test_registry(definition);
    let body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": []
    }))
    .unwrap();

    let prepared = support::prepare(&registry, ApiProtocol::ChatCompletions, body.clone().into())
        .expect("a request accepted by the fixed contract should preserve configured routes");

    assert_eq!(prepared.upstream_target_id(), "openai-main");
    assert_eq!(prepared.candidates().len(), 2);
    assert_eq!(
        prepared.candidates()[1].upstream_target_id(),
        "openai-tools"
    );
    assert_eq!(prepared.request().body(), &body.as_slice());
}

#[test]
fn static_disabled_routes_do_not_contribute_to_the_compiled_interface_or_plan() {
    let mut definition = base_definition();

    // Keep a weaker configured Route disabled so it cannot narrow the visible contract or enter planning.
    definition.upstream_targets[0].enabled = false;
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    {
        capabilities.function_tools = None;
    }
    let mut enabled = definition.upstream_targets[0].clone();
    enabled.id = "openai-enabled".to_owned();
    enabled.enabled = true;
    enabled.upstream_apis[0].upstream_model = "enabled-tool-model".to_owned();
    if let openbridge::registry::UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut enabled.upstream_apis[0].capabilities
    {
        capabilities.function_tools = Some(all_function_tools());
    }
    definition.upstream_targets.push(enabled);
    definition.routes.push(RouteConfig {
        id: "enabled-chat".to_owned(),
        upstream_target: "openai-enabled".to_owned(),
        upstream_operation: OperationKind::ChatCompletions,
        downstream_operation: ApiProtocol::ChatCompletions.operation(),
        mode: RouteMode::Native,
    });
    definition.public_models[0].routes = vec![
        "public-chat".to_owned(),
        "enabled-chat".to_owned(),
        "public-responses".to_owned(),
    ];
    let registry = build_test_registry(definition);

    // Verify that only the enabled Route shapes both the published Chat contract and its candidates.
    let info = serde_json::to_value(
        registry
            .public_model("public-model")
            .expect("the enabled Chat interface must keep the model visible")
            .info(),
    )
    .unwrap();
    assert_eq!(
        info["interfaces"]["chat_completions"]["tools"]["support"],
        "supported"
    );
    let body = serde_json::to_vec(&json!({"model": "public-model", "messages": []})).unwrap();
    let plan = support::prepare(&registry, ApiProtocol::ChatCompletions, body.into()).unwrap();
    assert_eq!(
        plan.candidates()
            .iter()
            .map(|candidate| candidate.route_id())
            .collect::<Vec<_>>(),
        ["enabled-chat"]
    );

    // Confirm that the same static contract admits the enabled Route's function-tool request.
    let tool_body = serde_json::to_vec(&json!({
        "model": "public-model",
        "messages": [],
        "tools": [{"type": "function", "function": {"name": "probe"}}]
    }))
    .unwrap();
    let tool_plan = support::prepare(&registry, ApiProtocol::ChatCompletions, tool_body.into())
        .expect("the disabled weaker Route must not reject the enabled interface capability");
    assert_eq!(tool_plan.candidates()[0].route_id(), "enabled-chat");
}
