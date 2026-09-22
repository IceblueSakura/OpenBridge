//! Function-call Event IR: independent deltas, terminals, and static materialization.
use openbridge::{
    lowering::generation::{GenerationRepresentationContract as Contract, lower_response},
    protocol::openai::{
        Profile, ResponseMetadata,
        function_events::{FunctionEventDecoder, FunctionEventEncoder},
    },
    semantic::task::generation::{
        Completion, EventError, Item, ItemId, MessageRole, Outcome, StreamEvent, StreamState,
        StreamTerminal, end_of_stream, materialize, reduce,
    },
};
use serde_json::{Value, json};

fn text(s: &str) -> openbridge::semantic::value::Text {
    openbridge::semantic::value::Text::new(s, "fixture", 256).unwrap()
}
fn metadata() -> ResponseMetadata {
    ResponseMetadata {
        id: "response_1".into(),
        model: "fixture-model".into(),
        created: 10,
    }
}
fn started(item: u64, call: &str, name: &str, message: Option<u64>) -> StreamEvent {
    StreamEvent::CallStarted {
        item: ItemId::new(item),
        call_id: text(call),
        name: text(name),
        message: message.map(ItemId::new),
    }
}
fn delta(item: u64, fragment: &str) -> StreamEvent {
    StreamEvent::ArgumentsDelta {
        item: ItemId::new(item),
        fragment: fragment.into(),
    }
}
fn finished(item: u64) -> StreamEvent {
    StreamEvent::CallFinished {
        item: ItemId::new(item),
    }
}
fn apply(events: &[StreamEvent]) -> Result<StreamState, EventError> {
    events.iter().cloned().try_fold(StreamState::new(), reduce)
}
fn decode(
    profile: Profile,
    payloads: &[Value],
) -> (
    Vec<StreamEvent>,
    openbridge::protocol::fidelity::FidelityRecords,
    ResponseMetadata,
) {
    let mut decoder = FunctionEventDecoder::new(profile);
    let mut events = Vec::new();
    for payload in payloads {
        events.extend(decoder.push(payload).unwrap());
    }
    decoder.finish().unwrap();
    let (fidelity, metadata) = decoder.into_parts().unwrap();
    (events, fidelity, metadata)
}

#[test]
fn argument_deltas_keep_exact_incomplete_json_and_call_identity() {
    let events = [
        started(2, "call_a", "weather", Some(1)),
        delta(2, "{\"city\":"),
        delta(2, "\"Paris\"}"),
        finished(2),
        StreamEvent::Terminal(StreamTerminal::Completed),
    ];
    let state = apply(&events).unwrap();
    let response = materialize(&state).unwrap();
    assert_eq!(response.completion(), Some(Completion::ToolCalls));
    let Item::ToolCall(call) = &response.items()[1].1 else {
        panic!("call");
    };
    assert_eq!(call.arguments, "{\"city\":\"Paris\"}");
    assert_eq!(call.call_id.as_str(), "call_a");
    assert_eq!(call.message, Some(ItemId::new(1)));
    let mut dropped = events.to_vec();
    dropped.remove(2);
    let shortened = materialize(&apply(&dropped).unwrap()).unwrap();
    let Item::ToolCall(call) = &shortened.items()[1].1 else {
        panic!("call");
    };
    assert_eq!(call.arguments, "{\"city\":");
    assert!(!call.arguments.contains("Paris"));
}

