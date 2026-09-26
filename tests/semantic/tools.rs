//! Independent oracles for the function-tool migration, without providers or production routing.
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, RepresentationError, lower_request,
        lower_response,
    },
    protocol::{
        fidelity::FidelityRecords,
        openai::{DecodedRequest, Profile, ResponseMetadata, chat, responses},
    },
    semantic::{task::generation::*, value::Text},
};
use serde_json::{Value, json};

fn text(s: &str) -> Text {
    Text::new(s, "fixture", 1024).unwrap()
}
fn chat_history() -> Value {
    json!({"messages":[
        {"role":"user","content":"weather?"},
        {"role":"assistant","content":"Checking.","tool_calls":[
            {"id":"call_a","type":"function","function":{"name":"weather","arguments":"{ \"city\": \"Paris\" }"}},
            {"id":"call_b","type":"function","function":{"name":"weather","arguments":"{\"city\":\"Rome\"}"}}
        ]},
        {"role":"tool","tool_call_id":"call_b","content":""},
        {"role":"tool","tool_call_id":"call_a","content":"sunny"}
    ],"tools":[{"type":"function","function":{"name":"weather","description":"","parameters":{"type":"object","properties":{"city":{"type":"string"}}},"strict":false}}],
    "tool_choice":{"type":"function","function":{"name":"weather"}},"parallel_tool_calls":false})
}
fn request_wire(d: &DecodedRequest, profile: Profile) -> Value {
    let t = lower_request(&d.semantic, &d.fidelity, profile, Contract::full()).unwrap();
    match profile {
        Profile::Chat => chat::encode_generation(&t).unwrap(),
        Profile::Responses => responses::encode_generation(&t).unwrap(),
    }
}
fn expected_responses_history() -> Value {
    json!({"input":[
        {"type":"message","role":"user","content":[{"type":"input_text","text":"weather?"}]},
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Checking."}]},
        {"type":"function_call","call_id":"call_a","name":"weather","arguments":"{ \"city\": \"Paris\" }"},
        {"type":"function_call","call_id":"call_b","name":"weather","arguments":"{\"city\":\"Rome\"}"},
        {"type":"function_call_output","call_id":"call_b","output":""},
        {"type":"function_call_output","call_id":"call_a","output":"sunny"}
    ],"tools":[{"type":"function","name":"weather","description":"","parameters":{"type":"object","properties":{"city":{"type":"string"}}},"strict":false}],
    "tool_choice":{"type":"function","name":"weather"},"parallel_tool_calls":false})
}
#[test]
fn independent_decode_preserves_function_meaning_and_message_ownership() {
    let d = chat::decode_generation(&chat_history()).unwrap();
    assert_eq!(d.semantic.items().len(), 6);
    let (_, Item::ToolCall(call)) = &d.semantic.items()[2] else {
        panic!("call")
    };
    assert_eq!(
        call,
        &ToolCall {
            call_id: text("call_a"),
            name: text("weather"),
            arguments: "{ \"city\": \"Paris\" }".into(),
            message: Some(ItemId::new(2)),
            status: ItemLifecycle::Completed,
        }
    );
    let ToolDefinition::Function(def) = &d.semantic.tools()[0] else {
        panic!("function");
    };
    assert_eq!(def.description.as_deref(), Some(""));
    assert_eq!(def.strict, FunctionStrictness::Explicit(false));
    assert_eq!(
        def.parameters,
        Some(json!({"type":"object","properties":{"city":{"type":"string"}}}))
    );
    assert_eq!(
        d.semantic.tool_choice(),
        Some(&ToolChoice::Specific(text("weather")))
    );
    let q = GenerationRequirements::derive(&d.semantic);
    assert!(q.tool_history);
    assert_eq!(q.parallel_tool_calls, Some(false));
    assert!(!q.strict_function_tools);
    assert_eq!(request_wire(&d, Profile::Chat), chat_history());
    assert_eq!(
        request_wire(&d, Profile::Responses),
        expected_responses_history()
    );
}
#[test]
fn independent_ir_encodes_call_and_empty_result_without_source_wire() {
    let items = vec![
        (
            ItemId::new(10),
            Item::ToolCall(ToolCall {
                call_id: text("call_new"),
                name: text("lookup"),
                arguments: "not valid JSON".into(),
                message: None,
                status: ItemLifecycle::Completed,
            }),
        ),
        (
            ItemId::new(20),
            Item::ToolResult(ToolResult {
                call_id: text("call_new"),
                output: "".into(),
                status: None,
            }),
        ),
    ];
    let d = DecodedRequest {
        semantic: GenerationRequest::new(items, GenerationControls::default()).unwrap(),
        fidelity: FidelityRecords::default(),
    };
    assert_eq!(
        request_wire(&d, Profile::Chat),
        json!({"messages":[
            {"role":"assistant","content":null,"tool_calls":[{"id":"call_new","type":"function","function":{"name":"lookup","arguments":"not valid JSON"}}]},
            {"role":"tool","tool_call_id":"call_new","content":""}
        ]})
    );
    assert_eq!(
        request_wire(&d, Profile::Responses),
        json!({"input":[
            {"type":"function_call","call_id":"call_new","name":"lookup","arguments":"not valid JSON"},
            {"type":"function_call_output","call_id":"call_new","output":""}
        ]})
    );
}
#[test]
fn responses_decode_uses_call_ids_not_wire_ids_or_result_positions() {
    let mut wire = expected_responses_history();
    wire["input"][2]["id"] = json!("wire_a");
    wire["input"][3]["id"] = json!("wire_b");
    let d = responses::decode_generation(&wire).unwrap();
    let (_, Item::ToolCall(call)) = &d.semantic.items()[2] else {
        panic!("call")
    };
    assert_eq!(call.call_id.as_str(), "call_a");
    assert_eq!(call.message, None);
    assert_eq!(d.fidelity.response_item_id(ItemId::new(3)), Some("wire_a"));
    assert_eq!(request_wire(&d, Profile::Responses), wire);
    let chat = request_wire(&d, Profile::Chat);
    // The named call-run normalization merges only independent calls, never adjacent text.
    assert_eq!(
        chat["messages"][1],
        json!({"role":"assistant","content":"Checking."})
    );
    assert_eq!(
        chat["messages"][2]["tool_calls"].as_array().unwrap().len(),
        2
    );
    assert_eq!(chat["messages"][3]["tool_call_id"], "call_b");
}
#[test]
fn replacement_insertion_and_reordering_drive_both_encoders() {
    let mut d = chat::decode_generation(&chat_history()).unwrap();
    let mut items = d.semantic.items().to_vec();
    if let Item::ToolCall(c) = &mut items[2].1 {
        c.arguments = "{\"city\":\"Tokyo\"}".into();
    }
    items.swap(2, 3);
    items.insert(
        4,
        (
            ItemId::new(99),
            Item::ToolCall(ToolCall {
                call_id: text("call_c"),
                name: text("weather"),
                arguments: "{}".into(),
                message: Some(ItemId::new(2)),
                status: ItemLifecycle::Completed,
            }),
        ),
    );
    items.push((
        ItemId::new(100),
        Item::ToolResult(ToolResult {
            call_id: text("call_c"),
            output: "rain".into(),
            status: None,
        }),
    ));
    d.semantic = d.semantic.with_items(items).unwrap();
    let c = request_wire(&d, Profile::Chat);
    let r = request_wire(&d, Profile::Responses);
    assert_eq!(c["messages"][1]["tool_calls"][0]["id"], "call_b");
    assert_eq!(
        c["messages"][1]["tool_calls"][1]["function"]["arguments"],
        "{\"city\":\"Tokyo\"}"
    );
    assert_eq!(c["messages"][1]["tool_calls"][2]["id"], "call_c");
    assert_eq!(r["input"][2]["call_id"], "call_b");
    assert_eq!(r["input"][3]["arguments"], "{\"city\":\"Tokyo\"}");
    assert_eq!(r["input"][4]["call_id"], "call_c");
}
#[test]
fn deletion_cannot_be_undone_by_stale_fidelity_and_requirements_follow_final_ir() {
    let mut wire = expected_responses_history();
    wire["input"][2]["id"] = json!("old_call_item");
    let mut d = responses::decode_generation(&wire).unwrap();
    d.semantic = d
        .semantic
        .retain_items(|_, item| matches!(item, Item::Message(m) if m.role == MessageRole::User))
        .unwrap()
        .with_tool_settings(None, None, None)
        .unwrap();
    assert_eq!(
        d.fidelity.response_item_id(ItemId::new(3)),
        Some("old_call_item")
    );
    let q = GenerationRequirements::derive(&d.semantic);
    assert_eq!(q.tool_count, 0);
    assert!(!q.tool_history);
    assert_eq!(q.parallel_tool_calls, None);
    assert_eq!(q.tool_choice, None);
    assert_eq!(
        request_wire(&d, Profile::Responses),
        json!({"input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"weather?"}]}]})
    );
    assert_eq!(
        request_wire(&d, Profile::Chat),
        json!({"messages":[{"role":"user","content":"weather?"}]})
    );
}
#[test]
fn dangling_duplicate_and_reordered_relationships_fail_semantic_validation() {
    let d = chat::decode_generation(&chat_history()).unwrap();
    assert!(matches!(
        d.semantic
            .clone()
            .retain_items(|id, _| id != ItemId::new(3)),
        Err(GenerationError::InvalidToolResult)
    ));
    assert!(matches!(
        d.semantic
            .clone()
            .retain_items(|id, _| id != ItemId::new(2)),
        Err(GenerationError::InvalidMessageGroup)
    ));
    let mut items = d.semantic.items().to_vec();
    items.swap(2, 5);
    assert!(d.semantic.clone().with_items(items).is_err());
    let mut items = d.semantic.items().to_vec();
    if let Item::ToolCall(c) = &mut items[3].1 {
        c.call_id = text("call_a");
    }
    assert!(matches!(
        d.semantic.clone().with_items(items),
        Err(GenerationError::DuplicateCall)
    ));
    let mut items = d.semantic.items().to_vec();
    if let Item::ToolResult(r) = &mut items[4].1 {
        r.call_id = text("call_a");
    }
    assert!(matches!(
        d.semantic.with_items(items),
        Err(GenerationError::InvalidToolResult)
    ));
}
#[test]
fn strict_omission_is_source_default_not_false_and_cross_profile_fails() {
    for profile in [Profile::Chat, Profile::Responses] {
        let mut wire = if profile == Profile::Chat {
            chat_history()
        } else {
            expected_responses_history()
        };
        let def = if profile == Profile::Chat {
            &mut wire["tools"][0]["function"]
        } else {
            &mut wire["tools"][0]
        };
        def.as_object_mut().unwrap().remove("strict");
        let d = if profile == Profile::Chat {
            chat::decode_generation(&wire)
        } else {
            responses::decode_generation(&wire)
        }
        .unwrap();
        let ToolDefinition::Function(t) = &d.semantic.tools()[0] else {
            panic!("function");
        };
        assert_eq!(
            t.strict,
            FunctionStrictness::Omitted(if profile == Profile::Chat {
                StrictDefault::NonStrict
            } else {
                StrictDefault::NormalizeSchema
            })
        );
        assert_eq!(request_wire(&d, profile), wire);
        let other = if profile == Profile::Chat {
            Profile::Responses
        } else {
            Profile::Chat
        };
        assert!(matches!(
            lower_request(&d.semantic, &d.fidelity, other, Contract::full()),
            Err(RepresentationError::StrictDefault)
        ));
    }
}
#[test]
fn explicit_false_true_and_tool_controls_keep_presence() {
    for strict in [false, true] {
        for choice in [
            json!("none"),
            json!("auto"),
            json!("required"),
            json!({"type":"function","function":{"name":"weather"}}),
        ] {
            let mut wire = chat_history();
            wire["tools"][0]["function"]["strict"] = json!(strict);
            if strict {
                wire["tools"][0]["function"]["parameters"]["required"] = json!(["city"]);
                wire["tools"][0]["function"]["parameters"]["additionalProperties"] = json!(false);
            }
            wire["tool_choice"] = choice;
            let d = chat::decode_generation(&wire).unwrap();
            assert_eq!(request_wire(&d, Profile::Chat), wire);
            assert_eq!(
                request_wire(&d, Profile::Responses)["tools"][0]["strict"],
                strict
            );
        }
    }
    let wire = json!({"messages":[{"role":"user","content":"x"}],"tools":[],"tool_choice":"none","parallel_tool_calls":false});
    assert_eq!(
        request_wire(&chat::decode_generation(&wire).unwrap(), Profile::Chat),
        wire
    );
}
#[test]
fn target_failure_is_local_and_unsupported_domains_cannot_reach_encoding() {
    let mut wire = chat_history();
    wire["parallel_tool_calls"] = json!(true);
    wire["tools"][0]["function"]["strict"] = json!(true);
    wire["tools"][0]["function"]["parameters"]["required"] = json!(["city"]);
    wire["tools"][0]["function"]["parameters"]["additionalProperties"] = json!(false);
    let d = chat::decode_generation(&wire).unwrap();
    for (contract, expected) in [
        (
            Contract {
                tools: false,
                ..Contract::full()
            },
            RepresentationError::Tools,
        ),
        (
            Contract {
                parallel_tool_calls: false,
                ..Contract::full()
            },
            RepresentationError::ParallelTools,
        ),
        (
            Contract {
                strict_tools: false,
                ..Contract::full()
            },
            RepresentationError::StrictTools,
        ),
    ] {
        assert!(
            matches!(lower_request(&d.semantic, &d.fidelity, Profile::Chat, contract), Err(e) if e == expected)
        );
    }
    assert_eq!(request_wire(&d, Profile::Chat), wire);
    let ir = d.semantic.with_output(OutputConstraint::JsonObject);
    // Structured output now maps to Chat; the target capability gate still refuses it.
    assert!(lower_request(&ir, &d.fidelity, Profile::Chat, Contract::full()).is_ok());
    assert!(matches!(
        lower_request(
            &ir,
            &d.fidelity,
            Profile::Chat,
            Contract {
                structured_output: false,
                ..Contract::full()
            }
        ),
        Err(RepresentationError::StructuredOutput)
    ));
}
#[test]
fn unmodeled_tool_semantics_invalid_presence_and_missing_associations_are_rejected() {
    let base = chat_history();
    let mut cases = Vec::new();
    let mut w = base.clone();
    w["tools"][0]["type"] = json!("custom");
    cases.push(w);
    let mut w = base.clone();
    w["tools"][0]["function"]["strict"] = json!("false");
    cases.push(w);
    let mut w = base.clone();
    w["tools"][0]["function"]["parameters"] = json!([]);
    cases.push(w);
    let mut w = base.clone();
    w["tools"][0]["function"]["unknown_policy"] = json!(true);
    cases.push(w);
    let mut w = base.clone();
    w["tool_choice"] = json!({"type":"function","function":{"name":"missing"}});
    cases.push(w);
    let mut w = base.clone();
    w["messages"][2]["tool_call_id"] = json!("missing");
    cases.push(w);
    let mut w = base;
    w["stream"] = json!(true);
    cases.push(w);
    for w in cases {
        assert!(chat::decode_generation(&w).is_err());
    }
    let mut w = expected_responses_history();
    w["input"][0]["content"][0]["type"] = json!("input_image");
    assert!(responses::decode_generation(&w).is_err());
}
fn chat_response() -> Value {
    json!({"id":"response_1","object":"chat.completion","created":10,"model":"fixture-model",
        "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[
            {"id":"call_a","type":"function","function":{"name":"weather","arguments":"{\"city\":\"Paris\"}"}},
            {"id":"call_b","type":"function","function":{"name":"weather","arguments":"{}"}}
        ]},"finish_reason":"tool_calls"}],"usage":null})
}
fn responses_response() -> Value {
    json!({"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"completed","output":[
        {"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"{\"city\":\"Paris\"}","status":"completed"},
        {"id":"item_3","type":"function_call","call_id":"call_b","name":"weather","arguments":"{}","status":"completed"}
    ],"usage":null})
}
#[test]
fn static_response_encodes_independent_expectations_and_replays_into_history() {
    let a = chat::decode_response(&chat_response()).unwrap();
    assert_eq!(a.semantic.completion(), Some(Completion::ToolCalls));
    let t = lower_response(
        &a.semantic,
        &a.fidelity,
        &a.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        responses::encode_response(&t).unwrap(),
        responses_response()
    );
    let b = responses::decode_response(&responses_response()).unwrap();
    let t = lower_response(
        &b.semantic,
        &b.fidelity,
        &b.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(chat::encode_response(&t).unwrap(), chat_response());
    // The exact output semantic items become history; call_id is unchanged across both boundaries.
    let mut history = b.semantic.items().to_vec();
    history.push((
        ItemId::new(50),
        Item::ToolResult(ToolResult {
            call_id: text("call_b"),
            output: "rain".into(),
            status: None,
        }),
    ));
    history.push((
        ItemId::new(51),
        Item::ToolResult(ToolResult {
            call_id: text("call_a"),
            output: "sunny".into(),
            status: None,
        }),
    ));
    let d = DecodedRequest {
        semantic: GenerationRequest::new(history, GenerationControls::default()).unwrap(),
        fidelity: b.fidelity,
    };
    assert_eq!(
        request_wire(&d, Profile::Chat)["messages"][1],
        json!({"role":"tool","tool_call_id":"call_b","content":"rain"})
    );
}
#[test]
fn independently_constructed_static_ir_and_mutation_determine_all_response_wire() {
    let item = (
        ItemId::new(7),
        Item::ToolCall(ToolCall {
            call_id: text("c"),
            name: text("f"),
            arguments: "{broken".into(),
            message: None,
            status: ItemLifecycle::Completed,
        }),
    );
    let ir = GenerationResponse::new(vec![item], Completion::ToolCalls).unwrap();
    let metadata = ResponseMetadata {
        id: "r".into(),
        model: "fixture".into(),
        created: 0.into(),
        context: Default::default(),
    };
    let mut fidelity = FidelityRecords::default();
    fidelity
        .record_response_item_id(ItemId::new(7), "source_item")
        .unwrap();
    let t = lower_response(
        &ir,
        &fidelity,
        &metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        responses::encode_response(&t).unwrap()["output"],
        json!([{"id":"source_item","type":"function_call","call_id":"c","name":"f","arguments":"{broken","status":"completed"}])
    );
    let ir = ir
        .with_items(
            vec![(
                ItemId::new(8),
                Item::Message(Message {
                    phase: None,
                    status: ItemLifecycle::Completed,
                    role: MessageRole::Assistant,
                    parts: vec![Part {
                        id: PartId::new(1),
                        content: ContentPart::Text(text("replaced").into()),
                    }],
                }),
            )],
            Completion::Stop,
        )
        .unwrap();
    for profile in [Profile::Chat, Profile::Responses] {
        let t = lower_response(&ir, &fidelity, &metadata, profile, Contract::full()).unwrap();
        let wire = if profile == Profile::Chat {
            chat::encode_response(&t).unwrap()
        } else {
            responses::encode_response(&t).unwrap()
        };
        assert!(!wire.to_string().contains("source_item"));
        assert!(!wire.to_string().contains("function_call"));
        assert!(wire.to_string().contains("replaced"));
    }
}
#[test]
fn response_failures_and_independent_message_grouping_do_not_become_success() {
    let mut w = chat_response();
    w["choices"][0]["finish_reason"] = json!("length");
    assert_eq!(
        chat::decode_response(&w).unwrap().semantic.outcome(),
        Outcome::Incomplete
    );
    let mut w = chat_response();
    w["choices"][0]["finish_reason"] = json!("stop");
    assert!(chat::decode_response(&w).is_err());
    let mut w = responses_response();
    w["status"] = json!("incomplete");
    w["output"][0]["status"] = json!("incomplete");
    w["output"][1]["status"] = json!("incomplete");
    assert_eq!(
        responses::decode_response(&w).unwrap().semantic.outcome(),
        Outcome::Incomplete
    );
    let mut w = responses_response();
    w["output"][0]["status"] = json!("in_progress");
    assert!(responses::decode_response(&w).is_err());
    let mut w = responses_response();
    w["output"].as_array_mut().unwrap().insert(0, json!({"type":"message","id":"msg","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Checking.","annotations":[]}]}));
    let d = responses::decode_response(&w).unwrap();
    assert!(matches!(
        lower_response(
            &d.semantic,
            &d.fidelity,
            &d.metadata,
            Profile::Chat,
            Contract::full()
        ),
        Err(RepresentationError::MessageGrouping)
    ));
}
#[test]
fn usage_projects_known_totals_across_profiles_without_estimating() {
    let mut wire = chat_response();
    wire["usage"] = json!({"prompt_tokens":5,"completion_tokens":3,"total_tokens":8,"completion_tokens_details":{"reasoning_tokens":2}});
    let d = chat::decode_response(&wire).unwrap();
    assert_eq!(
        d.semantic.usage(),
        Some(Usage {
            input_tokens: 5,
            output_tokens: 3,
            total_tokens: 8,
            reasoning_tokens: Some(2),
            cached_input_tokens: None,
            input_cache_write_tokens: None,
        })
    );
    let chat = lower_response(
        &d.semantic,
        &d.fidelity,
        &d.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        chat::encode_response(&chat).unwrap()["usage"],
        wire["usage"]
    );
    let responses = lower_response(
        &d.semantic,
        &d.fidelity,
        &d.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        responses::encode_response(&responses).unwrap()["usage"],
        json!({"input_tokens":5,"output_tokens":3,"total_tokens":8,"output_tokens_details":{"reasoning_tokens":2}})
    );
    wire["usage"]["prompt_tokens"] = json!(0);
    wire["usage"]["completion_tokens"] = json!(0);
    wire["usage"]["total_tokens"] = json!(0);
    wire["usage"]
        .as_object_mut()
        .unwrap()
        .remove("completion_tokens_details");
    assert_eq!(
        chat::decode_response(&wire)
            .unwrap()
            .semantic
            .usage()
            .unwrap()
            .total_tokens,
        0
    );
    wire["usage"] = json!({"prompt_tokens":1,"completion_tokens":1,"total_tokens":3});
    assert!(chat::decode_response(&wire).is_err());
    wire["usage"] =
        json!({"prompt_tokens":1,"completion_tokens":1,"total_tokens":2,"audio_tokens":1});
    assert!(chat::decode_response(&wire).is_err());
}
#[test]
fn bounded_values_fidelity_collisions_and_codec_profile_mismatch_fail() {
    let mut wire = chat_history();
    wire["messages"][1]["tool_calls"][0]["function"]["arguments"] =
        json!("x".repeat(MAX_TEXT_BYTES + 1));
    assert!(chat::decode_generation(&wire).is_err());
    let mut f = FidelityRecords::default();
    f.record_response_item_id(ItemId::new(1), "same").unwrap();
    assert!(f.record_response_item_id(ItemId::new(2), "same").is_err());
    let d = chat::decode_generation(&chat_history()).unwrap();
    let t = lower_request(&d.semantic, &d.fidelity, Profile::Chat, Contract::full()).unwrap();
    assert!(responses::encode_generation(&t).is_err());
}

