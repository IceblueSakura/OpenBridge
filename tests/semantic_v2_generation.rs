use openbridge::{
    lowering::generation::{GenerationRepresentationContract, check},
    protocol::openai::{chat, responses},
    semantic::task::generation::{GenerationRequirements, Item},
};
#[test]
fn chat_ir_responses_vertical_slice() {
    let source = serde_json::json!({"messages":[{"role":"system","content":"Be precise."},{"role":"user","content":"Hello"}],"temperature":0.25,"max_completion_tokens":128});
    let d = chat::decode_generation(&source).unwrap();
    let q = GenerationRequirements::derive(&d.semantic);
    assert_eq!(q.instruction_count, 1);
    assert_eq!(q.message_count, 1);
    check(&d.semantic, GenerationRepresentationContract::full()).unwrap();
    let target = encode(&d.semantic, true);
    assert_eq!(target["input"][1]["content"][0]["text"], "Hello");
    assert_eq!(target["max_output_tokens"], 128);
}
#[test]
fn ir_deletion_controls_both_encoders() {
    let source = serde_json::json!({"messages":[{"role":"system","content":"obsolete"},{"role":"user","content":"keep"}]});
    let d = chat::decode_generation(&source).unwrap();
    let ir = d
        .semantic
        .retain_items(|_, item| !matches!(item, Item::Instruction(_)))
        .unwrap();
    let chat_out = encode(&ir, false);
    let responses_out = encode(&ir, true);
    assert_eq!(chat_out["messages"].as_array().unwrap().len(), 1);
    assert_eq!(responses_out["input"].as_array().unwrap().len(), 1);
    assert_eq!(chat_out["messages"][0]["content"], "keep");
    assert_eq!(responses_out["input"][0]["content"][0]["text"], "keep");
}
#[test]
fn unrepresentable_semantics_fail_before_encoding() {
    let source = serde_json::json!({"messages":[{"role":"system","content":"required"},{"role":"user","content":"hello"}]});
    let d = chat::decode_generation(&source).unwrap();
    let c = GenerationRepresentationContract {
        instructions: false,
        ..GenerationRepresentationContract::full()
    };
    assert!(check(&d.semantic, c).is_err());
}

#[test]
fn responses_and_chat_decode_to_same_semantics() {
    let chat_wire = serde_json::json!({"messages":[{"role":"developer","content":"Be brief."},{"role":"user","content":"Hello"}],"max_completion_tokens":64});
    let responses_wire = serde_json::json!({"input":[{"role":"developer","content":"Be brief."},{"role":"user","content":[{"type":"input_text","text":"Hello"}]}],"max_output_tokens":64});
    let a = chat::decode_generation(&chat_wire).unwrap().semantic;
    let b = responses::decode_generation(&responses_wire)
        .unwrap()
        .semantic;
    assert_eq!(
        GenerationRequirements::derive(&a),
        GenerationRequirements::derive(&b)
    );
    assert_eq!(encode(&a, false), encode(&b, false));
    assert_eq!(encode(&a, true), encode(&b, true));
}

fn encode(
    r: &openbridge::semantic::task::generation::GenerationRequest,
    responses_target: bool,
) -> serde_json::Value {
    use openbridge::{
        lowering::generation::lower_request,
        protocol::{fidelity::FidelityRecords, openai::Profile},
    };
    let fidelity = FidelityRecords::default();
    let profile = if responses_target {
        Profile::Responses
    } else {
        Profile::Chat
    };
    let t = lower_request(
        r,
        &fidelity,
        profile,
        GenerationRepresentationContract::full(),
    )
    .unwrap();
    if responses_target {
        responses::encode_generation(&t).unwrap()
    } else {
        chat::encode_generation(&t).unwrap()
    }
}
