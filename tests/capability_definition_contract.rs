//! Verifies protocol naming and reserved boundaries for canonical Model and Chat/Responses capabilities.

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use openbridge::{
    core::{
        ApiProtocol, ChatCompletionsCapabilities, ExecutableResponsesState, HostedToolKind,
        ImageDetail, ImageDetailPolicy, ImageDetailProfile, ImageInputCapabilities, ImageMediaType,
        ImageSourceCapabilities, InlineImageInputLimits, InlineImageInputProfile,
        JsonSchemaSupport, ProviderResponsesStateCeiling, RemoteImageInputLimits, ResponseInclude,
        ResponsesAffinity, ResponsesCapabilities, StorageSupport, StructuredOutputMode,
        StructuredOutputProfile,
    },
    pipeline::RequestPlanningError,
    providers::compiled_config,
    registry::{
        CanonicalTaskKind, InputModality, OutputModality, RegistryError, UpstreamApiCapabilities,
        build_registry,
    },
};
use serde_json::{Value, json};

const HOSTED_TOOLS: &[HostedToolKind] = &[
    HostedToolKind::WebSearch,
    HostedToolKind::FileSearch,
    HostedToolKind::CodeInterpreter,
    HostedToolKind::ComputerUse,
    HostedToolKind::ImageGeneration,
    HostedToolKind::Mcp,
    HostedToolKind::Shell,
    HostedToolKind::ApplyPatch,
    HostedToolKind::ToolSearch,
    HostedToolKind::Skills,
    HostedToolKind::ProgrammaticToolCalling,
];
const INCLUDES: &[ResponseInclude] = &[
    ResponseInclude::WebSearchCallSources,
    ResponseInclude::CodeInterpreterCallOutputs,
    ResponseInclude::ComputerCallOutputImageUrl,
    ResponseInclude::FileSearchCallResults,
    ResponseInclude::InputImageImageUrl,
    ResponseInclude::OutputTextLogprobs,
    ResponseInclude::ReasoningEncryptedContent,
];

type ChatReservation = fn(&mut ChatCompletionsCapabilities);
type ResponsesReservation = fn(&mut ResponsesCapabilities);

const STORED_CONTINUATION_STATE: ExecutableResponsesState = ExecutableResponsesState::new(
    StorageSupport::Supported,
    ResponsesAffinity::TargetBoundContinuation,
);
const IMAGE_MEDIA_TYPES: &[ImageMediaType] = &[ImageMediaType::Png, ImageMediaType::Jpeg];
const EXPLICIT_IMAGE_DETAILS: &[ImageDetail] = &[ImageDetail::Low, ImageDetail::High];
const REMOTE_IMAGE_LIMITS: RemoteImageInputLimits = RemoteImageInputLimits::new(2_048);
const INLINE_IMAGE_LIMITS: InlineImageInputLimits =
    InlineImageInputLimits::new(1_024, 768, 2_048, 1_536);
const INLINE_IMAGE_PROFILE: InlineImageInputProfile =
    InlineImageInputProfile::new(IMAGE_MEDIA_TYPES, INLINE_IMAGE_LIMITS);
const EXPLICIT_IMAGE_DETAIL_PROFILE: ImageDetailProfile =
    ImageDetailProfile::new(Some(ImageDetail::Auto), EXPLICIT_IMAGE_DETAILS);
const REMOTE_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    1,
    ImageSourceCapabilities::RemoteUrl(REMOTE_IMAGE_LIMITS),
    ImageDetailPolicy::OmittedOnly {
        default: Some(ImageDetail::Auto),
    },
);
const DATA_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    2,
    ImageSourceCapabilities::DataUrl(INLINE_IMAGE_PROFILE),
    ImageDetailPolicy::Explicit(EXPLICIT_IMAGE_DETAIL_PROFILE),
);
const REMOTE_AND_DATA_IMAGE_INPUT: ImageInputCapabilities = ImageInputCapabilities::new(
    2,
    ImageSourceCapabilities::RemoteUrlAndDataUrl {
        remote: REMOTE_IMAGE_LIMITS,
        data: INLINE_IMAGE_PROFILE,
    },
    ImageDetailPolicy::Explicit(EXPLICIT_IMAGE_DETAIL_PROFILE),
);
const JSON_OBJECT_OUTPUT: StructuredOutputProfile = StructuredOutputProfile::JsonObject;
const NON_STRICT_JSON_SCHEMA_OUTPUT: StructuredOutputProfile =
    StructuredOutputProfile::JsonSchema(JsonSchemaSupport::NonStrictOnly);
