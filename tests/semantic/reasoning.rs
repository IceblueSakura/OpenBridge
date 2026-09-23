//! Reasoning ownership, opaque replay, mixed tool continuation and resource failures.
use crate::events_support::*;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, lower_request, lower_response,
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
fn reasoning(status: &str) -> Value {
    json!({"id":"rs","type":"reasoning","status":status,"summary":[{"type":"summary_text","text":"summary"}],"content":[{"type":"reasoning_text","text":"reasoning"}],"encrypted_content":"final-synthetic"})
}
fn wire_part(output: usize, id: &str, index: usize, summary: bool, s: &str) -> Vec<Value> {
    let (key, stem) = if summary {
        ("summary_index", "reasoning_summary_text")
    } else {
        ("content_index", "reasoning_text")
    };
    let mut values = vec![];
    if summary {
        values.push(json!({"type":"response.reasoning_summary_part.added","output_index":output,"item_id":id,"part":{"type":"summary_text","text":""}}));
    }
    values.push(json!({"type":format!("response.{stem}.delta"),"output_index":output,"item_id":id,"delta":s}));
    values.push(
        json!({"type":format!("response.{stem}.done"),"output_index":output,"item_id":id,"text":s}),
    );
    if summary {
        values.push(json!({"type":"response.reasoning_summary_part.done","output_index":output,"item_id":id,"part":{"type":"summary_text","text":s}}));
    }
    for v in &mut values {
        v[key] = json!(index);
    }
    values
}
#[test]
fn controls_keep_absence_empty_none_disabled_and_encrypted_output_distinct() {
    let input = json!([{"role":"user","content":"hello"}]);
    let absent = responses::decode_generation(&json!({"input":input})).unwrap();
    let empty = responses::decode_generation(&json!({"input":input,"reasoning":{}})).unwrap();
    let none=responses::decode_generation(&json!({"input":input,"reasoning":{"effort":"none","summary":false},"include":["reasoning.encrypted_content"]})).unwrap();
    assert_ne!(absent.semantic.reasoning(), empty.semantic.reasoning());
    assert_eq!(
        none.semantic.reasoning().effort(),
        Some(ReasoningEffort::None)
    );
    assert_eq!(
        none.semantic.reasoning().summary(),
        Some(ReasoningSummary::Disabled)
    );
    let t = lower_request(
        &none.semantic,
        &none.fidelity,
        Profile::Responses,
        contract(),
    )
    .unwrap();
    let v = responses::encode_generation(&t).unwrap();
    assert_eq!(v["reasoning"], json!({"effort":"none","summary":false}));
    assert_eq!(v["include"], json!(["reasoning.encrypted_content"]));
    assert!(lower_request(&none.semantic, &none.fidelity, Profile::Chat, contract()).is_err());
    for bad in [
        json!({"effort":"none","summary":"auto"}),
        json!({"summary":[]}),
        json!({"summary":true}),
    ] {
        assert!(responses::decode_generation(&json!({"input":input,"reasoning":bad})).is_err());
    }
    let chat =
        chat::decode_generation(&json!({"messages":input,"reasoning_effort":"low"})).unwrap();
    assert_eq!(
        chat.semantic.reasoning().effort(),
        Some(ReasoningEffort::Low)
    );
}
#[test]
fn standard_max_effort_is_representable_but_unknown_labels_fail() {
    let d = responses::decode_generation(
        &json!({"input":[{"role":"user","content":"hello"}],"reasoning":{"effort":"max"}}),
    )
    .unwrap();
    for p in [Profile::Chat, Profile::Responses] {
        assert!(lower_request(&d.semantic, &d.fidelity, p, Contract::full()).is_ok());
    }
    assert!(
        responses::decode_generation(
            &json!({"input":"hello","reasoning":{"effort":"unregistered"}})
        )
        .is_err()
    );
}

