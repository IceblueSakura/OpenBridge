//! Refusal and non-success terminals for the text and function-call path.
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, RepresentationError, lower_response,
    },
    protocol::openai::{Profile, ResponseMetadata, chat, responses},
    semantic::task::generation::{ContentPart, Item, ItemId, Outcome},
};
use serde_json::{Value, json};

fn metadata() -> ResponseMetadata {
    ResponseMetadata {
        id: "response_1".into(),
        model: "fixture-model".into(),
        created: 10,
    }
}
fn chat_refusal() -> Value {
    json!({"id":"response_1","object":"chat.completion","created":10,"model":"fixture-model",
        "choices":[{"index":0,"message":{"role":"assistant","content":null,"refusal":"Cannot do that."},"finish_reason":"stop"}],
        "usage":null})
}
fn responses_refusal() -> Value {
    json!({"id":"response_1","object":"response","created_at":10,"model":"fixture-model","status":"completed",
        "output":[{"id":"item_1","type":"message","role":"assistant","status":"completed","content":[{"type":"refusal","refusal":"Cannot do that."}]}],
        "usage":null})
}

#[test]
fn refusal_is_not_text_and_user_refusal_fails() {
    let decoded = chat::decode_response(&chat_refusal()).unwrap();
    let Item::Message(message) = &decoded.semantic.items()[0].1 else {
        panic!("message");
    };
    assert!(matches!(message.parts[0].content, ContentPart::Refusal(_)));
    let fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    let metadata = metadata();
    let chat = lower_response(
        &decoded.semantic,
        &fidelity,
        &metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        chat::encode_response(&chat).unwrap()["choices"][0]["message"]["refusal"],
        json!("Cannot do that.")
    );
    let mut responses_fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    responses_fidelity
        .record_response_item_id(ItemId::new(1), "item_1")
        .unwrap();
    let responses = lower_response(
        &decoded.semantic,
        &responses_fidelity,
        &metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        responses::encode_response(&responses).unwrap()["output"][0]["content"][0]["type"],
        json!("refusal")
    );
    let other = responses::decode_response(&responses_refusal()).unwrap();
    let Item::Message(message) = &other.semantic.items()[0].1 else {
        panic!("message");
    };
    let ContentPart::Refusal(text) = &message.parts[0].content else {
        panic!("refusal");
    };
    assert_eq!(text.as_str(), "Cannot do that.");
    let mut history = json!({"messages":[{"role":"user","content":null,"refusal":"no"}]});
    assert!(chat::decode_generation(&history).is_err());
    history["messages"][0]["role"] = json!("assistant");
    history["messages"][0]["content"] = json!("visible");
    assert!(chat::decode_generation(&history).is_err());
}

#[test]
fn incomplete_and_failed_responses_do_not_encode_as_success() {
    let mut chat_wire = chat_refusal();
    chat_wire["choices"][0]["message"] = json!({"role":"assistant","content":"Partial"});
    chat_wire["choices"][0]["finish_reason"] = json!("length");
    let decoded = chat::decode_response(&chat_wire).unwrap();
    assert_eq!(decoded.semantic.outcome(), Outcome::Incomplete);
    assert!(decoded.semantic.completion().is_none());
    let fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    let metadata = metadata();
    let encoded = lower_response(
        &decoded.semantic,
        &fidelity,
        &metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    assert_eq!(
        responses::encode_response(&encoded).unwrap()["status"],
        json!("incomplete")
    );
    let mut failed = responses_refusal();
    failed["status"] = json!("failed");
    failed["output"][0]["status"] = json!("incomplete");
    let decoded = responses::decode_response(&failed).unwrap();
    assert_eq!(decoded.semantic.outcome(), Outcome::Failed);
    let mut fidelity = openbridge::protocol::fidelity::FidelityRecords::default();
    fidelity
        .record_response_item_id(decoded.semantic.items()[0].0, "item_1")
        .unwrap();
    assert!(matches!(
        lower_response(
            &decoded.semantic,
            &fidelity,
            &metadata,
            Profile::Chat,
            Contract::full()
        ),
        Err(RepresentationError::Terminal)
    ));
    assert_eq!(
        responses::encode_response(
            &lower_response(
                &decoded.semantic,
                &fidelity,
                &metadata,
                Profile::Responses,
                Contract::full()
            )
            .unwrap()
        )
        .unwrap()["status"],
        json!("failed")
    );
}
