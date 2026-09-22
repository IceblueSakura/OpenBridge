//! Assistant text Event IR closed against the completed static response contract.
use openbridge::{
    lowering::generation::{GenerationRepresentationContract as Contract, lower_response},
    protocol::openai::{
        Profile, ResponseMetadata,
        function_events::{FunctionEventDecoder, FunctionEventEncoder},
    },
    semantic::task::generation::{
        Completion, ContentPart, EventError, Item, ItemId, MessageRole, PartId, StreamEvent,
        StreamState, StreamTerminal, materialize, reduce,
    },
};
use serde_json::{Value, json};

fn metadata() -> ResponseMetadata {
    ResponseMetadata {
        id: "response_1".into(),
        model: "fixture-model".into(),
        created: 10,
    }
}
fn text_delta(item: u64, part: u64, fragment: &str) -> StreamEvent {
    StreamEvent::TextDelta {
        item: ItemId::new(item),
        part: PartId::new(part),
        fragment: fragment.into(),
    }
}
fn apply(events: &[StreamEvent]) -> Result<StreamState, EventError> {
    events.iter().cloned().try_fold(StreamState::new(), reduce)
}
fn decode(profile: Profile, payloads: &[Value]) -> Vec<StreamEvent> {
    let mut decoder = FunctionEventDecoder::new(profile);
    let mut events = Vec::new();
    for payload in payloads {
        events.extend(decoder.push(payload).unwrap());
    }
    decoder.finish().unwrap();
    events
}

#[test]
fn text_deltas_materialize_to_one_assistant_message() {
    let events = [
        StreamEvent::TextStarted {
            item: ItemId::new(1),
            part: PartId::new(1),
        },
        text_delta(1, 1, "Hel"),
        text_delta(1, 1, "lo"),
        StreamEvent::TextFinished {
            item: ItemId::new(1),
            part: PartId::new(1),
        },
        StreamEvent::Terminal(StreamTerminal::Completed),
    ];
    let response = materialize(&apply(&events).unwrap()).unwrap();
    assert_eq!(response.completion(), Some(Completion::Stop));
    let Item::Message(message) = &response.items()[0].1 else {
        panic!("message");
    };
    assert_eq!(message.role, MessageRole::Assistant);
    let ContentPart::Text(text) = &message.parts[0].content else {
        panic!("text");
    };
    assert_eq!(text.as_str(), "Hello");
    let mut dropped = events.to_vec();
    dropped.remove(2);
    let shortened = materialize(&apply(&dropped).unwrap()).unwrap();
    let Item::Message(message) = &shortened.items()[0].1 else {
        panic!("message");
    };
    let ContentPart::Text(text) = &message.parts[0].content else {
        panic!("text");
    };
    assert_eq!(text.as_str(), "Hel");
}

#[test]
fn unfinished_text_and_length_do_not_become_success() {
    assert!(matches!(
        apply(&[
            StreamEvent::TextStarted {
                item: ItemId::new(1),
                part: PartId::new(1),
            },
            text_delta(1, 1, "Hel"),
            StreamEvent::Terminal(StreamTerminal::Completed),
        ]),
        Err(EventError::Lifecycle)
    ));
    let chat = [
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":"length"}]}),
    ];
    let events = decode(Profile::Chat, &chat);
    assert!(
        events
            .iter()
            .any(|event| { matches!(event, StreamEvent::Terminal(StreamTerminal::Incomplete)) })
    );
    assert_eq!(
        materialize(&apply(&events).unwrap()).unwrap().outcome(),
        openbridge::semantic::task::generation::Outcome::Incomplete
    );
}

#[test]
fn chat_and_responses_text_fragments_match_independent_static_output() {
    let chat = [
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}),
        json!({"id":"response_1","object":"chat.completion.chunk","created":10,"model":"fixture-model","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}]}),
    ];
    let events = decode(Profile::Chat, &chat);
    let response = materialize(&apply(&events).unwrap()).unwrap();
    let fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    let metadata = metadata();
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
            ["content"],
        json!("Hello")
    );
    let responses = [
        json!({"type":"response.created","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"in_progress","output":[]}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"item_1","type":"message","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","output_index":0,"content_index":0,"item_id":"item_1","part":{"type":"output_text","text":"","annotations":[]}}),
        json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"item_id":"item_1","delta":"He"}),
        json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"item_id":"item_1","delta":"llo"}),
        json!({"type":"response.output_text.done","output_index":0,"content_index":0,"item_id":"item_1","text":"Hello"}),
        json!({"type":"response.content_part.done","output_index":0,"content_index":0,"item_id":"item_1","part":{"type":"output_text","text":"Hello","annotations":[]}}),
        json!({"type":"response.output_item.done","output_index":0,"item":{"id":"item_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Hello","annotations":[]}]}}),
        json!({"type":"response.completed","response":{"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"completed","output":[{"id":"item_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Hello","annotations":[]}]}]}}),
    ];
    let events = decode(Profile::Responses, &responses);
    let other = materialize(&apply(&events).unwrap()).unwrap();
    let Item::Message(message) = &other.items()[0].1 else {
        panic!("message");
    };
    let ContentPart::Text(text) = &message.parts[0].content else {
        panic!("text");
    };
    assert_eq!(text.as_str(), "Hello");
    let mut fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    fidelity
        .record_response_item_id(other.items()[0].0, "item_1")
        .unwrap();
    let static_responses = lower_response(
        &other,
        &fidelity,
        &metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        openbridge::protocol::openai::responses::encode_response(&static_responses).unwrap()["output"]
            [0]["content"][0]["text"],
        json!("Hello")
    );
}

#[test]
fn encoded_text_deltas_drop_deleted_fragments() {
    let events = [
        StreamEvent::TextStarted {
            item: ItemId::new(1),
            part: PartId::new(1),
        },
        text_delta(1, 1, "Hel"),
        text_delta(1, 1, "lo"),
        StreamEvent::TextFinished {
            item: ItemId::new(1),
            part: PartId::new(1),
        },
        StreamEvent::Terminal(StreamTerminal::Completed),
    ];
    let mut encoder = FunctionEventEncoder::new(Profile::Chat, metadata()).unwrap();
    let fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    let wire: Vec<_> = events
        .iter()
        .flat_map(|event| encoder.encode(event, &fidelity).unwrap())
        .collect();
    assert!(
        wire.iter()
            .any(|chunk| chunk["choices"][0]["delta"]["content"] == "lo")
    );
    let kept: Vec<_> = events
        .into_iter()
        .filter(
            |event| !matches!(event, StreamEvent::TextDelta { fragment, .. } if fragment == "lo"),
        )
        .collect();
    let mut encoder = FunctionEventEncoder::new(Profile::Chat, metadata()).unwrap();
    let wire: Vec<_> = kept
        .iter()
        .flat_map(|event| encoder.encode(event, &fidelity).unwrap())
        .collect();
    let rendered = serde_json::to_string(&wire).unwrap();
    assert!(!rendered.contains("lo"));
    assert!(rendered.contains("Hel"));
}
