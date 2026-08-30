//! Characterizes the provider-neutral static Generation IR before production wiring exists.

use openbridge::core::ResponseInclude;
use openbridge::ir::generation::{
    AudioResource, BoundedBytes, CacheDirective, CacheKey, CacheRetention, CallId, Candidate,
    CandidateId, ChangeAuthorization, ChangeKind, ChangeReason, ContentPart, ContinuationState,
    FidelityError, FileResource, FinishReason, FunctionTool, GenerationControls, GenerationRequest,
    GenerationResponse, ImageDetail, ImageResource, InlineResource, InputItem, Instruction,
    InstructionAuthority, InstructionOrigin, ItemId, JsonObject, JsonSchema, LossPolicy, MediaType,
    Message, MessageRole, OpaqueExposure, OpaqueKind, OpaqueState, OutputConstraint, OutputItem,
    OutputProjection, ParallelToolCalls, ProviderNamespace, ProviderOrigin, ProviderReference,
    ReasoningEffort, ReasoningItem, ReasoningPart, ReasoningPresence, ReasoningRequest,
    ReasoningSummary, RequestState, Resource, ResourceSource, ResponseId, ResponseMessage,
    ResponseStatus, ResponseValidationError, SemanticChange, SemanticPath, ServerToolConfig,
    ServerToolInput, ServerToolKind, Source, SourceId, SourceLocation, SourceRef,
    StructuredOutputRequirement, TextAnnotation, TextContent, TextValue, ToolCall, ToolChoice,
    ToolChoiceRequirement, ToolDefinition, ToolDirectiveId, ToolExecutor, ToolInput, ToolKind,
    ToolName, ToolOrigin, ToolOutput, ToolPlanId, ToolResult, ToolResultStatus, ToolVisibility,
    Transform, UrlValue, Usage, ValidationError, enforce_loss_policy,
    project_semantic_requirements,
};
use serde_json::json;

fn request_with_user_text() -> GenerationRequest {
    GenerationRequest::new(vec![InputItem::Message(
        Message::new(
            MessageRole::User,
            vec![ContentPart::text(
                TextValue::new("hello", 1_024).expect("message text must fit"),
            )],
        )
        .expect("message must contain content"),
    )])
    .expect("request must be valid")
}

#[test]
fn ordered_input_projects_instruction_and_text_requirements() {
    let instruction = Instruction::new(
        InstructionAuthority::System,
        InstructionOrigin::Downstream,
        TextValue::new("Answer concisely", 1_024).expect("instruction must fit"),
    );
    let message = Message::new(
        MessageRole::User,
        vec![ContentPart::text(
            TextValue::new("hello", 1_024).expect("message text must fit"),
        )],
    )
    .expect("message must contain content");

    let request = GenerationRequest::new(vec![
        InputItem::Instruction(instruction),
        InputItem::Message(message),
    ])
    .expect("ordered request must be valid");

    assert!(matches!(request.input()[0], InputItem::Instruction(_)));
    assert!(matches!(request.input()[1], InputItem::Message(_)));

    let requirements = project_semantic_requirements(&request);
    assert!(requirements.input().instructions());
    assert_eq!(requirements.input().text_parts(), 1);
    assert!(!requirements.tools().function_tools());
    assert!(!requirements.input().image_input());
    assert!(!requirements.input().audio_input());
    assert!(!requirements.input().file_input());
}

#[test]
fn function_tools_and_structured_output_project_typed_requirements() {
    let schema = JsonSchema::new(
        json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false
        }),
        4_096,
    )
    .expect("schema must be a bounded JSON object");
    let tool = ToolDefinition::new(
        ToolName::new("lookup", 64).expect("tool name must fit"),
        ToolOrigin::Downstream,
        ToolExecutor::Client,
        ToolVisibility::Public,
        ToolKind::Function(FunctionTool::new(None, schema.clone(), true)),
    );

    let request = request_with_user_text()
        .with_tools(vec![tool], ToolChoice::Auto, ParallelToolCalls::Allow)
        .expect("tool configuration must be valid")
        .with_output(OutputConstraint::JsonSchema {
            name: TextValue::new("answer", 64).expect("schema name must fit"),
            schema,
            strict: true,
        });

    let requirements = project_semantic_requirements(&request);
    assert!(requirements.tools().function_tools());
    assert!(requirements.tools().strict_function_tools());
    assert_eq!(
        requirements.tools().parallel_tool_calls(),
        ParallelToolCalls::Allow
    );
    assert_eq!(
        requirements.output().structured_output(),
        StructuredOutputRequirement::JsonSchema { strict: true }
    );
}