#[test]
fn transformed_total_request_budget_includes_tools_and_history() {
    let payload = "x".repeat(MAX_TEXT_BYTES - 100);
    let items = (0..4)
        .map(|i| {
            (
                ItemId::new(i),
                Item::Message(Message {
                    phase: None,
                    role: MessageRole::User,
                    status: ItemLifecycle::Completed,
                    parts: vec![Part {
                        id: PartId::new(i),
                        content: ContentPart::Text(crate::events_support::text(&payload).into()),
                    }],
                }),
            )
        })
        .collect();
    let request = GenerationRequest::new(items, GenerationControls::default()).unwrap();
    assert!(
        request
            .with_tool_settings(
                Some(vec![ToolDefinition::Function(FunctionTool {
                    name: text("tool"),
                    output_schema: None,
                    description: Some(payload),
                    parameters: None,
                    strict: FunctionStrictness::Explicit(false)
                })]),
                None,
                None
            )
            .is_err()
    );
}

#[test]
fn semantic_construction_cannot_bypass_identifier_or_control_validation() {
    let empty = Text::allowing_empty("", "fixture", 1).unwrap();
    let call = Item::ToolCall(ToolCall {
        call_id: empty.clone(),
        name: text("f"),
        arguments: "{}".into(),
        message: None,
        status: ItemLifecycle::Completed,
    });
    assert!(
        GenerationRequest::new(vec![(ItemId::new(1), call)], GenerationControls::default())
            .is_err()
    );
    let d = chat::decode_generation(&chat_history()).unwrap();
    let tool = ToolDefinition::Function(FunctionTool {
        name: empty,
        output_schema: None,
        description: None,
        parameters: None,
        strict: FunctionStrictness::Explicit(false),
    });
    assert!(
        d.semantic
            .clone()
            .with_tools(vec![tool], ToolChoice::Auto)
            .is_err()
    );
    let mut controls = GenerationControls::default();
    controls.max_output_tokens = Some(0);
    assert!(GenerationRequest::new(d.semantic.items().to_vec(), controls).is_err());
}
