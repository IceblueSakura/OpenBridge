use super::*;
use crate::ir::generation::{CallId, ItemId, ToolJsonValue};

fn request(protocol: ApiProtocol, value: Value) -> WireRequest {
    decode_request(protocol, value.as_object().unwrap(), 8192).unwrap()
}
fn render(value: &WireRequest) -> Result<Value, StaticCodecError> {
    Ok(serde_json::from_slice(&encode_native_request(value, "target")?).unwrap())
}
fn entries(value: &WireRequest) -> Vec<(InputIdentity, InputItem)> {
    value
        .semantic
        .input_identities()
        .iter()
        .copied()
        .zip(value.semantic.input().iter().cloned())
        .collect()
}
fn set(value: &mut WireRequest, entries: Vec<(InputIdentity, InputItem)>) {
    value.semantic = value
        .semantic
        .clone()
        .with_identified_input(entries)
        .unwrap();
}
fn text(value: &str) -> ContentPart {
    ContentPart::text(TextValue::new(value, 8192).unwrap())
}

#[test]
fn reordered_messages_keep_identity_hints_and_new_messages_do_not_borrow_them() {
    let mut value = request(
        ApiProtocol::ChatCompletions,
        json!({"model":"public","messages":[
        {"role":"user","content":"first","name":"source_a"},{"role":"user","content":"second","name":"source_b"}]}),
    );
    let original = entries(&value);
    set(
        &mut value,
        vec![
            original[1].clone(),
            (
                InputIdentity::new(9, 9),
                InputItem::Message(Message::new(MessageRole::User, vec![text("new")]).unwrap()),
            ),
            original[0].clone(),
        ],
    );
    let output = render(&value).unwrap();
    assert_eq!(
        output["messages"],
        json!([{"role":"user","content":"second","name":"source_b"},{"role":"user","content":"new"},{"role":"user","content":"first","name":"source_a"}])
    );
    set(&mut value, vec![original[1].clone()]);
    assert_eq!(
        render(&value).unwrap()["messages"],
        json!([{"role":"user","content":"second","name":"source_b"}])
    );
}

#[test]
fn text_edits_and_part_reordering_preserve_unmodified_audio_format() {
    let audio = json!({"type":"input_audio","input_audio":{"data":"AQ==","format":"wav"}});
    let mut value = request(
        ApiProtocol::ChatCompletions,
        json!({"model":"public","messages":[{"role":"user","content":[{"type":"text","text":"old"},audio]}]}),
    );
    let mut input = entries(&value);
    let InputItem::Message(message) = &input[0].1 else {
        panic!()
    };
    let changed = message
        .clone()
        .with_identified_content(vec![
            (1, message.content()[1].clone()),
            (0, text("new")),
            (2, text("added")),
        ])
        .unwrap();
    input[0].1 = InputItem::Message(changed);
    set(&mut value, input);
    assert_eq!(
        render(&value).unwrap()["messages"][0]["content"],
        json!([audio,{"type":"text","text":"new"},{"type":"text","text":"added"}])
    );
    let mut input = entries(&value);
    let InputItem::Message(message) = &input[0].1 else {
        panic!()
    };
    input[0].1 = InputItem::Message(
        message
            .clone()
            .with_identified_content(vec![(0, text("remaining"))])
            .unwrap(),
    );
    set(&mut value, input);
    assert_eq!(
        render(&value).unwrap()["messages"][0]["content"],
        json!([{"type":"text","text":"remaining"}])
    );
}