#[test]
fn bounded_values_and_tool_configuration_reject_invalid_shapes() {
    assert_eq!(TextValue::new("", 8), Err(ValidationError::EmptyText));
    assert_eq!(
        TextValue::new("too large", 3),
        Err(ValidationError::TextTooLarge { max_bytes: 3 })
    );
    assert_eq!(
        JsonSchema::new(json!(["not", "an", "object"]), 128),
        Err(ValidationError::InvalidJsonSchema)
    );

    let schema = JsonSchema::new(json!({ "type": "object" }), 128).expect("schema must fit");
    let named_tool = || {
        ToolDefinition::new(
            ToolName::new("duplicate", 64).expect("tool name must fit"),
            ToolOrigin::Downstream,
            ToolExecutor::Client,
            ToolVisibility::Public,
            ToolKind::Function(FunctionTool::new(None, schema.clone(), false)),
        )
    };
    let error = request_with_user_text()
        .with_tools(
            vec![named_tool(), named_tool()],
            ToolChoice::Auto,
            ParallelToolCalls::Inactive,
        )
        .expect_err("duplicate tool names must fail");
    assert_eq!(
        error,
        ValidationError::DuplicateToolName {
            name: "duplicate".to_owned()
        }
    );
}

#[test]
fn provider_references_and_fidelity_ids_require_origin_and_bounds() {
    let namespace = ProviderNamespace::new("responses", 64).expect("namespace must fit");
    let unbound = OpaqueState::new(
        namespace,
        OpaqueKind::Continuation,
        BoundedBytes::new([1_u8, 2], 64).expect("opaque payload must fit"),
        None,
        OpaqueExposure::InternalOnly,
    )
    .expect("originless downstream state is valid");
    assert!(ProviderReference::new(unbound).is_err());
    assert!(ToolPlanId::new("", 64).is_err());
    assert!(ToolPlanId::new("too-long", 3).is_err());
    assert!(ToolDirectiveId::new("", 64).is_err());
    assert!(InlineResource::new(BoundedBytes::default()).is_err());

    let wrong_origin = ProviderOrigin::new(
        ProviderNamespace::new("other", 64).expect("namespace must fit"),
        "target/api",
        128,
    )
    .expect("origin must fit");
    assert!(
        OpaqueState::new(
            ProviderNamespace::new("responses", 64).expect("namespace must fit"),
            OpaqueKind::Session,
            BoundedBytes::new([4_u8], 64).expect("state must fit"),
            Some(wrong_origin),
            OpaqueExposure::InternalOnly,
        )
        .is_err()
    );
    let wrong_kind = OpaqueState::new(
        ProviderNamespace::new("responses", 64).expect("namespace must fit"),
        OpaqueKind::ThoughtSignature,
        BoundedBytes::new([5_u8], 64).expect("state must fit"),
        None,
        OpaqueExposure::InternalOnly,
    )
    .expect("originless downstream state is valid");
    assert!(ContinuationState::new(wrong_kind).is_err());

    let continuation_only = GenerationRequest::new(Vec::new())
        .expect("static IR permits continuation-only input")
        .with_state(RequestState::new(
            Some(
                ContinuationState::new(
                    OpaqueState::new(
                        ProviderNamespace::new("responses", 64).expect("namespace must fit"),
                        OpaqueKind::Continuation,
                        BoundedBytes::new([3_u8], 64).expect("continuation must fit"),
                        None,
                        OpaqueExposure::InternalOnly,
                    )
                    .expect("originless downstream continuation is valid"),
                )
                .expect("continuation kind must be valid"),
            ),
            None,
            false,
        ));
    assert!(continuation_only.input().is_empty());
    assert!(continuation_only.state().continuation().is_some());
}

