//! Independent Native wire oracles for semantic replacement and deletion.

use super::*;
use crate::ir::generation::{
    Candidate, ContentPart, GenerationRequest, InputItem, Message, MessageRole, OutputItem,
    ResponseMessage, TextValue,
};
use serde_json::json;

#[test]
fn native_request_replacement_does_not_replay_old_input() {
    for (protocol, source, expected) in [
        (
            ApiProtocol::ChatCompletions,
            json!({"model":"public","messages":[{"role":"user","content":"old"}]}),
            json!({"model":"target","messages":[{"role":"user","content":"new"}]}),
        ),
        (
            ApiProtocol::Responses,
            json!({"model":"public","input":"old"}),
            json!({"model":"target","input":"new"}),
        ),
    ] {
        let mut decoded =
            request::decode_request(protocol, source.as_object().unwrap(), 8192).unwrap();
        decoded.semantic = GenerationRequest::new(vec![InputItem::Message(
            Message::new(
                MessageRole::User,
                vec![ContentPart::text(TextValue::new("new", 8192).unwrap())],
            )
            .unwrap(),
        )])
        .unwrap();
        let actual: Value =
            serde_json::from_slice(&request::encode_native_request(&decoded, "target").unwrap())
                .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn native_response_replacement_does_not_replay_old_content() {
    let source = json!({
        "id":"resp_test", "object":"response", "status":"completed",
        "output":[{"id":"msg_test","type":"message","role":"assistant","status":"completed",
            "content":[{"type":"output_text","text":"old","annotations":[]}]}]
    });
    let mut decoded = response::decode_response(
        ApiProtocol::Responses,
        source.as_object().unwrap(),
        ReasoningOutput::Unsupported,
        8192,
    )
    .unwrap();
    let candidate = &decoded.semantic.candidates()[0];
    let OutputItem::Message(item) = &candidate.output()[0] else {
        panic!("expected decoded assistant message");
    };
    let replacement = ResponseMessage::new(
        item.id().clone(),
        vec![ContentPart::text(TextValue::new("new", 8192).unwrap())],
        None,
    )
    .unwrap();
    decoded.semantic = GenerationResponse::new(
        decoded.semantic.id().clone(),
        vec![
            Candidate::new(
                candidate.id().clone(),
                vec![OutputItem::Message(replacement)],
                candidate.finish().cloned(),
            )
            .unwrap(),
        ],
        decoded.semantic.status(),
        decoded.semantic.usage().copied(),
        decoded.semantic.extensions().to_vec(),
    )
    .unwrap();
    let actual: Value = serde_json::from_slice(
        &response::encode_native_response(
            &decoded,
            source.as_object().unwrap(),
            ApiProtocol::Responses,
            "public",
            ReasoningOutput::Unsupported,
            8192,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual["output"][0]["content"][0]["text"], json!("new"));
}