#[test]
fn calls_preserve_explicit_message_group_and_results_must_remain_correlated() {
    let mut value = request(
        ApiProtocol::ChatCompletions,
        json!({"model":"public","messages":[
        {"role":"assistant","content":null,"reasoning_content":"reason","tool_calls":[
            {"id":"a","type":"function","function":{"name":"first","arguments":"{ }"}},
            {"id":"b","type":"function","function":{"name":"second","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"a","content":"result a"},{"role":"tool","tool_call_id":"b","content":"result b"}]}),
    );
    let original = entries(&value);
    assert_eq!(
        original
            .iter()
            .map(|(identity, _)| identity.group())
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 1, 2]
    );
    set(
        &mut value,
        vec![
            original[0].clone(),
            original[2].clone(),
            original[1].clone(),
            original[4].clone(),
            original[3].clone(),
        ],
    );
    let output = render(&value).unwrap();
    assert_eq!(output["messages"].as_array().unwrap().len(), 3);
    assert_eq!(output["messages"][0]["reasoning_content"], "reason");
    assert_eq!(output["messages"][0]["tool_calls"][0]["id"], "b");
    assert_eq!(
        output["messages"][0]["tool_calls"][1]["function"]["arguments"],
        "{ }"
    );
    set(
        &mut value,
        vec![
            original[0].clone(),
            original[2].clone(),
            original[3].clone(),
        ],
    );
    assert!(matches!(
        render(&value),
        Err(StaticCodecError::InvalidToolIdentity)
    ));
    let InputItem::PriorToolCall(call) = &original[2].1 else {
        panic!()
    };
    let changed = ToolCall::new(
        call.id().clone(),
        call.call_id().clone(),
        ToolName::new("renamed", 8192).unwrap(),
        ToolInput::Function(JsonObject::new(json!({"x":1}), 8192).unwrap()),
        None,
    );
    set(
        &mut value,
        vec![
            original[0].clone(),
            (original[2].0, InputItem::PriorToolCall(changed)),
            original[4].clone(),
        ],
    );
    assert_eq!(
        render(&value).unwrap()["messages"][0]["tool_calls"],
        json!([{"id":"b","type":"function","function":{"name":"renamed","arguments":"{\"x\":1}"}}])
    );
}

#[test]
fn tool_result_json_and_text_remain_distinct_through_native_encode() {
    for payload in [
        json!({"answer":1}),
        json!([1, 2]),
        json!(false),
        json!(null),
        json!(1.25),
    ] {
        let mut value = request(
            ApiProtocol::Responses,
            json!({"model":"public","input":[
            {"type":"function_call","id":"fc_a","call_id":"a","name":"f","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":payload}]}),
        );
        let mut input = entries(&value);
        let InputItem::ToolResult(result) = &input[1].1 else {
            panic!()
        };
        let [ToolOutput::Json(json_value)] = result.output() else {
            panic!("JSON was collapsed into text")
        };
        assert_eq!(json_value.as_value(), &payload);
        input[1].1 = InputItem::ToolResult(ToolResult::new(
            result.id().clone(),
            result.call_id().clone(),
            ToolResultStatus::Success,
            vec![ToolOutput::Text(
                TextValue::new(payload.to_string(), 8192).unwrap(),
            )],
            None,
        ));
        set(&mut value, input);
        assert_eq!(
            render(&value).unwrap()["input"][1]["output"],
            json!(payload.to_string())
        );
    }
    assert!(ToolJsonValue::new(json!("text"), 8192).is_err());
    assert!(ToolJsonValue::new(json!([1, 2]), 1).is_err());
}

#[test]
fn instructions_and_new_calls_are_encoded_without_restoring_deleted_source_items() {
    let mut value = request(
        ApiProtocol::Responses,
        json!({"model":"public","instructions":"old policy","input":"old prompt"}),
    );
    let original = entries(&value);
    let policy = Instruction::new(
        InstructionAuthority::System,
        InstructionOrigin::Downstream,
        TextValue::new("new policy", 8192).unwrap(),
    );
    let call = ToolCall::new(
        ItemId::new("internal-only", 8192).unwrap(),
        CallId::new("new-call", 8192).unwrap(),
        ToolName::new("lookup", 8192).unwrap(),
        ToolInput::Function(JsonObject::new(json!({}), 8192).unwrap()),
        None,
    );
    let result = ToolResult::new(
        ItemId::new("result-internal", 8192).unwrap(),
        call.call_id().clone(),
        ToolResultStatus::Success,
        vec![ToolOutput::Text(TextValue::new("result", 8192).unwrap())],
        None,
    );
    set(
        &mut value,
        vec![
            (original[0].0, InputItem::Instruction(policy)),
            (InputIdentity::new(9, 9), InputItem::PriorToolCall(call)),
            (InputIdentity::new(10, 10), InputItem::ToolResult(result)),
        ],
    );
    let encoded = render(&value).unwrap();
    assert_eq!(encoded["instructions"], "new policy");
    assert_eq!(
        encoded["input"],
        json!([
        {"type":"function_call","id":"ob_input_call_1","call_id":"new-call","name":"lookup","arguments":"{}"},
        {"type":"function_call_output","call_id":"new-call","output":"result"}])
    );
    let mut input = entries(&value);
    input.remove(0);
    set(&mut value, input);
    assert!(render(&value).unwrap().get("instructions").is_none());
}

#[test]
fn unmapped_history_and_metadata_fail_closed_instead_of_disappearing() {
    for source in [
        json!({"model":"public","messages":[{"role":"user","content":"old","name":"source"}]}),
        json!({"model":"public","messages":[{"role":"assistant","content":null,"audio":{"id":"audio_previous"}},{"role":"user","content":"old"}]}),
    ] {
        let mut value = request(ApiProtocol::ChatCompletions, source);
        assert!(render(&value).is_ok());
        value.semantic = GenerationRequest::new(vec![InputItem::Message(
            Message::new(MessageRole::User, vec![text("new")]).unwrap(),
        )])
        .unwrap();
        assert!(matches!(
            render(&value),
            Err(StaticCodecError::UnsupportedSemantics)
        ));
    }
}

#[test]
fn appending_after_deletion_never_reuses_the_deleted_source_identity() {
    let mut value = request(
        ApiProtocol::ChatCompletions,
        json!({"model":"public","messages":[
            {"role":"user","content":"first"},
            {"role":"user","content":"same text","name":"deleted-source"}
        ]}),
    );
    let original = entries(&value);
    set(&mut value, vec![original[0].clone()]);
    value.semantic = value
        .semantic
        .clone()
        .with_appended_input([InputItem::Message(
            Message::new(MessageRole::User, vec![text("same text")]).unwrap(),
        )])
        .unwrap();
    let output = render(&value).unwrap();
    assert_eq!(
        output["messages"][1],
        json!({"role":"user","content":"same text"})
    );
    assert_ne!(
        value.semantic.input_identities()[1].id(),
        original[1].0.id()
    );
}

#[test]
fn identity_and_group_validation_rejects_ambiguous_layouts() {
    let message = InputItem::Message(Message::new(MessageRole::User, vec![text("x")]).unwrap());
    let request = GenerationRequest::new(vec![message.clone()]).unwrap();
    assert!(
        request
            .clone()
            .with_identified_input(vec![
                (InputIdentity::new(0, 0), message.clone()),
                (InputIdentity::new(0, 1), message.clone())
            ])
            .is_err()
    );
    assert!(
        request
            .with_identified_input(vec![
                (InputIdentity::new(0, 0), message.clone()),
                (InputIdentity::new(1, 1), message.clone()),
                (InputIdentity::new(2, 0), message)
            ])
            .is_err()
    );
    assert!(
        Message::new(MessageRole::User, vec![text("x")])
            .unwrap()
            .with_identified_content(vec![(0, text("a")), (0, text("b"))])
            .is_err()
    );
}

#[test]
fn existing_input_identity_cannot_be_reparented_between_message_groups() {
    let mut value = request(
        ApiProtocol::ChatCompletions,
        json!({"model":"public","messages":[
            {"role":"user","content":"first"},
            {"role":"user","content":"second"}
        ]}),
    );
    let original = entries(&value);
    let moved_identity = InputIdentity::new(original[1].0.id(), original[0].0.group());

    set(
        &mut value,
        vec![
            original[0].clone(),
            (moved_identity, original[1].1.clone()),
        ],
    );

    assert!(matches!(
        render(&value),
        Err(StaticCodecError::UnsupportedSemantics)
    ));
}