const STRICT_COMBINED_OUTPUT: StructuredOutputProfile =
    StructuredOutputProfile::JsonObjectAndJsonSchema(JsonSchemaSupport::StrictSupported);

#[test]
fn response_include_values_round_trip_exact_wire_paths() {
    // Keep typed analysis, Models projection, and egress JSON on one exact closed wire vocabulary.
    for include in INCLUDES {
        let wire = include.as_wire();
        assert_eq!(ResponseInclude::from_wire(wire), Some(*include));
        assert_eq!(serde_json::to_value(include).unwrap(), json!(wire));
    }
    assert_eq!(ResponseInclude::from_wire("future.output"), None);
}

#[test]
fn checked_in_targets_publish_only_the_probed_request_option_pairs() {
    const CHAT_TARGETS: &[&str] = &[
        "bailian-glm-5-2",
        "bailian-qwen3-6-27b",
        "bailian-deepseek-v4-pro",
        "deepseek-v4-pro",
        "longcat-2",
        "kimi-cn-kimi-k3",
    ];
    const RESPONSES_TARGETS: &[&str] = &[
        "bailian-qwen3-7-plus",
        "bailian-qwen3-7-max",
        "bailian-qwen3-8-max",
        "deepseek-v4-flash",
        "longcat-2",
        "mimo-v2-5-pro",
        "mimo-v2-5",
        "openrouter-deepseek-v4-flash",
        "openrouter-minimax-m3",
    ];
    const REASONING_INCLUDE_TARGETS: &[&str] = &[
        "chatgpt-gpt-5-3-codex-spark",
        "chatgpt-gpt-5-5",
        "chatgpt-gpt-5-6-luna",
        "chatgpt-gpt-5-6-sol",
        "chatgpt-gpt-5-6-terra",
        "deepseek-v4-flash",
        "mimo-v2-5",
        "openrouter-deepseek-v4-flash",
    ];
    const REASONING_INCLUDE: &[ResponseInclude] = &[ResponseInclude::ReasoningEncryptedContent];

    // Compare every checked-in generation API against the exact Target/API pairs proved upstream.
    for target in compiled_config().upstream_targets {
        for upstream_api in target.upstream_apis {
            match upstream_api.capabilities {
                UpstreamApiCapabilities::ChatCompletions(capabilities) => assert_eq!(
                    capabilities.prompt_cache_key,
                    CHAT_TARGETS.contains(&target.id.as_str()),
                    "unexpected Chat prompt_cache_key capability for {}",
                    target.id
                ),
                UpstreamApiCapabilities::Responses(capabilities) => {
                    assert_eq!(
                        capabilities.prompt_cache_key,
                        RESPONSES_TARGETS.contains(&target.id.as_str()),
                        "unexpected Responses prompt_cache_key capability for {}",
                        target.id
                    );
                    assert_eq!(
                        capabilities.include,
                        if REASONING_INCLUDE_TARGETS.contains(&target.id.as_str()) {
                            REASONING_INCLUDE
                        } else {
                            &[]
                        },
                        "unexpected Responses include capability for {}",
                        target.id,
                    );
                }
                UpstreamApiCapabilities::Embeddings(_) => {}
            }
        }
    }
}

