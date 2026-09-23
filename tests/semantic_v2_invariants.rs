//! Representability and budget admission must agree with emitted wire, including transforms.
#[path = "support/semantic_events.rs"]
mod support;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, lower_request, lower_response,
    },
    protocol::{
        fidelity::FidelityRecords,
        openai::{Profile, responses},
    },
    semantic::task::generation::*,
};
use serde_json::json;
use support::*;
#[test]
fn partial_history_cannot_lose_item_status_on_chat() {
    let d = responses::decode_generation(&json!({"input":[call_item("fc","c","{","incomplete")]}))
        .unwrap();
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Chat, Contract::full()).is_err());
}
#[test]
fn chat_finish_cannot_replace_mixed_message_and_call_status() {
    let d=responses::decode_response(&envelope("incomplete",json!([{"id":"m","type":"message","role":"assistant","status":"completed","content":[]},call_item("fc","c","{","incomplete")]))).unwrap();
    let m = metadata();
    let mut items = d.semantic.items().to_vec();
    let owner = items[0].0;
    let Item::ToolCall(call) = &mut items[1].1 else {
        panic!("call");
    };
    call.message = Some(owner);
    let response = GenerationResponse::unfinished(items, Outcome::Incomplete).unwrap();
    assert!(matches!(
        lower_response(&response, &d.fidelity, &m, Profile::Chat, Contract::full()),
        Err(openbridge::lowering::generation::RepresentationError::Terminal)
    ));
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
fn empty_message_wire_identity_cannot_collide_with_another_item() {
    let items = vec![
        (
            ItemId::new(1),
            Item::Message(Message {
                role: MessageRole::Assistant,
                status: ItemLifecycle::Completed,
                parts: vec![],
            }),
        ),
        (
            ItemId::new(2),
            Item::Message(Message {
                role: MessageRole::Assistant,
                status: ItemLifecycle::Completed,
                parts: vec![Part {
                    id: PartId::new(1),
                    content: ContentPart::Text(text("hello").into()),
                }],
            }),
        ),
    ];
    let response = GenerationResponse::new(items, Completion::Stop).unwrap();
    let mut f = FidelityRecords::default();
    f.record_response_item_id(ItemId::new(2), "item_1").unwrap();
    assert!(
        lower_response(
            &response,
            &f,
            &metadata(),
            Profile::Responses,
            Contract::full()
        )
        .is_err()
    );
}
#[test]
fn transformed_total_request_budget_includes_tools_and_history() {
    let payload = "x".repeat(MAX_TEXT_BYTES - 100);
    let items = (0..4)
        .map(|i| {
            (
                ItemId::new(i),
                Item::Message(Message {
                    role: MessageRole::User,
                    status: ItemLifecycle::Completed,
                    parts: vec![Part {
                        id: PartId::new(i),
                        content: ContentPart::Text(text(&payload).into()),
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
