use super::*;

fn decoded(protocol: ApiProtocol, source: &Value) -> WireResponse {
    decode_response(
        protocol,
        source.as_object().unwrap(),
        ReasoningOutput::PlainText,
        8192,
    )
    .unwrap()
}

fn render(
    protocol: ApiProtocol,
    source: &Value,
    response: &WireResponse,
) -> Result<Value, StaticCodecError> {
    let bytes = encode_native_response(
        response,
        source.as_object().unwrap(),
        protocol,
        "public",
        ReasoningOutput::PlainText,
        8192,
    )?;
    Ok(serde_json::from_slice(&bytes).unwrap())
}

fn replace(response: &mut WireResponse, output: Vec<OutputItem>, finish: Option<FinishReason>) {
    let candidate = &response.semantic.candidates()[0];
    response.semantic = GenerationResponse::new(
        response.semantic.id().clone(),
        vec![Candidate::new(candidate.id().clone(), output, finish).unwrap()],
        response.semantic.status(),
        response.semantic.usage().copied(),
        response.semantic.extensions().to_vec(),
    )
    .unwrap();
}

fn text(value: &str) -> ContentPart {
    ContentPart::text(TextValue::new(value, 8192).unwrap())
}

fn responses(parts: Value) -> Value {
    json!({"id":"resp_test","object":"response","status":"completed","output":[
        {"id":"msg_test","type":"message","role":"assistant","status":"completed","content":parts}
    ]})
}

#[test]
fn part_identity_preserves_annotations_on_reorder_and_removes_them_on_delete() {
    let source = responses(json!([
        {"type":"output_text","text":"first","annotations":[{"type":"url_citation","url":"https://example.com/a","start_index":0,"end_index":5}]},
        {"type":"output_text","text":"second","annotations":[{"type":"url_citation","url":"https://example.com/b","start_index":0,"end_index":6}]}
    ]));
    let mut response = decoded(ApiProtocol::Responses, &source);
    let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
        panic!()
    };
    assert_eq!(message.content(), &[text("first"), text("second")]);
    let reordered = message
        .clone()
        .with_identified_content(vec![(1, text("second")), (0, text("first"))])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(reordered.clone())],
        Some(FinishReason::Stop),
    );
    let actual = render(ApiProtocol::Responses, &source, &response).unwrap();
    assert_eq!(
        actual["output"][0]["content"],
        json!([
            {"type":"output_text","text":"second","annotations":[{"type":"url_citation","url":"https://example.com/b","start_index":0,"end_index":6}]},
            {"type":"output_text","text":"first","annotations":[{"type":"url_citation","url":"https://example.com/a","start_index":0,"end_index":5}]}
        ])
    );
    let deleted = reordered
        .with_identified_content(vec![(1, text("second"))])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(deleted)],
        Some(FinishReason::Stop),
    );
    let actual = render(ApiProtocol::Responses, &source, &response).unwrap();
    assert_eq!(
        actual["output"][0]["content"],
        json!([
            {"type":"output_text","text":"second","annotations":[{"type":"url_citation","url":"https://example.com/b","start_index":0,"end_index":6}]}
        ])
    );
}

#[test]
fn content_edits_reject_stale_annotations_and_duplicate_part_identities() {
    let source = responses(
        json!([{"type":"output_text","text":"old","annotations":[{"type":"url_citation","url":"https://example.com","start_index":0,"end_index":3}]}]),
    );
    let mut response = decoded(ApiProtocol::Responses, &source);
    let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
        panic!()
    };
    assert!(
        message
            .clone()
            .with_identified_content(vec![(0, text("a")), (0, text("b"))])
            .is_err()
    );
    let replacement = message
        .clone()
        .with_identified_content(vec![(0, text("new"))])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(replacement)],
        Some(FinishReason::Stop),
    );
    assert!(matches!(
        render(ApiProtocol::Responses, &source, &response),
        Err(StaticCodecError::UnsupportedSemantics)
    ));
}

#[test]
fn new_response_items_do_not_inherit_old_metadata_or_expose_internal_ids() {
    let source = responses(
        json!([{"type":"output_text","text":"old","annotations":[{"type":"url_citation","url":"https://example.com"}]}]),
    );
    let mut response = decoded(ApiProtocol::Responses, &source);
    let message = ResponseMessage::new(
        ItemId::new("internal-only", 8192).unwrap(),
        vec![text("new")],
        None,
    )
    .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(message)],
        Some(FinishReason::Stop),
    );
    let actual = render(ApiProtocol::Responses, &source, &response).unwrap();
    assert_eq!(
        actual["output"],
        json!([{"id":"ob_output_0","type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":"new","annotations":[]}]}])
    );
}