#[test]
fn structured_output_profile_is_a_non_empty_const_union_with_derived_accessors() {
    // Verify each closed variant derives its exact stable mode set and strictness contract.
    assert_eq!(
        JSON_OBJECT_OUTPUT.modes(),
        &[StructuredOutputMode::JsonObject]
    );
    assert!(JSON_OBJECT_OUTPUT.supports(StructuredOutputMode::JsonObject));
    assert!(!JSON_OBJECT_OUTPUT.supports(StructuredOutputMode::JsonSchema));
    assert!(!JSON_OBJECT_OUTPUT.supports_strict_schema());

    assert_eq!(
        NON_STRICT_JSON_SCHEMA_OUTPUT.modes(),
        &[StructuredOutputMode::JsonSchema]
    );
    assert!(!NON_STRICT_JSON_SCHEMA_OUTPUT.supports(StructuredOutputMode::JsonObject));
    assert!(NON_STRICT_JSON_SCHEMA_OUTPUT.supports(StructuredOutputMode::JsonSchema));
    assert!(!NON_STRICT_JSON_SCHEMA_OUTPUT.supports_strict_schema());

    assert_eq!(
        STRICT_COMBINED_OUTPUT.modes(),
        &[
            StructuredOutputMode::JsonObject,
            StructuredOutputMode::JsonSchema,
        ]
    );
    assert!(STRICT_COMBINED_OUTPUT.supports(StructuredOutputMode::JsonObject));
    assert!(STRICT_COMBINED_OUTPUT.supports(StructuredOutputMode::JsonSchema));
    assert!(STRICT_COMBINED_OUTPUT.supports_strict_schema());
}

#[test]
fn executable_responses_state_derives_continuation_affinity_from_one_union() {
    let cases = [
        (ResponsesAffinity::Unbound, false, false, false),
        (ResponsesAffinity::TargetBound, false, true, false),
        (ResponsesAffinity::TargetBoundContinuation, true, true, true),
    ];

    // Construct both storage states for every closed affinity variant.
    for (affinity, supports_continuation, is_target_bound, requires_single_member) in cases {
        let without_storage = ExecutableResponsesState::new(StorageSupport::Unsupported, affinity);
        let with_storage = ExecutableResponsesState::new(StorageSupport::Supported, affinity);

        // Derive affinity facts identically while retaining storage as an independent payload.
        for state in [&without_storage, &with_storage] {
            assert_eq!(state.supports_previous_response_id(), supports_continuation);
            assert_eq!(state.is_target_bound(), is_target_bound);
            assert_eq!(
                state.requires_single_credential_member(),
                requires_single_member
            );
        }
        assert!(!without_storage.supports_store());
        assert!(with_storage.supports_store());
        assert_ne!(without_storage, with_storage);
    }

    assert!(STORED_CONTINUATION_STATE.supports_previous_response_id());
}

#[test]
fn provider_responses_state_ceiling_preserves_independent_axes() {
    for (ceiling, supports_store, supports_continuation) in [
        (ProviderResponsesStateCeiling::Stateless, false, false),
        (ProviderResponsesStateCeiling::Storage, true, false),
        (ProviderResponsesStateCeiling::Continuation, false, true),
        (
            ProviderResponsesStateCeiling::StorageAndContinuation,
            true,
            true,
        ),
    ] {
        assert_eq!(ceiling.supports_store(), supports_store);
        assert_eq!(
            ceiling.supports_previous_response_id(),
            supports_continuation
        );
    }
}

