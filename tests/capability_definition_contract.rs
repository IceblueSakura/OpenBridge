//! Verifies protocol naming and reserved boundaries for canonical Model and Chat/Responses capabilities.

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use openbridge::{
    core::{
        ApiProtocol, ChatCompletionsCapabilities, HostedToolKind, ResponseInclude,
        ResponsesCapabilities,
    },
    pipeline::RequestPlanningError,
    registry::{InputModality, ModelMode, OutputModality, UpstreamApiCapabilities, build_registry},
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

#[test]
fn canonical_model_mode_and_modalities_compile_into_public_model_information() {
    let mut definition = support::definition("model-facts", "public-model", "upstream");
    definition.models[0].mode = Some(ModelMode::Chat);
    definition.models[0].input_modalities = Some(vec![
        InputModality::Text,
        InputModality::Image,
        InputModality::Audio,
        InputModality::File,
    ]);
    definition.models[0].output_modalities = Some(vec![
        OutputModality::Text,
        OutputModality::Image,
        OutputModality::Audio,
    ]);

    // Compile canonical facts and confirm that model ceilings remain separate from interface capabilities.
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();
    let model = registry.model("openai/test-model").unwrap();
    assert_eq!(model.mode(), Some(ModelMode::Chat));
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
    let chat_cases: [(&str, ChatReservation); 8] = [
        ("custom_tool_calling", |capabilities| {
            capabilities.custom_tool_calling = true
        }),
        ("file_input", |capabilities| capabilities.file_input = true),
        ("predicted_outputs", |capabilities| {
            capabilities.predicted_outputs = true
        }),
        ("web_search", |capabilities| capabilities.web_search = true),
        ("prompt_caching", |capabilities| {
            capabilities.prompt_caching = true
        }),
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

    let responses_cases: [(&str, ResponsesReservation); 10] = [
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
        ("prompt_caching", |capabilities| {
            capabilities.prompt_caching = true
        }),
        ("context_management", |capabilities| {
            capabilities.context_management = true
        }),
        ("include", |capabilities| capabilities.include = INCLUDES),
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
            "prompt_caching",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "prompt_cache_key": "cache-test"
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
            "prompt_caching",
            json!({"model": "public-model", "input": "hello", "prompt_cache_key": "cache-test"}),
        ),
        (
            "context_management",
            json!({"model": "public-model", "input": "hello", "context_management": {}}),
        ),
        (
            "include",
            json!({"model": "public-model", "input": "hello", "include": ["message.output_text.logprobs"]}),
        ),
        (
            "moderation",
            json!({"model": "public-model", "input": "hello", "moderation": {}}),
        ),
    ];

    // Submit each capability-bearing Responses request and prove that it stops before the Provider adapter.
    assert_reserved_requests_unimplemented(&registry, ApiProtocol::Responses, cases);
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