#[test]
fn identity_conflicts_open_calls_and_duplicate_finish_fail_before_success() {
    assert!(matches!(
        apply(&[
            started(1, "call_a", "weather", None),
            started(2, "call_a", "other", None),
        ]),
        Err(EventError::Identity)
    ));
    assert!(matches!(
        apply(&[started(1, "call_a", "weather", None), delta(9, "{")]),
        Err(EventError::Identity)
    ));
    assert!(matches!(
        apply(&[
            started(1, "call_a", "weather", None),
            finished(1),
            delta(1, "{"),
        ]),
        Err(EventError::Lifecycle)
    ));
    assert!(matches!(
        apply(&[
            started(1, "call_a", "weather", None),
            StreamEvent::Terminal(StreamTerminal::Completed),
        ]),
        Err(EventError::Lifecycle)
    ));
    assert!(matches!(
        apply(&[
            started(1, "call_a", "weather", Some(7)),
            started(2, "call_b", "weather", None),
        ]),
        Err(EventError::Lifecycle)
    ));
    let open = apply(&[started(1, "call_a", "weather", None)]).unwrap();
    assert!(matches!(
        end_of_stream(&open),
        Err(EventError::EofBeforeTerminal)
    ));
}

#[test]
fn failed_incomplete_and_error_terminals_do_not_materialize_as_success() {
    for terminal in [
        StreamTerminal::Failed,
        StreamTerminal::Incomplete,
        StreamTerminal::Error,
    ] {
        let state = apply(&[
            started(1, "call_a", "weather", None),
            delta(1, "{"),
            finished(1),
            StreamEvent::Terminal(terminal),
        ])
        .unwrap();
        assert_eq!(state.terminal(), Some(terminal));
        let materialized = materialize(&state);
        match terminal {
            StreamTerminal::Error => assert!(matches!(
                materialized,
                Err(EventError::TerminalFailure(StreamTerminal::Error))
            )),
            StreamTerminal::Failed => {
                assert_eq!(materialized.unwrap().outcome(), Outcome::Failed)
            }
            StreamTerminal::Incomplete => {
                assert_eq!(materialized.unwrap().outcome(), Outcome::Incomplete)
            }
            StreamTerminal::Completed => unreachable!("completed is not in this set"),
        }
        assert!(end_of_stream(&state).is_ok());
    }
}

#[test]
fn chat_and_responses_fragments_materialize_to_independent_static_calls() {
    let chat = [
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"weather","arguments":""}}]},"finish_reason":null}]}),
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":"}}]},"finish_reason":null}]}),
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Paris\"}"}}]},"finish_reason":"tool_calls"}]}),
    ];
    let (events, fidelity, metadata) = decode(Profile::Chat, &chat);
    assert!(fidelity.response_item_id(ItemId::new(2)).is_none());
    let response = materialize(&apply(&events).unwrap()).unwrap();
    assert_eq!(
        response.items()[0],
        (
            ItemId::new(1),
            Item::Message(openbridge::semantic::task::generation::Message {
                role: MessageRole::Assistant,
                parts: vec![],
            })
        )
    );
    let Item::ToolCall(call) = &response.items()[1].1 else {
        panic!("call");
    };
    assert_eq!(call.call_id.as_str(), "call_a");
    assert_eq!(call.name.as_str(), "weather");
    assert_eq!(call.arguments, "{\"city\":\"Paris\"}");
    assert_eq!(call.message, Some(ItemId::new(1)));
    let responses = [
        json!({"type":"response.created","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"in_progress","output":[]}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"","status":"in_progress"}}),
        json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"item_2","delta":"{\"ci"}),
        json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"item_2","delta":"ty\":\"Paris\"}"}),
        json!({"type":"response.function_call_arguments.done","output_index":0,"item_id":"item_2","arguments":"{\"city\":\"Paris\"}"}),
        json!({"type":"response.output_item.done","output_index":0,"item":{"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"{\"city\":\"Paris\"}","status":"completed"}}),
        json!({"type":"response.completed","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"completed","output":[{"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"{\"city\":\"Paris\"}","status":"completed"}]}}),
    ];
    let (events, fidelity, responses_metadata) = decode(Profile::Responses, &responses);
    assert_eq!(fidelity.response_item_id(ItemId::new(1)), Some("item_2"));
    assert_eq!(responses_metadata, metadata);
    let other = materialize(&apply(&events).unwrap()).unwrap();
    let Item::ToolCall(other_call) = &other.items()[0].1 else {
        panic!("call");
    };
    assert_eq!(other_call.call_id, call.call_id);
    assert_eq!(other_call.name, call.name);
    assert_eq!(other_call.arguments, call.arguments);
    assert_eq!(other_call.message, None);
    let fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    let static_chat = lower_response(
        &response,
        &fidelity,
        &metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        openbridge::protocol::openai::chat::encode_response(&static_chat).unwrap()["choices"][0]["message"]
            ["tool_calls"],
        json!([{"id":"call_a","type":"function","function":{"name":"weather","arguments":"{\"city\":\"Paris\"}"}}])
    );
}