#[test]
fn image_input_capabilities_bind_each_source_to_its_complete_payload() {
    assert_eq!(REMOTE_IMAGE_INPUT.max_parts(), 1);
    let ImageSourceCapabilities::RemoteUrl(remote) = REMOTE_IMAGE_INPUT.sources() else {
        panic!("remote-only image profile must retain its remote payload");
    };
    assert_eq!(remote.max_url_length(), 2_048);
    assert_eq!(
        REMOTE_IMAGE_INPUT.detail_policy(),
        ImageDetailPolicy::OmittedOnly {
            default: Some(ImageDetail::Auto),
        }
    );

    assert_eq!(DATA_IMAGE_INPUT.max_parts(), 2);
    let ImageSourceCapabilities::DataUrl(data) = DATA_IMAGE_INPUT.sources() else {
        panic!("data-only image profile must retain its inline payload");
    };
    assert_eq!(data.media_types(), IMAGE_MEDIA_TYPES);
    assert_eq!(data.limits(), INLINE_IMAGE_LIMITS);
    assert_eq!(data.limits().max_inline_encoded_bytes(), 1_024);
    assert_eq!(data.limits().max_inline_decoded_bytes(), 768);
    assert_eq!(data.limits().max_total_inline_encoded_bytes(), 2_048);
    assert_eq!(data.limits().max_total_inline_decoded_bytes(), 1_536);

    let ImageDetailPolicy::Explicit(detail) = DATA_IMAGE_INPUT.detail_policy() else {
        panic!("explicit image detail policy must retain its checked profile");
    };
    assert_eq!(detail.default(), Some(ImageDetail::Auto));
    assert_eq!(detail.allowed(), EXPLICIT_IMAGE_DETAILS);
    assert!(!detail.allowed().contains(&ImageDetail::Auto));

    let ImageSourceCapabilities::RemoteUrlAndDataUrl { remote, data } =
        REMOTE_AND_DATA_IMAGE_INPUT.sources()
    else {
        panic!("combined image profile must retain both source payloads");
    };
    assert_eq!(remote, REMOTE_IMAGE_LIMITS);
    assert_eq!(data, INLINE_IMAGE_PROFILE);
}

#[test]
fn provider_image_ceiling_accepts_each_source_subset_and_rejects_payload_elevation() {
    // Compile every closed source variant as a narrower executable OpenAI Chat profile.
    for (case, image_input) in [
        ("remote", REMOTE_IMAGE_INPUT),
        ("data", DATA_IMAGE_INPUT),
        ("remote-and-data", REMOTE_AND_DATA_IMAGE_INPUT),
    ] {
        let mut definition = support::definition(case, "public-model", "upstream");
        let UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[0].capabilities
        else {
            panic!("fixture must expose a Chat Completions upstream API");
        };
        capabilities.image_input = Some(image_input);
        build_registry(support::bootstrap(support::BOOTSTRAP), definition)
            .expect("each source-specific profile must be a valid Provider subset");
    }

    // Reject a complete remote payload whose URL budget exceeds the Provider ceiling.
    let mut elevated = support::definition("elevated-image", "public-model", "upstream");
    let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut elevated.upstream_targets[0].upstream_apis[0].capabilities
    else {
        panic!("fixture must expose a Chat Completions upstream API");
    };
    capabilities.image_input = Some(ImageInputCapabilities::new(
        1,
        ImageSourceCapabilities::RemoteUrl(RemoteImageInputLimits::new(8_193)),
        ImageDetailPolicy::OmittedOnly {
            default: Some(ImageDetail::Auto),
        },
    ));
    assert!(matches!(
        build_registry(support::bootstrap(support::BOOTSTRAP), elevated),
        Err(RegistryError::CapabilityElevation {
            upstream_operation: openbridge::core::OperationKind::ChatCompletions,
            ..
        })
    ));
}

#[test]
fn canonical_model_task_and_modalities_compile_into_public_model_information() {
    let mut definition = support::definition("model-facts", "public-model", "upstream");
    let profile = support::generation_profile_mut(&mut definition.models[0]);
    profile.input_modalities = Some(vec![
        InputModality::Text,
        InputModality::Image,
        InputModality::Audio,
        InputModality::File,
    ]);
    profile.output_modalities = Some(vec![
        OutputModality::Text,
        OutputModality::Image,
        OutputModality::Audio,
    ]);

    // Compile canonical facts and confirm that model ceilings remain separate from interface capabilities.
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();
    let model = registry.model("openai/test-model").unwrap();
    assert_eq!(model.task_kind(), CanonicalTaskKind::Generation);
    assert_eq!(model.input_modalities().unwrap().len(), 4);
    assert_eq!(model.output_modalities().unwrap().len(), 3);

    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["capabilities"]["modalities"]["input"],
        json!(["text", "image", "audio", "file"])
    );
    assert_eq!(
        info["interfaces"]["chat_completions"]["modalities"]["input"],
        json!(["text"])
    );
}

