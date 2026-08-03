//! 验证 canonical Model 与 Chat/Responses capability 的协议命名和预留边界。

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};

use openbridge::{
    core::{
        ApiProtocol, ChatCompletionsCapabilities, HostedToolKind, ResponseInclude,
        ResponsesCapabilities,
    },
    pipeline::RequestPlanningError,
    registry::{
        InputModality, ModelConfig, ModelMode, OutputModality, UpstreamApiCapabilities,
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

type ModelReservation = fn(&mut ModelConfig);
type ChatReservation = fn(&mut ChatCompletionsCapabilities);
type ResponsesReservation = fn(&mut ResponsesCapabilities);

#[test]
fn definitions_expose_protocol_specific_reserved_fields() {
    // 构造 canonical Model 的预留 mode 与输入输出模态。
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

    // 构造 Chat Completions 尚未进入请求路径的标准能力位置。
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

    // 构造 Responses 尚未进入请求路径的 tool、状态与附加输出位置。
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
fn every_model_reservation_stops_at_the_processing_boundary() {
    let cases: [(&str, ModelReservation); 3] = [
        ("mode", |model| model.mode = Some(ModelMode::Chat)),
        ("input_modalities", |model| {
            model.input_modalities = Some(vec![InputModality::Text])
        }),
        ("output_modalities", |model| {
            model.output_modalities = Some(vec![OutputModality::Text])
        }),
    ];

    // 逐项构造独立 definition，避免前一个预留字段掩盖后一个字段的失败边界。
    for (case, configure) in cases {
        let mut definition = support::definition(case, "public-model", "upstream");
        configure(&mut definition.models[0]);

        assert_unimplemented(
            case,
            "model mode and modality processing is not implemented",
            move || {
                let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
            },
        );
    }
}

#[test]
fn every_chat_reservation_stops_before_registry_compilation() {
    let cases: [(&str, ChatReservation); 10] = [
        ("custom_tool_calling", |capabilities| {
            capabilities.custom_tool_calling = true
        }),
        ("audio_input", |capabilities| {
            capabilities.audio_input = true
        }),
        ("file_input", |capabilities| capabilities.file_input = true),
        ("audio_output", |capabilities| {
            capabilities.audio_output = true
        }),
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

    // 逐项启用 Chat capability，并确认任何单字段都不能进入 runtime registry。
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

    // 逐项启用 Responses capability，并确认任何单字段都不能进入 runtime registry。
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
            "audio_input",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": [{"type": "input_audio", "input_audio": {"data": "synthetic", "format": "wav"}}]}]
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
            "audio_output",
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "modalities": ["text", "audio"],
                "audio": {"format": "wav", "voice": "alloy"}
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

    // 逐项提交 Chat 请求，证明 Native 路径不会把预留字段透传到 Provider adapter。
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

    // 逐项提交 Responses 请求，证明 Native 路径不会把预留字段透传到 Provider adapter。
    assert_reserved_requests_unimplemented(&registry, ApiProtocol::Responses, cases);
}

/// 逐项提交预留请求并核对协议专有的稳定 panic 文本。
fn assert_reserved_requests_unimplemented(
    registry: &openbridge::registry::RuntimeRegistry,
    protocol: ApiProtocol,
    cases: Vec<(&str, Value)>,
) {
    // 为每个字段独立序列化并进入生产请求分析/规划入口。
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

/// 捕获并核对预留接口的稳定 `unimplemented!` 文本。
fn assert_unimplemented(case: &str, expected: &str, action: impl FnOnce()) {
    // 捕获 panic，避免一个字段阻止同组其他预留字段得到验证。
    let panic = match catch_unwind(AssertUnwindSafe(action)) {
        Ok(()) => panic!("{case} must stop at its unimplemented boundary"),
        Err(panic) => panic,
    };

    // 归一化 Rust 常见 panic payload 并核对稳定说明。
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