#[test]
fn chat_text_edit_preserves_same_audio_resource_but_deletion_does_not_restore_it() {
    let source = json!({"id":"chatcmpl_test","object":"chat.completion","choices":[
        {"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"old",
            "audio":{"id":"audio_test","data":"AQI=","transcript":"sound","expires_at":99}}}
    ]});
    let mut response = decoded(ApiProtocol::ChatCompletions, &source);
    let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
        panic!()
    };
    let audio = message.content()[1].clone();
    let edited = message
        .clone()
        .with_identified_content(vec![(0, text("new")), (1, audio)])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(edited.clone())],
        Some(FinishReason::Stop),
    );
    let actual = render(ApiProtocol::ChatCompletions, &source, &response).unwrap();
    assert_eq!(
        actual["choices"][0]["message"],
        json!({"role":"assistant","content":"new",
        "audio":{"id":"audio_test","data":"AQI=","transcript":"sound","expires_at":99}})
    );
    let deleted = edited
        .with_identified_content(vec![(0, text("new"))])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(deleted)],
        Some(FinishReason::Stop),
    );
    assert_eq!(
        render(ApiProtocol::ChatCompletions, &source, &response).unwrap()["choices"][0]["message"],
        json!({"role":"assistant","content":"new"})
    );
}

#[test]
fn function_call_edits_encode_ir_arguments_and_deletions_remove_calls() {
    for (protocol, source) in [
        (
            ApiProtocol::Responses,
            json!({"id":"resp_test","object":"response","status":"completed","output":[
                {"id":"fc_test","type":"function_call","call_id":"call_test","name":"old","arguments":"{ \"x\": 1 }","status":"completed"}
            ]}),
        ),
        (
            ApiProtocol::ChatCompletions,
            json!({"id":"chatcmpl_test","object":"chat.completion","choices":[
                {"index":0,"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[
                    {"id":"call_test","type":"function","function":{"name":"old","arguments":"{ \"x\": 1 }"}}
                ]}}
            ]}),
        ),
    ] {
        let mut response = decoded(protocol, &source);
        let OutputItem::ToolCall(call) = &response.semantic.candidates()[0].output()[0] else {
            panic!()
        };
        let replacement = ToolCall::new(
            call.id().clone(),
            call.call_id().clone(),
            ToolName::new("new", 8192).unwrap(),
            ToolInput::Function(JsonObject::new(json!({"x":2}), 8192).unwrap()),
            None,
        );
        replace(
            &mut response,
            vec![OutputItem::ToolCall(replacement)],
            Some(FinishReason::ToolCalls),
        );
        let actual = render(protocol, &source, &response).unwrap();
        match protocol {
            ApiProtocol::Responses => assert_eq!(
                actual["output"],
                json!([
                    {"id":"fc_test","type":"function_call","call_id":"call_test","name":"new","arguments":"{\"x\":2}","status":"completed"}
                ])
            ),
            ApiProtocol::ChatCompletions => assert_eq!(
                actual["choices"][0]["message"]["tool_calls"],
                json!([
                    {"id":"call_test","type":"function","function":{"name":"new","arguments":"{\"x\":2}"}}
                ])
            ),
        }
        let message = ResponseMessage::new(
            ItemId::new("inserted", 8192).unwrap(),
            vec![text("answer")],
            None,
        )
        .unwrap();
        replace(
            &mut response,
            vec![OutputItem::Message(message)],
            Some(FinishReason::Stop),
        );
        let actual = render(protocol, &source, &response).unwrap();
        match protocol {
            ApiProtocol::Responses => assert_eq!(
                actual["output"],
                json!([{"id":"ob_output_0","type":"message","role":"assistant","status":"completed",
                "content":[{"type":"output_text","text":"answer","annotations":[]}]}])
            ),
            ApiProtocol::ChatCompletions => {
                assert_eq!(
                    actual["choices"][0]["message"],
                    json!({"role":"assistant","content":"answer"})
                );
                assert_eq!(actual["choices"][0]["finish_reason"], json!("stop"));
            }
        }
    }
}