#[test]
fn media_reasoning_controls_and_state_project_without_wire_facts() {
    let image = Resource::Image(ImageResource::new(
        ResourceSource::Url(
            UrlValue::new("https://example.invalid/image.png", 2_048).expect("URL must fit"),
        ),
        Some(MediaType::new("image/png", 64).expect("media type must fit")),
        Some(ImageDetail::High),
    ));
    let audio = Resource::Audio(AudioResource::new(
        ResourceSource::Inline(
            InlineResource::new(BoundedBytes::new([1_u8, 2, 3], 64).expect("audio must fit"))
                .expect("inline audio must be non-empty"),
        ),
        Some(MediaType::new("audio/wav", 64).expect("media type must fit")),
    ));
    let file = Resource::File(FileResource::new(
        ResourceSource::Url(
            UrlValue::new("https://example.invalid/document.pdf", 2_048).expect("URL must fit"),
        ),
        Some(MediaType::new("application/pdf", 64).expect("media type must fit")),
    ));
    let message = Message::new(
        MessageRole::User,
        vec![
            ContentPart::Resource(image),
            ContentPart::Resource(audio),
            ContentPart::Resource(file),
        ],
    )
    .expect("message must contain resources");

    let namespace = ProviderNamespace::new("responses", 64).expect("namespace must fit");
    let origin =
        ProviderOrigin::new(namespace.clone(), "target/api", 128).expect("origin must fit");
    let continuation = OpaqueState::new(
        namespace,
        OpaqueKind::Continuation,
        BoundedBytes::new(b"resp_opaque".to_vec(), 128).expect("state must fit"),
        Some(origin),
        OpaqueExposure::Returnable,
    )
    .expect("origin namespace must match");
    let continuation = ContinuationState::new(continuation).expect("kind must be continuation");
    let request = GenerationRequest::new(vec![InputItem::Message(message)])
        .expect("request must be valid")
        .with_reasoning(ReasoningRequest::new(
            ReasoningEffort::High,
            ReasoningSummary::Auto,
        ))
        .with_controls(GenerationControls::new(Some(512), Some(2)).expect("controls must be valid"))
        .expect("controls must match configured tools")
        .with_state(RequestState::new(Some(continuation), None, false));

    let requirements = project_semantic_requirements(&request);
    assert!(requirements.input().image_input());
    assert!(requirements.input().audio_input());
    assert!(requirements.input().file_input());
    assert_eq!(requirements.input().image().count(), 1);
    assert_eq!(requirements.input().image().url_sources(), 1);
    assert!(
        requirements
            .input()
            .image()
            .image_details()
            .contains(&ImageDetail::High)
    );
    assert_eq!(requirements.input().audio().inline_sources(), 1);
    assert_eq!(requirements.input().audio().total_inline_bytes(), 3);
    assert_eq!(
        requirements.reasoning().presence(),
        ReasoningPresence::Present
    );
    assert_eq!(requirements.reasoning().effort(), ReasoningEffort::High);
    assert_eq!(requirements.reasoning().summary(), ReasoningSummary::Auto);
    assert_eq!(requirements.controls().max_output_tokens(), Some(512));
    assert_eq!(requirements.controls().candidate_count(), Some(2));
    assert!(requirements.state().continuation());
    assert!(!requirements.state().background());
}

#[test]
fn response_preserves_ordered_items_and_rejects_duplicate_identity() {
    let item_id = ItemId::new("item-1", 64).expect("item ID must fit");
    let message = OutputItem::Message(
        ResponseMessage::new(
            item_id.clone(),
            vec![ContentPart::text(
                TextValue::new("answer", 128).expect("text must fit"),
            )],
            None,
        )
        .expect("message output must be valid"),
    );
    let reasoning = OutputItem::Reasoning(
        ReasoningItem::new(
            ItemId::new("item-2", 64).expect("item ID must fit"),
            vec![ReasoningPart::Summary(
                TextValue::new("summary", 128).expect("summary must fit"),
            )],
            None,
        )
        .expect("reasoning output must be valid"),
    );
    let candidate = Candidate::new(
        CandidateId::new("candidate-0", 64).expect("candidate ID must fit"),
        vec![message, reasoning],
        Some(FinishReason::Stop),
    )
    .expect("candidate must be valid");
    let response = GenerationResponse::new(
        ResponseId::new("response-1", 64).expect("response ID must fit"),
        vec![candidate],
        ResponseStatus::Completed,
        None,
        Vec::new(),
    )
    .expect("response must be valid");
    assert!(matches!(
        response.candidates()[0].output()[0],
        OutputItem::Message(_)
    ));
    assert!(matches!(
        response.candidates()[0].output()[1],
        OutputItem::Reasoning(_)
    ));

    let duplicate = Candidate::new(
        CandidateId::new("candidate-1", 64).expect("candidate ID must fit"),
        vec![
            OutputItem::Message(
                ResponseMessage::new(
                    item_id.clone(),
                    vec![ContentPart::text(
                        TextValue::new("one", 128).expect("text must fit"),
                    )],
                    None,
                )
                .expect("message must be valid"),
            ),
            OutputItem::Message(
                ResponseMessage::new(
                    item_id,
                    vec![ContentPart::text(
                        TextValue::new("two", 128).expect("text must fit"),
                    )],
                    None,
                )
                .expect("message must be valid"),
            ),
        ],
        Some(FinishReason::Stop),
    );
    assert!(matches!(
        duplicate,
        Err(ResponseValidationError::DuplicateItemId { .. })
    ));
}

