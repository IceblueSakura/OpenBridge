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
fn definitions_expose_protocol_specific_reserved_fields() {
    // Build reserved canonical Model modes and input/output modalities.
    let mut definition = support::definition("reserved-model-facts", "public-model", "upstream");
    let model = &mut definition.models[0];
    model.mode = Some(ModelMode::Chat);
    model.input_modalities = Some(vec![
        InputModality::Text,
        InputModality::Image,
        InputModality::Audio,
        InputModality::File,
    ]);
    model.output_modalities = Some(vec![
        OutputModality::Text,
        OutputModality::Image,
        OutputModality::Audio,
    ]);

    // Build standard Chat Completions capability positions not yet on the request path.
    let chat = ChatCompletionsCapabilities {
        custom_tool_calling: true,
        audio_input: true,
        file_input: true,
        audio_output: true,
        predicted_outputs: true,
        web_search: true,
        prompt_caching: true,
        moderation: true,
        logprobs: true,
        multiple_choices: true,
        ..ChatCompletionsCapabilities::default()
    };

    // Build Responses tool, state, and additional-output positions not yet on the request path.
    let responses = ResponsesCapabilities {
        custom_tool_calling: true,
        hosted_tools: HOSTED_TOOLS,
        file_input: true,
        conversation: true,
        prompt_templates: true,
        prompt_caching: true,
        context_management: true,
        include: INCLUDES,
        moderation: true,
        logprobs: true,
        ..ResponsesCapabilities::default()
    };

    assert_eq!(model.mode, Some(ModelMode::Chat));
    assert_eq!(model.input_modalities.as_deref().unwrap().len(), 4);
    assert_eq!(model.output_modalities.as_deref().unwrap().len(), 3);
    assert!(chat.custom_tool_calling && chat.audio_output && chat.predicted_outputs);
    assert_eq!(responses.hosted_tools, HOSTED_TOOLS);
    assert_eq!(responses.include, INCLUDES);
    assert!(responses.conversation && responses.context_management);
}

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
fn every_chat_reservation_stops_before_registry_compilation() {
    let cases: [(&str, ChatReservation); 8] = [
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

    // Enable each Chat capability and confirm that no individual field enters the runtime registry.
    for (case, configure) in cases {
        let mut definition = support::definition(case, "public-model", "upstream");
        let UpstreamApiCapabilities::ChatCompletions(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[0].capabilities
        else {
            panic!("fixture must expose a Chat Completions upstream API");
        };
        configure(capabilities);

        assert_unimplemented(
            case,
            "reserved Chat Completions capabilities are not implemented",
            move || {
                let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
            },
        );
    }
}

#[test]
fn every_responses_reservation_stops_before_registry_compilation() {
    let cases: [(&str, ResponsesReservation); 10] = [
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

    // Enable each Responses capability and confirm that no individual field enters the runtime registry.
    for (case, configure) in cases {
        let mut definition = support::definition(case, "public-model", "upstream");
        let UpstreamApiCapabilities::Responses(capabilities) =
            &mut definition.upstream_targets[0].upstream_apis[1].capabilities
        else {
            panic!("fixture must expose a Responses upstream API");
        };
        configure(capabilities);

        assert_unimplemented(
            case,
            "reserved Responses capabilities are not implemented",
            move || {
                let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
            },
        );
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
        (
            "logprobs",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "logprobs": true
            }),
        ),
        (
            "multiple_choices",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "n": 2
            }),
        ),
    ];

    // Submit each Chat request and prove that the Native path does not pass reserved fields to the Provider adapter.
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
        (
            "logprobs",
            json!({"model": "public-model", "input": "hello", "top_logprobs": 1}),
        ),
    ];

    // Submit each Responses request and prove that the Native path does not pass reserved fields to the Provider adapter.
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

/// Captures and checks the stable `unimplemented!` text for the reserved interface.
fn assert_unimplemented(case: &str, expected: &str, action: impl FnOnce()) {
    // Capture the panic so one field does not prevent the remaining fields from being checked.
    let panic = match catch_unwind(AssertUnwindSafe(action)) {
        Ok(()) => panic!("{case} must stop at its unimplemented boundary"),
        Err(panic) => panic,
    };

    // Normalize common Rust panic payloads and check the stable message.
    let message = if let Some(message) = panic.downcast_ref::<&str>() {
        *message
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.as_str()
    } else {
        panic!("{case} produced a non-string panic payload");
    };
    let expected = format!("not implemented: {expected}");
    assert_eq!(message, expected, "unexpected panic for {case}");
}
