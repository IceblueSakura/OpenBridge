//! Text/refusal parts preserve boundaries, empty values and independent item lifecycle.
#[path = "support/semantic_events.rs"]
mod support;
use openbridge::{
    protocol::{
        fidelity::FidelityRecords,
        openai::{Profile, events::EventDecoder},
    },
    semantic::task::generation::*,
};
use serde_json::json;
use support::*;
#[test]
fn independent_text_wire_decodes_empty_and_multiple_parts() {
    let mut d = EventDecoder::new(Profile::Responses);
    d.push(&created()).unwrap();
    d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"m","type":"message","role":"assistant","status":"in_progress","content":[]}})).unwrap();
    for (i, s) in ["", "你好"].into_iter().enumerate() {
        for v in [
            json!({"type":"response.content_part.added","output_index":0,"item_id":"m","content_index":i,"part":{"type":"output_text","text":"","annotations":[]}}),
            json!({"type":"response.output_text.delta","output_index":0,"item_id":"m","content_index":i,"delta":s}),
            json!({"type":"response.output_text.done","output_index":0,"item_id":"m","content_index":i,"text":s}),
            json!({"type":"response.content_part.done","output_index":0,"item_id":"m","content_index":i,"part":{"type":"output_text","text":s,"annotations":[]}}),
        ] {
            d.push(&v).unwrap();
        }
    }
    let m = json!({"id":"m","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"","annotations":[]},{"type":"output_text","text":"你好","annotations":[]}]});
    d.push(&json!({"type":"response.output_item.done","output_index":0,"item":m}))
        .unwrap();
    d.push(&json!({"type":"response.completed","response":envelope("completed",json!([m]))}))
        .unwrap();
    let r = d.materialize().unwrap().semantic;
    let Item::Message(m) = &r.items()[0].1 else {
        panic!()
    };
    assert_eq!(m.parts.len(), 2);
    assert_ne!(m.parts[0].id, m.parts[1].id);
    assert_eq!(
        m.parts[0].content,
        ContentPart::Text(
            openbridge::semantic::value::Text::allowing_empty("", "test", 1).unwrap()
        )
    );
}
#[test]
fn independently_constructed_text_ir_encodes_expected_boundaries_and_refusal() {
    let mut e = vec![StreamEvent::Started, start(9, ItemKind::Message)];
    e.extend(part(9, 40, PartKind::Text, ""));
    e.extend(part(9, 70, PartKind::Refusal, "Cannot comply"));
    e.push(close(9, ItemLifecycle::Completed));
    e.push(terminal(StreamTerminal::Completed));
    let wire = encode(&e, Profile::Responses, &FidelityRecords::default());
    assert_eq!(
        wire.last().unwrap()["response"]["output"][0]["content"],
        json!([{"type":"output_text","text":"","annotations":[]},{"type":"refusal","refusal":"Cannot comply"}])
    );
    assert!(
        wire.iter()
            .any(|v| v["type"] == "response.refusal.delta" && v["delta"] == "Cannot comply")
    );
}
#[test]
fn text_delta_deletion_and_replacement_change_all_encoder_output() {
    let mut e = vec![StreamEvent::Started, start(1, ItemKind::Message)];
    e.extend(part(1, 9, PartKind::Text, "old"));
    e.push(close(1, ItemLifecycle::Completed));
    e.push(terminal(StreamTerminal::Completed));
    for event in &mut e {
        if let StreamEvent::Delta { fragment, .. } = event {
            *fragment = "new".into();
        }
    }
    for p in [Profile::Responses, Profile::Chat] {
        let wire = encode(&e, p, &FidelityRecords::default());
        assert!(!serde_json::to_string(&wire).unwrap().contains("old"));
        assert!(serde_json::to_string(&wire).unwrap().contains("new"));
    }
}
#[test]
fn part_close_does_not_close_item_and_snapshot_grammar_is_checked() {
    let mut d = EventDecoder::new(Profile::Responses);
    d.push(&created()).unwrap();
    d.push(&json!({"type":"response.output_item.added","output_index":0,"item":{"id":"m","type":"message","role":"assistant","status":"in_progress","content":[]}})).unwrap();
    d.push(&json!({"type":"response.content_part.added","output_index":0,"item_id":"m","content_index":0,"part":{"type":"output_text","text":""}})).unwrap();
    d.push(&json!({"type":"response.output_text.done","output_index":0,"item_id":"m","content_index":0,"text":""})).unwrap();
    d.push(&json!({"type":"response.content_part.done","output_index":0,"item_id":"m","content_index":0,"part":{"type":"output_text","text":""}})).unwrap();
    assert!(d.push(&json!({"type":"response.completed","response":envelope("completed",json!([{"id":"m","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"","annotations":[]}]}]))})).is_err());
}
#[test]
fn empty_failed_and_cancelled_static_event_closure_keeps_details() {
    for (terminal, status) in [
        (StreamTerminal::Failed, "failed"),
        (StreamTerminal::Cancelled, "cancelled"),
        (StreamTerminal::Incomplete, "incomplete"),
    ] {
        let details = match terminal {
            StreamTerminal::Failed => TerminalDetails {
                error: Some(ResponseError {
                    code: text("server_error"),
                    message: text("synthetic error"),
                    param: None,
                }),
                incomplete: None,
            },
            StreamTerminal::Incomplete => TerminalDetails {
                error: None,
                incomplete: Some(IncompleteReason::MaxOutputTokens),
            },
            _ => TerminalDetails::default(),
        };
        let e = vec![
            StreamEvent::Started,
            StreamEvent::Terminal {
                terminal,
                details: details.clone(),
            },
        ];
        let wire = encode(&e, Profile::Responses, &FidelityRecords::default());
        assert_eq!(wire.last().unwrap()["response"]["status"], status);
        assert_eq!(wire.last().unwrap()["response"]["output"], json!([]));
        let mut d = EventDecoder::new(Profile::Responses);
        for v in wire {
            d.push(&v).unwrap();
        }
        assert_eq!(d.materialize().unwrap().semantic.details(), &details);
    }
}
#[test]
fn chat_refusal_and_empty_text_survive_usage_and_done() {
    for (kind, value) in [
        (PartKind::Refusal, "no"),
        (PartKind::Refusal, ""),
        (PartKind::Text, ""),
    ] {
        let mut e = vec![StreamEvent::Started, start(1, ItemKind::Message)];
        e.extend(part(1, 1, kind, value));
        e.push(close(1, ItemLifecycle::Completed));
        e.push(StreamEvent::Usage(Usage {
            input_tokens: 3,
            output_tokens: 2,
            total_tokens: 5,
            reasoning_tokens: Some(1),
            cached_input_tokens: Some(0),
        }));
        e.push(terminal(StreamTerminal::Completed));
        let wire = encode(&e, Profile::Chat, &FidelityRecords::default());
        assert_eq!(wire.last().unwrap()["choices"], json!([]));
        let mut d = EventDecoder::new(Profile::Chat);
        for v in wire {
            d.push(&v).unwrap();
        }
        d.done().unwrap();
        assert_eq!(
            d.materialize().unwrap().semantic,
            materialize(&apply(&e).unwrap()).unwrap()
        );
    }
}
