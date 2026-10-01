//! Empty message owners are values, not disposable Chat packaging.
use crate::events_support::metadata;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, RepresentationError, lower_request,
        lower_response,
    },
    protocol::{
        fidelity::FidelityRecords,
        openai::{
            Profile, chat,
            events::{EventDecoder, EventEncoder},
            responses,
        },
    },
    semantic::task::generation::*,
};
use serde_json::{Value, json};

fn source() -> Value {
    json!({"id":"r","object":"chat.completion","created":0,"model":"synthetic","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"c","type":"function","function":{"name":"lookup","arguments":"{}"}}]},"finish_reason":"tool_calls"}]})
}
fn expected() -> Value {
    json!([
        {"id":"item_1","type":"message","role":"assistant","content":[],"status":"completed"},
        {"id":"item_2","type":"function_call","call_id":"c","name":"lookup","arguments":"{}","status":"completed"}
    ])
}

#[test]
fn empty_owner_survives_static_output_and_request_history() {
    let d = chat::decode_response(&source()).unwrap();
    let target = lower_response(
        &d.semantic,
        &d.fidelity,
        &d.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let wire = responses::encode_response(&target).unwrap();
    assert_eq!(wire["output"], expected());
    let request =
        GenerationRequest::new(d.semantic.items().to_vec(), GenerationControls::default()).unwrap();
    let target =
        lower_request(&request, &d.fidelity, Profile::Responses, Contract::full()).unwrap();
    assert_eq!(
        responses::encode_generation(&target).unwrap()["input"],
        json!([
            {"type":"message","role":"assistant","content":[]},
            {"type":"function_call","call_id":"c","name":"lookup","arguments":"{}"}
        ])
    );
    // Responses has no carrier association field; adjacency cannot re-invent it.
    let projected = responses::decode_response(&wire).unwrap();
    assert_eq!(
        projected
            .semantic
            .message_groups()
            .next()
            .unwrap()
            .calls()
            .len(),
        0
    );
    assert!(matches!(
        lower_response(
            &projected.semantic,
            &projected.fidelity,
            &projected.metadata,
            Profile::Chat,
            Contract::full()
        ),
        Err(RepresentationError::MessageGrouping)
    ));
}

#[test]
fn empty_owner_keeps_phase_status_and_participates_in_wire_identity_checks() {
    let d = chat::decode_response(&source()).unwrap();
    let mut items = d.semantic.items().to_vec();
    let Item::Message(owner) = &mut items[0].1 else {
        panic!("owner")
    };
    owner.phase = Some(Phase::Commentary);
    owner.status = ItemLifecycle::Incomplete;
    let response = GenerationResponse::new(items, Outcome::Incomplete).unwrap();
    let m = metadata();
    let target = lower_response(
        &response,
        &d.fidelity,
        &m,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let wire = responses::encode_response(&target).unwrap();
    assert_eq!(wire["output"][0]["type"], "message");
    assert_eq!(wire["output"][0]["phase"], "commentary");
    assert_eq!(wire["output"][0]["status"], "incomplete");
    assert_eq!(wire["output"][1]["status"], "completed");
    let mut collision = FidelityRecords::default();
    collision
        .record_response_item_id(ItemId::new(2), "item_1")
        .unwrap();
    assert!(matches!(
        lower_response(
            &response,
            &collision,
            &m,
            Profile::Responses,
            Contract::full()
        ),
        Err(RepresentationError::Metadata)
    ));
}

#[test]
fn calls_only_chat_stream_projects_a_self_consistent_responses_terminal() {
    let chunk = |delta: Value, finish: Value| json!({"id":"r","object":"chat.completion.chunk","created":0,"model":"synthetic","choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
    let mut source = EventDecoder::new(Profile::Chat);
    let mut events = vec![];
    for wire in [
        chunk(
            json!({"role":"assistant","tool_calls":[{"index":0,"id":"c","type":"function","function":{"name":"lookup","arguments":"{"}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls":[{"index":0,"function":{"arguments":"}"}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
    ] {
        events.extend(source.push(&wire).unwrap());
    }
    events.extend(source.done().unwrap());
    let decoded = source.materialize().unwrap();
    let group = decoded.semantic.message_groups().next().unwrap();
    assert!(group.message().parts.is_empty());
    assert_eq!(group.calls().map(|c| c.call_id).collect::<Vec<_>>(), ["c"]);
    let mut encoder = EventEncoder::new(Profile::Responses, decoded.metadata.clone()).unwrap();
    let mut consumer = EventDecoder::new(Profile::Responses);
    let mut wire = vec![];
    for event in events {
        for frame in encoder.encode(&event, &decoded.fidelity).unwrap() {
            consumer.push(&frame).unwrap();
            wire.push(frame);
        }
    }
    encoder.finish().unwrap();
    consumer.finish().unwrap();
    assert_eq!(wire.last().unwrap()["response"]["output"], expected());
    let d = consumer.materialize().unwrap();
    assert_eq!(d.semantic.items().len(), 2);
    assert_eq!(d.semantic.outcome(), Outcome::Completed);
}