#[test]
fn reserved_interface_capabilities_fail_closed_before_registry_compilation() {
    let chat_cases: [(&str, ChatReservation); 7] = [
        ("custom_tool_calling", |capabilities| {
            capabilities.custom_tool_calling = true
        }),
        ("file_input", |capabilities| capabilities.file_input = true),
        ("predicted_outputs", |capabilities| {
            capabilities.predicted_outputs = true
        }),
        ("web_search", |capabilities| capabilities.web_search = true),
        ("moderation", |capabilities| capabilities.moderation = true),
        ("logprobs", |capabilities| capabilities.logprobs = true),
        ("multiple_choices", |capabilities| {
            capabilities.multiple_choices = true
        }),
    ];

    // Enable each reserved Chat capability and require compilation to stop at the closed boundary.
    for (case, configure) in chat_cases {
        let mut definition = support::definition(case, "public-model", "upstream");
        let UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[0].capabilities
        else {
            panic!("fixture must expose a Chat Completions upstream API");
        };
        configure(capabilities);

        assert_registry_compilation_panics(case, move || {
            let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
        });
    }

    let responses_cases: [(&str, ResponsesReservation); 8] = [
        ("custom_tool_calling", |capabilities| {
            capabilities.custom_tool_calling = true
        }),
        ("hosted_tools", |capabilities| {
            capabilities.hosted_tools = HOSTED_TOOLS
        }),
        ("file_input", |capabilities| capabilities.file_input = true),
        ("conversation", |capabilities| {
            capabilities.conversation = true
        }),
        ("prompt_templates", |capabilities| {
            capabilities.prompt_templates = true
        }),
        ("context_management", |capabilities| {
            capabilities.context_management = true
        }),
        ("moderation", |capabilities| capabilities.moderation = true),
        ("logprobs", |capabilities| capabilities.logprobs = true),
    ];

    // Enable each reserved Responses capability and require compilation to stop at the closed boundary.
    for (case, configure) in responses_cases {
        let mut definition = support::definition(case, "public-model", "upstream");
        let UpstreamApiCapabilities::Responses(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[1].capabilities
        else {
            panic!("fixture must expose a Responses upstream API");
        };
        configure(capabilities);

        assert_registry_compilation_panics(case, move || {
            let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
        });
    }
}

#[test]
fn every_reserved_chat_request_field_stops_before_route_planning() {
    let registry = support::registry("reserved-chat-requests", "public-model", "upstream");
    let cases = vec![
        (
            "custom_tool_calling",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "tools": [{"type": "custom", "name": "shell", "format": {"type": "text"}}]
            }),
        ),
        (
            "file_input",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": [{"type": "file", "file": {"file_id": "file_test"}}]}]
            }),
        ),
        (
            "predicted_outputs",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "prediction": {"type": "content", "content": "known output"}
            }),
        ),
        (
            "web_search",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "web_search_options": {}
            }),
        ),
        (
            "prompt_cache_options",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "prompt_cache_options": {"type": "ephemeral"}
            }),
        ),
        (
            "prompt_cache_retention",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "prompt_cache_retention": "24h"
            }),
        ),
        (
            "prompt_cache_breakpoint",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": [{"type": "prompt_cache_breakpoint"}]}]
            }),
        ),
        (
            "moderation",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "moderation": {}
            }),
        ),
    ];

    // Submit each capability-bearing Chat request and prove that it stops before the Provider adapter.
    assert_reserved_requests_unimplemented(&registry, ApiProtocol::ChatCompletions, cases);
}