#[test]
fn fidelity_rejects_unscoped_loss_by_default() {
    let transform = Transform::new(
        "lowered",
        vec![SemanticChange::new(
            SemanticPath::new("input[0].tool"),
            ChangeKind::Lossy,
            ChangeReason::SemanticOmission,
            ChangeAuthorization::default(),
        )],
    );

    assert!(matches!(
        enforce_loss_policy(&transform, LossPolicy::Reject),
        Err(FidelityError::LossRejected { .. })
    ));
    assert!(enforce_loss_policy(&transform, LossPolicy::Allow).is_ok());
}

#[test]
fn ordered_tool_history_and_server_tools_project_without_flattening() {
    let schema = JsonSchema::new(json!({ "type": "object" }), 128).expect("schema must fit");
    let function_name = ToolName::new("lookup", 64).expect("tool name must fit");
    let function = ToolDefinition::new(
        function_name.clone(),
        ToolOrigin::Downstream,
        ToolExecutor::Client,
        ToolVisibility::Public,
        ToolKind::Function(FunctionTool::new(None, schema, false)),
    );
    let namespace = ProviderNamespace::new("responses", 64).expect("namespace must fit");
    let origin = ProviderOrigin::new(namespace, "target/api", 128).expect("origin must be bounded");
    let web_search = ToolDefinition::new(
        ToolName::new("web_search", 64).expect("tool name must fit"),
        ToolOrigin::GatewayPolicy(
            ToolPlanId::new("plan-1", 64).expect("tool-plan identity must fit"),
        ),
        ToolExecutor::Provider(origin),
        ToolVisibility::Internal,
        ToolKind::Server(ServerToolConfig::WebSearch),
    );
    let server_input = ServerToolInput::new(
        ServerToolKind::WebSearch,
        JsonObject::new(json!({ "query": "weather" }), 256).expect("server input must fit"),
    );
    assert_eq!(server_input.kind(), ServerToolKind::WebSearch);
    let call_id = CallId::new("call-1", 64).expect("call identity must fit");
    let call = ToolCall::new(
        ItemId::new("call-item", 64).expect("item identity must fit"),
        call_id.clone(),
        ToolName::new("web_search", 64).expect("tool name must fit"),
        ToolInput::Server(server_input),
        None,
    );
    let result = ToolResult::new(
        ItemId::new("result-item", 64).expect("item identity must fit"),
        call_id,
        ToolResultStatus::Success,
        vec![
            ToolOutput::Text(TextValue::new("sunny", 128).expect("tool output must fit")),
            ToolOutput::Resource(Resource::Image(ImageResource::new(
                ResourceSource::Url(
                    UrlValue::new("https://example.test/map.png", 2_048).expect("URL must fit"),
                ),
                Some(MediaType::new("image/png", 64).expect("media type must fit")),
                None,
            ))),
        ],
        None,
    );

    let request = GenerationRequest::new(vec![
        request_with_user_text().input()[0].clone(),
        InputItem::PriorToolCall(call),
        InputItem::ToolResult(result),
    ])
    .expect("ordered history must be valid")
    .with_tools(
        vec![function, web_search],
        ToolChoice::Specific(ToolName::new("lookup", 64).expect("tool name must fit")),
        ParallelToolCalls::RequireSerial,
    )
    .expect("tool configuration must be valid");

    assert!(matches!(request.input()[1], InputItem::PriorToolCall(_)));
    assert!(matches!(request.input()[2], InputItem::ToolResult(_)));
    let requirements = project_semantic_requirements(&request);
    assert!(requirements.tools().function_tools());
    assert!(requirements.tools().tool_history());
    assert_eq!(requirements.tools().choice(), ToolChoiceRequirement::Named);
    assert_eq!(
        requirements.tools().parallel_tool_calls(),
        ParallelToolCalls::RequireSerial
    );
    assert!(
        requirements
            .tools()
            .server_tools()
            .contains(&ServerToolKind::WebSearch)
    );
    assert!(
        requirements
            .tools()
            .server_history()
            .contains(&ServerToolKind::WebSearch)
    );
    assert_eq!(requirements.input().image().count(), 1);
}