#[test]
fn unmapped_empty_parts_and_content_dependent_fields_fail_closed_on_edits() {
    for parts in [
        json!([{"type":"output_text","text":""},{"type":"output_text","text":"old"}]),
        json!([{"type":"output_text","text":"old","provider_offset":2}]),
    ] {
        let source = responses(parts);
        let mut response = decoded(ApiProtocol::Responses, &source);
        assert_eq!(
            render(ApiProtocol::Responses, &source, &response).unwrap(),
            source
        );
        let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
            panic!()
        };
        let edited = message
            .clone()
            .with_identified_content(vec![(0, text("new"))])
            .unwrap();
        replace(
            &mut response,
            vec![OutputItem::Message(edited)],
            Some(FinishReason::Stop),
        );
        assert!(matches!(
            render(ApiProtocol::Responses, &source, &response),
            Err(StaticCodecError::UnsupportedSemantics)
        ));
    }
}

#[test]
fn reordered_items_keep_their_own_source_fields_and_fresh_items_do_not_borrow_them() {
    let source = json!({"id":"resp_test","object":"response","status":"completed","output":[
        {"id":"fc_a","type":"function_call","call_id":"call_a","name":"a","arguments":"{ }","provider_tag":"a"},
        {"id":"fc_b","type":"function_call","call_id":"call_b","name":"b","arguments":"{}","provider_tag":"b"}
    ]});
    let mut response = decoded(ApiProtocol::Responses, &source);
    let old = response.semantic.candidates()[0].output();
    let new_call = ToolCall::new(
        ItemId::new("internal-new", 8192).unwrap(),
        crate::ir::generation::CallId::new("call_new", 8192).unwrap(),
        ToolName::new("new", 8192).unwrap(),
        ToolInput::Function(JsonObject::new(json!({}), 8192).unwrap()),
        None,
    );
    let output = vec![
        old[1].clone(),
        OutputItem::ToolCall(new_call),
        old[0].clone(),
    ];
    replace(&mut response, output, Some(FinishReason::ToolCalls));
    assert_eq!(
        render(ApiProtocol::Responses, &source, &response).unwrap()["output"],
        json!([
            {"id":"fc_b","type":"function_call","call_id":"call_b","name":"b","arguments":"{}","provider_tag":"b"},
            {"id":"ob_output_0","type":"function_call","status":"completed","call_id":"call_new","name":"new","arguments":"{}"},
            {"id":"fc_a","type":"function_call","call_id":"call_a","name":"a","arguments":"{ }","provider_tag":"a"}
        ])
    );
}

#[test]
fn unknown_envelope_dependencies_cannot_survive_content_edits() {
    let mut source = responses(json!([{"type":"output_text","text":"old"}]));
    source["provider_summary"] = json!("old");
    let mut response = decoded(ApiProtocol::Responses, &source);
    assert_eq!(
        render(ApiProtocol::Responses, &source, &response).unwrap(),
        source
    );
    let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
        panic!()
    };
    let edited = message
        .clone()
        .with_identified_content(vec![(0, text("new"))])
        .unwrap();
    replace(
        &mut response,
        vec![OutputItem::Message(edited)],
        Some(FinishReason::Stop),
    );
    assert!(matches!(
        render(ApiProtocol::Responses, &source, &response),
        Err(StaticCodecError::UnsupportedSemantics)
    ));
}

#[test]
fn same_text_with_new_part_identity_does_not_inherit_old_annotations() {
    let source = responses(
        json!([{"type":"output_text","text":"same","annotations":[
            {"type":"url_citation","url":"https://example.com/source","start_index":0,"end_index":4}
        ]}]),
    );
    let mut response = decoded(ApiProtocol::Responses, &source);
    let OutputItem::Message(message) = &response.semantic.candidates()[0].output()[0] else {
        panic!()
    };
    let replacement = message
        .clone()
        .with_identified_content(vec![(7, text("same"))])
        .unwrap();

    replace(
        &mut response,
        vec![OutputItem::Message(replacement)],
        Some(FinishReason::Stop),
    );

    let actual = render(ApiProtocol::Responses, &source, &response).unwrap();
    assert_eq!(
        actual["output"][0]["content"],
        json!([{"type":"output_text","text":"same","annotations":[]}])
    );
}
