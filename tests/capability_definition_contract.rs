//! 验证 canonical Model 与 Chat/Responses capability 的协议命名和预留边界。

mod support;

use openbridge::{
    core::{ChatCompletionsCapabilities, HostedToolKind, ResponseInclude, ResponsesCapabilities},
    registry::{InputModality, ModelMode, OutputModality, UpstreamApiCapabilities, build_registry},
};

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
#[should_panic(expected = "model mode and modality processing is not implemented")]
fn configured_model_mode_stops_at_the_reserved_processing_boundary() {
    // 启用预留模型字段，并确认 registry 不会把它误编译成现有能力。
    let mut definition = support::definition("reserved-model-mode", "public-model", "upstream");
    definition.models[0].mode = Some(ModelMode::Chat);

    let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
}

#[test]
#[should_panic(expected = "reserved Chat Completions capabilities are not implemented")]
fn configured_chat_reservation_stops_before_registry_compilation() {
    // 启用预留 Chat 字段，并确认 Provider contract 子集校验不会把它视为已实现。
    let mut definition = support::definition("reserved-chat", "public-model", "upstream");
    let UpstreamApiCapabilities::ChatCompletions(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    else {
        panic!("fixture must expose a Chat Completions upstream API");
    };
    capabilities.audio_input = true;

    let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
}

#[test]
#[should_panic(expected = "reserved Responses capabilities are not implemented")]
fn configured_responses_reservation_stops_before_registry_compilation() {
    // 启用预留 Responses 字段，并确认 Provider contract 子集校验不会把它视为已实现。
    let mut definition = support::definition("reserved-responses", "public-model", "upstream");
    let UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("fixture must expose a Responses upstream API");
    };
    capabilities.hosted_tools = &[HostedToolKind::WebSearch];

    let _ = build_registry(support::bootstrap(support::BOOTSTRAP), definition);
}
