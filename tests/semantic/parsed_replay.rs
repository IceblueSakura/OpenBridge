//! SDK derived `parsed` views are validated replay conveniences over the authoritative
//! raw text; they are never IR state, never emitted, and never admitted as wire facts.
use crate::events_support::*;
use crate::wire;
use openbridge::{
    lowering::generation::{GenerationRepresentationContract as Contract, lower_request},
    protocol::openai::{
        CodecError, DecodedRequest, Profile, chat, chat_envelope, envelope, events::EventDecoder,
        responses,
    },
    semantic::task::generation::*,
};
use serde_json::{Value, json};

const RAW: &str = "{\"ok\":true}";

fn output_part(parsed: Option<Value>) -> Value {
    let mut p = json!({"type":"output_text","text":RAW,"annotations":[]});
    if let Some(v) = parsed {
        p["parsed"] = v;
    }
    p
}
fn responses_input(part: Value) -> Value {
    json!({"model":"fixture-model","input":[{"type":"message","id":"m","role":"assistant","status":"completed","content":[part]}]})
}
fn decode_responses(part: Value) -> Result<DecodedRequest, CodecError> {
    Ok(envelope::decode_request_bytes(&serde_json::to_vec(&responses_input(part)).unwrap())?.task)
}
fn chat_request(message: Value) -> Value {
    json!({"model":"fixture-model","messages":[message]})
}
fn decode_chat(message: Value) -> Result<DecodedRequest, CodecError> {
    Ok(
        chat_envelope::decode_request_bytes(&serde_json::to_vec(&chat_request(message)).unwrap())?
            .task,
    )
}
fn message_text(d: &DecodedRequest) -> String {
    let Item::Message(m) = &d.semantic.items()[0].1 else {
        panic!()
    };
    let ContentPart::Text(t) = &m.parts[0].content else {
        panic!()
    };
    t.as_str().to_owned()
}
fn wire_pair(d: &DecodedRequest) -> (Value, Value) {
    (
        responses::encode_generation(
            &lower_request(
                &d.semantic,
                &d.fidelity,
                Profile::Responses,
                Contract::full(),
            )
            .unwrap(),
        )
        .unwrap(),
        chat::encode_generation(
            &lower_request(&d.semantic, &d.fidelity, Profile::Chat, Contract::full()).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn history_replay_admits_a_consistent_parsed_and_keeps_the_raw_text_authoritative() {
    let expected = json!({"ok":true});
    // Missing, null and a consistent value are one admitted history shape with the same IR.
    let admitted = [
        decode_responses(output_part(None)).unwrap(),
        decode_responses(output_part(Some(Value::Null))).unwrap(),
        decode_responses(output_part(Some(expected.clone()))).unwrap(),
    ];
    assert_eq!(admitted[0].semantic, admitted[1].semantic);
    assert_eq!(admitted[1].semantic, admitted[2].semantic);
    for d in &admitted {
        assert_eq!(message_text(d), RAW);
        let (r, c) = wire_pair(d);
        assert_eq!(r["input"][0]["content"][0]["text"], json!(RAW));
        assert!(r["input"][0]["content"][0].get("parsed").is_none());
        assert_eq!(c["messages"][0]["content"], json!(RAW));
        assert!(c["messages"][0].get("parsed").is_none());
    }
    // The Chat message shell has the same rule on its own wire shape.
    let admitted = [
        decode_chat(json!({"role":"assistant","content":RAW})).unwrap(),
        decode_chat(json!({"role":"assistant","content":RAW,"parsed":Value::Null})).unwrap(),
        decode_chat(json!({"role":"assistant","content":RAW,"parsed":expected})).unwrap(),
    ];
    assert_eq!(admitted[0].semantic, admitted[1].semantic);
    assert_eq!(admitted[1].semantic, admitted[2].semantic);
    for d in &admitted {
        assert_eq!(message_text(d), RAW);
        let (r, c) = wire_pair(d);
        assert_eq!(c["messages"][0]["content"], json!(RAW));
        assert!(c["messages"][0].get("parsed").is_none());
        assert_eq!(r["input"][0]["content"][0]["text"], json!(RAW));
        assert!(!r.to_string().contains("parsed"));
    }
}

#[test]
fn inconsistent_or_underivable_parsed_cannot_pass_admission() {
    // A pydantic-shaped value that is not the raw text's parse cannot prove its derivation.
    for parsed in [
        json!({"ok":true,"extra":1}),
        json!({"ok":"true"}),
        json!([true]),
        json!(true),
        json!("{}"),
    ] {
        assert!(decode_responses(output_part(Some(parsed.clone()))).is_err());
        assert!(decode_chat(json!({"role":"assistant","content":RAW,"parsed":parsed})).is_err());
    }
    // Non-null views of a body that is not valid JSON are underivable.
    let mut part = output_part(Some(json!({"ok":1})));
    part["text"] = json!("hello");
    assert!(decode_responses(part).is_err());
    // A null view of unparseable text is still just "no derived value".
    let mut part = output_part(Some(Value::Null));
    part["text"] = json!("hello");
    assert!(decode_responses(part).is_ok());
    assert!(decode_chat(json!({"role":"assistant","content":"hello","parsed":{"ok":1}})).is_err());
    assert!(
        decode_chat(json!({"role":"assistant","content":"hello","parsed":Value::Null})).is_ok()
    );
    // A missing or null Chat body never derives a non-null view.
    for message in [
        json!({"role":"assistant","content":null,"parsed":{"ok":true}}),
        json!({"role":"assistant","parsed":{"ok":true}}),
        json!({"role":"assistant","content":"","parsed":{"ok":true}}),
    ] {
        assert!(decode_chat(message).is_err());
    }
    // Only assistant messages carry the SDK view; other roles reject the key outright.
    for message in [
        json!({"role":"user","content":RAW,"parsed":{"ok":true}}),
        json!({"role":"user","content":RAW,"parsed":Value::Null}),
        json!({"role":"system","content":"rules","parsed":Value::Null}),
        json!({"role":"developer","content":"rules","parsed":Value::Null}),
        json!({"role":"tool","tool_call_id":"call","content":"out","parsed":Value::Null}),
    ] {
        assert!(decode_chat(message).is_err());
    }
    // Input-text parts are user inputs and never carry a derived view.
    assert!(
        decode_responses(json!({"type":"input_text","text":RAW,"parsed":{"ok":true}})).is_err()
    );
}

#[test]
fn static_responses_stream_records_and_chat_terminals_reject_parsed_views() {
    // A static upstream response never carries client-side views.
    let mut response = wire::response(2);
    response["output"][0]["content"][0]["parsed"] = json!({"ok":false});
    assert!(envelope::decode_response_bytes(&serde_json::to_vec(&response).unwrap()).is_err());
    // The SDK's typed text-done wrapper adds `parsed`; the wire record never has it.
    let mut d = EventDecoder::new(Profile::Responses);
    d.push(&created()).unwrap();
    d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"m","type":"message","role":"assistant","status":"in_progress","content":[]}})).unwrap();
    d.push(&json!({"type":"response.content_part.added","output_index":0,"item_id":"m","content_index":0,"part":{"type":"output_text","text":"","annotations":[]}})).unwrap();
    d.push(&json!({"type":"response.output_text.delta","output_index":0,"item_id":"m","content_index":0,"delta":RAW})).unwrap();
    assert!(
        d.push(&json!({"type":"response.output_text.done","output_index":0,"item_id":"m","content_index":0,"text":RAW,"parsed":{"ok":true}}))
            .is_err()
    );
    // Part snapshots are wire facts; a dumped parsed view poisons them too.
    let mut d = EventDecoder::new(Profile::Responses);
    d.push(&created()).unwrap();
    d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"m","type":"message","role":"assistant","status":"in_progress","content":[]}})).unwrap();
    d.push(&json!({"type":"response.content_part.added","output_index":0,"item_id":"m","content_index":0,"part":{"type":"output_text","text":"","annotations":[]}})).unwrap();
    d.push(&json!({"type":"response.output_text.delta","output_index":0,"item_id":"m","content_index":0,"delta":RAW})).unwrap();
    d.push(&json!({"type":"response.output_text.done","output_index":0,"item_id":"m","content_index":0,"text":RAW})).unwrap();
    assert!(
        d.push(&json!({"type":"response.content_part.done","output_index":0,"item_id":"m","content_index":0,"part":{"type":"output_text","text":RAW,"annotations":[],"parsed":{"ok":true}}}))
            .is_err()
    );
    // The Chat static message and its chunks never echo a derived view either.
    assert!(
        chat_envelope::decode_response_bytes(
            &serde_json::to_vec(&json!({"id":"c","object":"chat.completion","created":1,"model":"fixture-model","choices":[{"index":0,"message":{"role":"assistant","content":RAW,"parsed":{"ok":true}},"finish_reason":"stop"}]})).unwrap()
        )
        .is_err()
    );
    let mut d = EventDecoder::new(Profile::Chat);
    assert!(
        d.push(&json!({"id":"c","object":"chat.completion.chunk","created":1,"model":"fixture-model","choices":[{"index":0,"delta":{"role":"assistant","content":null,"refusal":null,"tool_calls":null,"parsed":{"ok":true}},"finish_reason":null}],"usage":null}))
            .is_err()
    );
}