#[test]
fn encoded_events_follow_semantic_deltas_and_drop_deleted_fidelity() {
    let events = [
        started(1, "call_a", "weather", None),
        delta(1, "{\"city\":\"Paris\"}"),
        finished(1),
        started(2, "call_b", "forecast", None),
        delta(2, "{}"),
        finished(2),
        StreamEvent::Terminal(StreamTerminal::Completed),
    ];
    let mut fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    fidelity
        .record_response_item_id(ItemId::new(1), "item_a")
        .unwrap();
    fidelity
        .record_response_item_id(ItemId::new(2), "item_b")
        .unwrap();
    let mut encoder = FunctionEventEncoder::new(Profile::Responses, metadata()).unwrap();
    let wire: Vec<_> = events
        .iter()
        .flat_map(|event| encoder.encode(event, &fidelity).unwrap())
        .collect();
    assert!(
        wire.iter()
            .any(|v| v["type"] == "response.function_call_arguments.delta" && v["delta"] == "{}")
    );
    assert!(
        wire.last().unwrap()["response"]["output"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "item_b")
    );
    let kept: Vec<_> = events.into_iter().filter(|event| !matches!(event, StreamEvent::CallStarted { item, .. } | StreamEvent::ArgumentsDelta { item, .. } | StreamEvent::CallFinished { item } if item.get() == 2)).collect();
    let mut encoder = FunctionEventEncoder::new(Profile::Responses, metadata()).unwrap();
    let wire: Vec<_> = kept
        .iter()
        .flat_map(|event| encoder.encode(event, &fidelity).unwrap())
        .collect();
    let rendered = serde_json::to_string(&wire).unwrap();
    assert!(!rendered.contains("item_b"));
    assert!(!rendered.contains("call_b"));
    assert!(rendered.contains("item_a"));
}

#[test]
fn snapshot_mismatch_and_noncompleted_chat_finish_do_not_repair_arguments() {
    let mut decoder = FunctionEventDecoder::new(Profile::Responses);
    decoder
        .push(&json!({"type":"response.created","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"in_progress","output":[]}}))
        .unwrap();
    decoder
        .push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"{\"a\":","status":"in_progress"}}))
        .unwrap();
    assert!(decoder
        .push(&json!({"type":"response.function_call_arguments.done","output_index":0,"item_id":"item_2","arguments":"{\"a\":1}"}))
        .is_err());
    let mut decoder = FunctionEventDecoder::new(Profile::Chat);
    let events = decoder
        .push(&json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"weather","arguments":"{"}}]},"finish_reason":"length"}]}))
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::Terminal(StreamTerminal::Incomplete)))
    );
    assert_eq!(
        materialize(&apply(&events).unwrap()).unwrap().outcome(),
        Outcome::Incomplete
    );
}

#[test]
fn error_terminal_is_distinct_and_unknown_event_fields_fail() {
    let mut decoder = FunctionEventDecoder::new(Profile::Responses);
    decoder
        .push(&json!({"type":"response.created","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"in_progress","output":[]}}))
        .unwrap();
    let events = decoder
        .push(&json!({"type":"error","code":"server_error","message":"boom","param":null}))
        .unwrap();
    assert_eq!(events, vec![StreamEvent::Terminal(StreamTerminal::Error)]);
    assert!(decoder
        .push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"item_2","type":"function_call","call_id":"call_a","name":"weather","arguments":"","status":"in_progress"},"sequence_number":1}))
        .is_err());
}