#[test]
fn every_reserved_responses_request_field_stops_before_route_planning() {
    let registry = support::registry("reserved-responses-requests", "public-model", "upstream");
    let cases = vec![
        (
            "custom_tool_calling",
            json!({
                "model": "public-model",
                "input": "hello",
                "tools": [{"type": "custom", "name": "shell", "format": {"type": "text"}}]
            }),
        ),
        (
            "hosted_tools",
            json!({
                "model": "public-model",
                "input": "hello",
                "tools": [{"type": "web_search"}]
            }),
        ),
        (
            "file_input",
            json!({
                "model": "public-model",
                "input": [{"role": "user", "content": [{"type": "input_file", "file_id": "file_test"}]}]
            }),
        ),
        (
            "conversation",
            json!({"model": "public-model", "input": "hello", "conversation": "conv_test"}),
        ),
        (
            "prompt_templates",
            json!({"model": "public-model", "input": "hello", "prompt": {"id": "pmpt_test"}}),
        ),
        (
            "prompt_cache_options",
            json!({"model": "public-model", "input": "hello", "prompt_cache_options": {"type": "ephemeral"}}),
        ),
        (
            "prompt_cache_retention",
            json!({"model": "public-model", "input": "hello", "prompt_cache_retention": "24h"}),
        ),
        (
            "prompt_cache_breakpoint",
            json!({"model": "public-model", "input": [{"type": "prompt_cache_breakpoint"}]}),
        ),
        (
            "context_management",
            json!({"model": "public-model", "input": "hello", "context_management": {}}),
        ),
        (
            "moderation",
            json!({"model": "public-model", "input": "hello", "moderation": {}}),
        ),
    ];

    // Submit each capability-bearing Responses request and prove that it stops before the Provider adapter.
    assert_reserved_requests_unimplemented(&registry, ApiProtocol::Responses, cases);
}

#[test]
fn empty_responses_include_is_an_inactive_value_removed_before_egress() {
    let registry = support::registry("empty-include", "public-model", "upstream");
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "include": []
    }))
    .unwrap();

    // Admit the typed no-op value without claiming support for any non-empty projection.
    let plan = support::prepare(&registry, ApiProtocol::Responses, request.into())
        .expect("an empty include set must not require an interface capability");
    let upstream: Value = serde_json::from_slice(plan.request().body()).unwrap();
    assert!(upstream.get("include").is_none());
}

#[test]
fn unknown_responses_include_fails_as_an_unsupported_capability() {
    let registry = support::registry("unknown-include", "public-model", "upstream");
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "include": ["future.output"]
    }))
    .unwrap();

    // Fail closed during typed analysis instead of treating the whole field as one reserved switch.
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap_err(),
        RequestPlanningError::UnsupportedCapabilities
    ));
}

#[test]
fn undeclared_prompt_cache_key_is_a_parameter_capability_failure() {
    let registry = support::registry("prompt-cache-key", "public-model", "upstream");
    let request = serde_json::to_vec(&json!({
        "model": "public-model",
        "input": "hello",
        "prompt_cache_key": "cache-test"
    }))
    .unwrap();

    // Keep the forwarding option in the ordinary fixed-interface gate when this model does not expose it.
    assert!(matches!(
        support::prepare(&registry, ApiProtocol::Responses, request.into()).unwrap_err(),
        RequestPlanningError::UnsupportedParameter("prompt_cache_key")
    ));
}

/// Submits each reserved request and checks its protocol-specific stable panic text.
fn assert_reserved_requests_unimplemented(
    registry: &openbridge::registry::RuntimeRegistry,
    protocol: ApiProtocol,
    cases: Vec<(&str, Value)>,
) {
    // Serialize each field independently and pass it through the production analysis and planning entry point.
    for (case, request) in cases {
        let body = serde_json::to_vec(&request).expect("reserved request fixture must serialize");
        let error = support::prepare(registry, protocol, body.into())
            .expect_err("reserved request must stop before route planning");
        assert!(
            matches!(error, RequestPlanningError::UnimplementedCapabilities),
            "unexpected request error for {case}: {error:?}"
        );
    }
}

/// Requires registry compilation to stop before a reserved capability becomes executable.
fn assert_registry_compilation_panics(case: &str, action: impl FnOnce()) {
    let result = catch_unwind(AssertUnwindSafe(action));
    assert!(
        result.is_err(),
        "{case} must stop at its fail-closed compilation boundary"
    );
}