#[test]
fn independent_reasoning_stream_closes_and_preserves_both_part_domains_and_token() {
    let mut d = EventDecoder::new(Profile::Responses).with_replay_origin(origin());
    let mut events = d.push(&created()).unwrap();
    events.extend(d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"rs","type":"reasoning","status":"in_progress","summary":[],"encrypted_content":"partial-synthetic"}})).unwrap());
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ItemStarted {
            replay: Some(ReasoningReplay {
                value: EncryptedReasoning::Partial(_),
                ..
            }),
            ..
        }
    )));
    for v in wire_part(0, "rs", 0, true, "summary")
        .into_iter()
        .chain(wire_part(0, "rs", 0, false, "reasoning"))
    {
        events.extend(d.push(&v).unwrap());
    }
    events.extend(d.push(&json!({"type":"response.output_item.done","output_index":0,"item":reasoning("completed")})).unwrap());
    events.extend(d.push(&json!({"type":"response.completed","response":envelope("completed",json!([reasoning("completed")]))})).unwrap());
    let decoded = d.materialize().unwrap();
    let Item::Reasoning(r) = &decoded.semantic.items()[0].1 else {
        panic!()
    };
    assert_eq!(r.parts[0].1, ReasoningContent::Summary(text("summary")));
    assert_eq!(r.parts[1].1, ReasoningContent::Text(text("reasoning")));
    assert_ne!(r.parts[0].0, r.parts[1].0);
    assert_eq!(
        decoded.fidelity.encrypted_reasoning_replay(ItemId::new(1)),
        Some("final-synthetic")
    );
    let encoded = encode(&events, Profile::Responses, &decoded.fidelity);
    assert_eq!(
        encoded.last().unwrap()["response"]["output"],
        json!([reasoning("completed")])
    );
}
#[test]
fn final_event_token_is_authority_and_stale_fidelity_cannot_restore_it() {
    let mut d = responses::decode_response(&envelope("completed", json!([reasoning("completed")])))
        .unwrap();
    d.fidelity.bind_replay_origin(&origin()).unwrap();
    let mut events = vec![StreamEvent::Started, start(1, ItemKind::Reasoning)];
    events.push(StreamEvent::ItemFinished {
        item: ItemId::new(1),
        status: ItemLifecycle::Completed,
        replay: Some(token("new-synthetic", true)),
    });
    events.push(terminal(StreamTerminal::Completed));
    let wire = encode(&events, Profile::Responses, &d.fidelity);
    assert_eq!(
        wire.last().unwrap()["response"]["output"][0]["encrypted_content"],
        "new-synthetic"
    );
    assert!(
        !wire
            .iter()
            .any(|v| v.to_string().contains("final-synthetic"))
    );
    events[2] = close(1, ItemLifecycle::Completed);
    let wire = encode(&events, Profile::Responses, &d.fidelity);
    assert!(
        wire.last().unwrap()["response"]["output"][0]
            .get("encrypted_content")
            .is_none()
    );
}
#[test]
fn opaque_origin_and_owner_dependencies_are_checked_but_deleted_owners_are_irrelevant() {
    let mut d = responses::decode_generation(
        &json!({"input":[reasoning("completed"),{"role":"user","content":"next"}]}),
    )
    .unwrap();
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Responses, contract()).is_err());
    d.fidelity.bind_replay_origin(&origin()).unwrap();
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Responses, contract()).is_ok());
    let mut other = contract();
    other.replay_origin =
        Some(openbridge::semantic::value::ReplayOrigin::new("different-owner").unwrap());
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Responses, other).is_err());
    let mut items = d.semantic.items().to_vec();
    let Item::Reasoning(r) = &mut items[0].1 else {
        panic!()
    };
    r.parts[0].1 = ReasoningContent::Summary(text("changed"));
    let changed = d.semantic.clone().with_items(items).unwrap();
    assert!(lower_request(&changed, &d.fidelity, Profile::Responses, contract()).is_err());
    let deleted = d
        .semantic
        .clone()
        .retain_items(|_, i| !matches!(i, Item::Reasoning(_)))
        .unwrap();
    assert!(lower_request(&deleted, &d.fidelity, Profile::Chat, Contract::full()).is_ok());
    let mut no = contract();
    no.reasoning = false;
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Responses, no).is_err());
}
#[test]
fn partial_final_phases_and_resource_limits_fail_without_panics() {
    let state = apply(&[
        StreamEvent::Started,
        start(1, ItemKind::Reasoning),
        StreamEvent::PartStarted {
            item: ItemId::new(1),
            part: PartId::new(1),
            kind: PartKind::ReasoningText,
        },
    ])
    .unwrap();
    assert!(matches!(
        reduce(
            state,
            StreamEvent::Delta {
                item: ItemId::new(1),
                part: PartId::new(1),
                fragment: "x".repeat(MAX_TEXT_BYTES + 1),
                logprobs: vec![]
            }
        ),
        Err(EventError::Limit)
    ));
    let state = apply(&[StreamEvent::Started, start(1, ItemKind::Reasoning)]).unwrap();
    assert!(
        reduce(
            state,
            StreamEvent::ItemFinished {
                item: ItemId::new(1),
                status: ItemLifecycle::Completed,
                replay: Some(token("partial", false))
            }
        )
        .is_err()
    );
    let mut f = FidelityRecords::default();
    let owner = ReasoningItem {
        parts: vec![],
        status: ItemLifecycle::Completed,
    };
    for i in 0..MAX_ITEMS {
        f.record_replay(ItemId::new(i as u64), token("x", true), &owner)
            .unwrap();
    }
    assert!(
        f.record_replay(ItemId::new(MAX_ITEMS as u64), token("x", true), &owner)
            .is_err()
    );
    let mut f = FidelityRecords::default();
    for i in 0..4 {
        f.record_replay(
            ItemId::new(i),
            token(&"x".repeat(MAX_TEXT_BYTES), true),
            &owner,
        )
        .unwrap();
    }
    assert!(
        f.record_replay(ItemId::new(4), token("x", true), &owner)
            .is_err()
    );
}
#[test]
fn unsupported_snapshot_content_is_not_silently_dropped() {
    let mut d = EventDecoder::new(Profile::Responses);
    d.push(&created()).unwrap();
    d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"rs","type":"reasoning","status":"in_progress","summary":[]}})).unwrap();
    assert!(d.push(&json!({"type":"response.output_item.done","output_index":0,"item":reasoning("completed")})).is_err());
}
#[test]
fn static_incomplete_items_and_empty_failures_keep_their_own_status() {
    let wire = envelope(
        "incomplete",
        json!([
            reasoning("completed"),
            call_item("fc", "c", "{", "incomplete")
        ]),
    );
    let mut d = responses::decode_response(&wire).unwrap();
    d.fidelity.bind_replay_origin(&origin()).unwrap();
    let m = metadata();
    let t = lower_response(&d.semantic, &d.fidelity, &m, Profile::Responses, contract()).unwrap();
    let encoded = responses::encode_response(&t).unwrap();
    assert_eq!(encoded["output"][0]["status"], "completed");
    assert_eq!(encoded["output"][1]["status"], "incomplete");
    let d = responses::decode_response(&envelope("failed", json!([]))).unwrap();
    assert!(lower_response(&d.semantic, &d.fidelity, &m, Profile::Responses, contract()).is_ok());
}
#[test]
fn reasoning_and_parallel_call_results_preserve_continuation_history() {
    let mut events = vec![StreamEvent::Started, start(11, ItemKind::Reasoning)];
    events.extend(part(11, 90, PartKind::Summary, "plan"));
    events.extend(part(11, 80, PartKind::ReasoningText, "check"));
    events.push(StreamEvent::ItemFinished {
        item: ItemId::new(11),
        status: ItemLifecycle::Completed,
        replay: Some(token("first-token", true)),
    });
    events.extend(call(22, 70, "a", "{\"key\":1}"));
    events.extend(call(33, 60, "b", "{\"key\":2}"));
    events.push(StreamEvent::Usage(Usage {
        input_tokens: 4,
        output_tokens: 6,
        total_tokens: 10,
        reasoning_tokens: Some(3),
        cached_input_tokens: Some(0),
        input_cache_write_tokens: None,
    }));
    events.push(terminal(StreamTerminal::Completed));
    let wire = encode(&events, Profile::Responses, &FidelityRecords::default());
    let output = wire.last().unwrap()["response"]["output"].clone();
    assert_eq!(
        output,
        json!([{"id":"item_11","type":"reasoning","status":"completed","summary":[{"type":"summary_text","text":"plan"}],"content":[{"type":"reasoning_text","text":"check"}],"encrypted_content":"first-token"},call_item("item_22","a","{\"key\":1}","completed"),call_item("item_33","b","{\"key\":2}","completed")])
    );
    let mut decoder = EventDecoder::new(Profile::Responses).with_replay_origin(origin());
    for v in wire {
        decoder.push(&v).unwrap();
    }
    let first = decoder.materialize().unwrap();
    assert_eq!(first.semantic.usage().unwrap().reasoning_tokens, Some(3));
    let mut input = vec![json!({"role":"user","content":"check two keys"})];
    input.extend(output.as_array().unwrap().iter().cloned());
    input.extend([
        json!({"type":"function_call_output","call_id":"b","output":"second"}),
        json!({"type":"function_call_output","call_id":"a","output":"first"}),
    ]);
    let request = json!({"input":input,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"},"strict":false}],"include":["reasoning.encrypted_content"]});
    let mut d = responses::decode_generation(&request).unwrap();
    d.fidelity.bind_replay_origin(&origin()).unwrap();
    let encoded = responses::encode_generation(
        &lower_request(&d.semantic, &d.fidelity, Profile::Responses, contract()).unwrap(),
    )
    .unwrap();
    assert_eq!(encoded["input"][0]["content"][0]["text"], "check two keys");
    assert_eq!(encoded["input"][1]["encrypted_content"], "first-token");
    assert_eq!(encoded["input"][4]["call_id"], "b");
    assert_eq!(encoded["input"][5]["output"], "first");
    assert_eq!(encoded["tools"][0]["name"], "lookup");
}
#[test]
fn encoder_rejects_unbound_replay_before_emitting_it() {
    let mut encoder = EventEncoder::new(Profile::Responses, metadata()).unwrap();
    let f = FidelityRecords::default();
    encoder.encode(&StreamEvent::Started, &f).unwrap();
    assert!(
        encoder
            .encode(
                &StreamEvent::ItemStarted {
                    item: ItemId::new(1),
                    kind: ItemKind::Reasoning,
                    replay: Some(token("partial", false))
                },
                &f
            )
            .is_err()
    );
}
