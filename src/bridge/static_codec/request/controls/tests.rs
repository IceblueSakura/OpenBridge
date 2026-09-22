use super::super::{decode_request, encode_native_request};
use super::*;

fn request(protocol: ApiProtocol) -> super::super::WireRequest {
    let mut source = match protocol {
        ApiProtocol::ChatCompletions => json!({
            "model": "public", "messages": [{"role": "user", "content": [
                {"type": "text", "text": "look"},
                {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
            ]}], "max_tokens": 40, "max_completion_tokens": 80
        }),
        ApiProtocol::Responses => {
            json!({"model": "public", "input": "hello", "max_output_tokens": 80})
        }
    };
    source.as_object_mut().unwrap().extend(
        json!({
            "temperature": 1, "top_p": 0.8, "top_k": 12, "n": 1, "seed": 9,
            "frequency_penalty": 0.2, "presence_penalty": 0.4, "stop": "OLD"
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    decode_request(protocol, source.as_object().unwrap(), 8192).unwrap()
}

#[test]
fn native_wire_uses_replaced_ir_controls_without_changing_other_semantics() {
    for protocol in [ApiProtocol::ChatCompletions, ApiProtocol::Responses] {
        let mut request = request(protocol);
        let mut expected = request.source.clone();
        expected.extend(
            json!({
                "model": "target", "temperature": 0.3, "top_p": 0.5, "top_k": 24,
                "n": 2, "seed": -10, "frequency_penalty": -0.2,
                "presence_penalty": -0.4, "stop": ["NEW", "DONE"]
            })
            .as_object()
            .unwrap()
            .clone(),
        );
        let controls = GenerationControls::new(Some(80), Some(2))
            .unwrap()
            .with_sampling(Some(0.3), Some(0.5), Some(24))
            .unwrap()
            .with_penalties(Some(-0.2), Some(-0.4))
            .unwrap()
            .with_seed(Some(-10))
            .with_stop(Some(vec![
                StopSequence::new("NEW", 8192).unwrap(),
                StopSequence::new("DONE", 8192).unwrap(),
            ]));
        request.semantic = request.semantic.with_controls(controls).unwrap();
        let actual: Value =
            serde_json::from_slice(&encode_native_request(&request, "target").unwrap()).unwrap();
        assert_eq!(actual, Value::Object(expected));
    }
}

#[test]
fn native_wire_does_not_restore_deleted_ir_controls() {
    for protocol in [ApiProtocol::ChatCompletions, ApiProtocol::Responses] {
        let mut request = request(protocol);
        let mut expected = request.source.clone();
        for field in [
            "temperature",
            "top_p",
            "top_k",
            "n",
            "seed",
            "frequency_penalty",
            "presence_penalty",
            "stop",
        ] {
            expected.remove(field);
        }
        expected.insert("model".to_owned(), json!("target"));
        request.semantic = request
            .semantic
            .with_controls(GenerationControls::new(Some(80), None).unwrap())
            .unwrap();
        let actual: Value =
            serde_json::from_slice(&encode_native_request(&request, "target").unwrap()).unwrap();
        assert_eq!(actual, Value::Object(expected));
    }
}

#[test]
fn native_wire_can_add_controls_absent_from_the_source() {
    let source = json!({"model": "public", "input": "hello"});
    let mut request =
        decode_request(ApiProtocol::Responses, source.as_object().unwrap(), 8192).unwrap();
    request.semantic = request
        .semantic
        .with_controls(
            GenerationControls::default()
                .with_seed(Some(42))
                .with_stop(Some(Vec::new())),
        )
        .unwrap();
    let actual: Value =
        serde_json::from_slice(&encode_native_request(&request, "target").unwrap()).unwrap();
    assert_eq!(
        actual,
        json!({"model": "target", "input": "hello", "seed": 42, "stop": []})
    );
}

#[test]
fn native_presence_keeps_null_as_a_hint_but_zero_and_empty_stop_values_active() {
    let null_source = json!({"model": "public", "input": "hello", "seed": null, "stop": null});
    let null_request = decode_request(
        ApiProtocol::Responses,
        null_source.as_object().unwrap(),
        8192,
    )
    .unwrap();
    assert_eq!(null_request.semantic.controls().seed(), None);
    assert_eq!(null_request.semantic.controls().stop(), None);
    let null_encoded: Value =
        serde_json::from_slice(&encode_native_request(&null_request, "target").unwrap()).unwrap();
    let mut null_expected = null_source;
    null_expected["model"] = json!("target");
    assert_eq!(null_encoded, null_expected);

    let active_source = json!({"model": "public", "input": "hello", "seed": 0, "stop": []});
    let active_request = decode_request(
        ApiProtocol::Responses,
        active_source.as_object().unwrap(),
        8192,
    )
    .unwrap();
    assert_eq!(active_request.semantic.controls().seed(), Some(0));
    assert_eq!(
        active_request.semantic.controls().stop(),
        Some([].as_slice())
    );
    let active_encoded: Value =
        serde_json::from_slice(&encode_native_request(&active_request, "target").unwrap()).unwrap();
    let mut active_expected = active_source;
    active_expected["model"] = json!("target");
    assert_eq!(active_encoded, active_expected);

    let empty_string_source = json!({"model": "public", "input": "hello", "stop": ""});
    let empty_string_request = decode_request(
        ApiProtocol::Responses,
        empty_string_source.as_object().unwrap(),
        8192,
    )
    .unwrap();
    let stop = empty_string_request.semantic.controls().stop().unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0].as_str(), "");
    let empty_string_encoded: Value =
        serde_json::from_slice(&encode_native_request(&empty_string_request, "target").unwrap())
            .unwrap();
    assert_eq!(empty_string_encoded["stop"], "");
}