#[test]
fn controls_cache_and_output_projection_preserve_explicit_values() {
    let controls = GenerationControls::new(Some(1_024), Some(3))
        .expect("limits must be valid")
        .with_sampling(Some(0.0), Some(0.95), Some(40))
        .expect("sampling controls must be finite")
        .with_stop(Some(Vec::new()))
        .with_seed(Some(7))
        .with_penalties(Some(-0.5), Some(0.25))
        .expect("penalties must be finite");
    let cache = CacheDirective::new(
        Some(CacheKey::new("prompt-v1", 64).expect("cache key must fit")),
        Some(CacheRetention::Hours24),
    );
    let request = request_with_user_text()
        .with_controls(controls)
        .expect("inactive tool controls must be valid")
        .with_state(RequestState::new(None, Some(cache), false))
        .with_output_projection(OutputProjection::new([
            ResponseInclude::WebSearchCallSources,
            ResponseInclude::ReasoningEncryptedContent,
        ]));

    let requirements = project_semantic_requirements(&request);
    assert_eq!(requirements.controls().max_output_tokens(), Some(1_024));
    assert_eq!(requirements.controls().candidate_count(), Some(3));
    assert_eq!(requirements.controls().temperature(), Some(0.0));
    assert_eq!(requirements.controls().top_p(), Some(0.95));
    assert_eq!(requirements.controls().top_k(), Some(40));
    assert_eq!(requirements.controls().stop(), Some([].as_slice()));
    assert_eq!(requirements.controls().seed(), Some(7));
    assert_eq!(requirements.controls().frequency_penalty(), Some(-0.5));
    assert_eq!(requirements.controls().presence_penalty(), Some(0.25));
    assert!(requirements.state().cache());
    assert!(
        requirements
            .output()
            .includes()
            .contains(&ResponseInclude::WebSearchCallSources)
    );
    assert_eq!(
        project_semantic_requirements(&request_with_user_text())
            .controls()
            .temperature(),
        None
    );
    assert!(
        GenerationControls::default()
            .with_sampling(Some(f64::NAN), None, None)
            .is_err()
    );
}

#[test]
fn failed_response_can_omit_candidates_and_finish_without_fabrication() {
    let response = GenerationResponse::new(
        ResponseId::new("response-failed", 64).expect("response identity must fit"),
        Vec::new(),
        ResponseStatus::Failed,
        Some(Usage::new(Some(10), Some(2), Some(12), None, None)),
        Vec::new(),
    )
    .expect("failed response without candidates is valid");

    assert!(response.candidates().is_empty());
    assert_eq!(response.status(), ResponseStatus::Failed);
    assert_eq!(
        response.usage().and_then(|usage| usage.total_tokens()),
        Some(12)
    );

    let incomplete = Candidate::new(
        CandidateId::new("candidate-incomplete", 64).expect("candidate identity must fit"),
        Vec::new(),
        None,
    )
    .expect("candidate without a fabricated finish reason is valid");
    assert_eq!(incomplete.finish(), None);
}

#[test]
fn citations_reference_ordered_source_items_without_raw_annotations() {
    let source_id = SourceId::new("source-1", 64).expect("source identity must fit");
    let source = Source::new(
        ItemId::new("source-item", 64).expect("item identity must fit"),
        source_id.clone(),
        Some(TextValue::new("Weather report", 128).expect("title must fit")),
        SourceLocation::Url(
            UrlValue::new("https://example.test/weather", 2_048).expect("URL must fit"),
        ),
        None,
    );
    let message = ResponseMessage::new(
        ItemId::new("message-item", 64).expect("item identity must fit"),
        vec![ContentPart::Text(TextContent::new(
            TextValue::new("It is sunny.", 128).expect("text must fit"),
            vec![TextAnnotation::Citation(SourceRef::new(source_id))],
        ))],
        None,
    )
    .expect("message must be valid");
    let candidate = Candidate::new(
        CandidateId::new("candidate-source", 64).expect("candidate identity must fit"),
        vec![OutputItem::Message(message), OutputItem::Source(source)],
        Some(FinishReason::Stop),
    )
    .expect("source output must be valid");

    assert!(matches!(candidate.output()[0], OutputItem::Message(_)));
    assert!(matches!(candidate.output()[1], OutputItem::Source(_)));

    let dangling_id = SourceId::new("missing-source", 64).expect("source identity must fit");
    let dangling = Candidate::new(
        CandidateId::new("candidate-dangling", 64).expect("candidate identity must fit"),
        vec![OutputItem::Message(
            ResponseMessage::new(
                ItemId::new("dangling-message", 64).expect("item identity must fit"),
                vec![ContentPart::Text(TextContent::new(
                    TextValue::new("uncited", 128).expect("text must fit"),
                    vec![TextAnnotation::Citation(SourceRef::new(dangling_id))],
                ))],
                None,
            )
            .expect("message must be valid"),
        )],
        Some(FinishReason::Stop),
    );
    assert!(matches!(
        dangling,
        Err(ResponseValidationError::UnknownSourceReference { .. })
    ));
}