#[test]
fn replaced_text_cannot_resurrect_a_parsed_value() {
    let d = decode_responses(output_part(Some(json!({"ok":true})))).unwrap();
    let items: Vec<_> = d
        .semantic
        .items()
        .iter()
        .map(|(id, item)| {
            let Item::Message(m) = item else { panic!() };
            let mut m = m.clone();
            let ContentPart::Text(t) = m.parts[0].content.clone() else {
                panic!()
            };
            m.parts[0].content = ContentPart::Text(
                t.replace_text(
                    openbridge::semantic::value::Text::allowing_empty(
                        "{\"ok\":false}",
                        "synthetic",
                        MAX_TEXT_BYTES,
                    )
                    .unwrap(),
                ),
            );
            (*id, Item::Message(m))
        })
        .collect();
    let semantic = d.semantic.clone().with_items(items).unwrap();
    for value in [
        responses::encode_generation(
            &lower_request(&semantic, &d.fidelity, Profile::Responses, Contract::full()).unwrap(),
        )
        .unwrap(),
        chat::encode_generation(
            &lower_request(&semantic, &d.fidelity, Profile::Chat, Contract::full()).unwrap(),
        )
        .unwrap(),
    ] {
        assert!(!value.to_string().contains("parsed"));
        assert!(!value.to_string().contains("{\"ok\":true}"));
    }
}
