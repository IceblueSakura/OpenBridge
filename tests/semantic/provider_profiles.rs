//! Independent synthetic oracles for scoped provider boundary differences.
use openbridge::{
    adapter::{Adapter, Dialect},
    lowering::generation::GenerationRepresentationContract as Contract,
    protocol::openai::Profile,
    semantic::value::ReplayOrigin,
};
use serde_json::{Value, json};
fn adapter(dialect: Dialect) -> Adapter {
    Adapter::new(
        Profile::Chat,
        dialect,
        Some(ReplayOrigin::new("synthetic-account").unwrap()),
    )
}
fn body() -> Value {
    json!({"id":"c1","object":"chat.completion","model":"m","created":1,
        "choices":[{"index":0,"message":{"role":"assistant","content":"pong","reasoning_content":"think"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":4,"completion_tokens":2,"total_tokens":6}})
}
#[test]
fn provider_token_spelling_is_only_an_outbound_mapping() {
    let client = adapter(Dialect::OpenBridge);
    let request=client.decode_request(br#"{"model":"m","messages":[{"role":"user","content":"hi"}],"max_completion_tokens":17}"#).unwrap();
    let wire = adapter(Dialect::Nvidia)
        .encode_request(&request, "bound", &Contract::full())
        .unwrap();
    assert_eq!(wire["max_tokens"], 17);
    assert!(wire.get("max_completion_tokens").is_none());
    let client_wire = client
        .encode_request(&request, "m", &Contract::full())
        .unwrap();
    assert_eq!(client_wire["max_completion_tokens"], 17);
    assert!(client_wire.get("max_tokens").is_none());
    let vendor_wire =
        br#"{"model":"m","messages":[{"role":"user","content":"hi"}],"max_tokens":29}"#;
    assert_eq!(
        adapter(Dialect::Nvidia)
            .decode_request(vendor_wire)
            .unwrap()
            .task
            .semantic
            .controls()
            .max_output_tokens,
        Some(29)
    );
    assert!(client.decode_request(vendor_wire).is_err());
    assert!(adapter(Dialect::Nvidia).decode_request(br#"{"model":"m","messages":[{"role":"user","content":"hi"}],"max_tokens":29,"max_completion_tokens":29}"#).is_err());
}
#[test]
fn transport_markers_and_stop_diagnostics_do_not_replace_semantic_terminals() {
    let provider = adapter(Dialect::LongCat);
    let mut value = body();
    value["lastOne"] = json!(false);
    value["choices"][0]["delta"] = Value::Null;
    value["choices"][0]["matched_stop"] = json!(2);
    value["usage"]["prompt_tokens_details"] = json!({"cached_tokens":0,"text_tokens":0,"image_tokens":0,"audio_tokens":0,"video_tokens":0});
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    assert!(
        adapter(Dialect::Standard)
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
    let same = provider
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert_eq!(same["choices"][0]["matched_stop"], 2);
    let client = adapter(Dialect::OpenBridge)
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert!(client["choices"][0].get("matched_stop").is_none());
    assert!(client.get("lastOne").is_none());
    let mut stream = provider.event_decoder();
    let chunk = |delta: Value, finish: Value| json!({"id":"c1","object":"chat.completion.chunk","model":"m","created":1,"lastOne":false,"choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
    stream
        .push(&chunk(
            json!({"role":"assistant","reasoning_content":"think"}),
            Value::Null,
        ))
        .unwrap();
    stream
        .push(&chunk(json!({"content":"pong"}), Value::Null))
        .unwrap();
    let mut finish = chunk(json!({}), json!("stop"));
    finish["choices"][0]["matched_stop"] = json!(2);
    finish["created"] = json!(2);
    stream.push(&finish).unwrap();
    stream.push(&json!({"id":"c1","object":"chat.completion.chunk","model":"m","created":3,"lastOne":true,"choices":[],"usage":value["usage"]})).unwrap();
    stream.done().unwrap();
    assert_eq!(stream.materialize().unwrap().semantic, decoded.semantic);
    assert_eq!(
        provider
            .encode_response(&stream.materialize().unwrap(), &Contract::full())
            .unwrap()["choices"][0]["matched_stop"],
        2
    );
    value["usage"]["prompt_tokens_details"]["image_tokens"] = json!(1);
    assert!(
        provider
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
    value["usage"]["prompt_tokens_details"]["image_tokens"] = json!(0);
    value["lastOne"] = json!("true");
    assert!(
        provider
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
    let mut decoder = provider.event_decoder();
    assert!(decoder.push(&json!({"id":"c1","object":"chat.completion.chunk","model":"m","created":1,"choices":[],"lastOne":true})).is_err());
}
#[test]
fn documented_inactive_message_fields_do_not_admit_media_or_legacy_calls() {
    let provider = adapter(Dialect::Bailian);
    let mut value = body();
    value["choices"][0]["message"]["audio"] = Value::Null;
    value["choices"][0]["message"]["function_call"] = Value::Null;
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    let output = adapter(Dialect::OpenBridge)
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert_eq!(output["choices"][0]["message"]["content"], "pong");
    assert!(output["choices"][0]["message"].get("audio").is_none());
    for field in ["audio", "function_call"] {
        let mut invalid = value.clone();
        invalid["choices"][0]["message"][field] = json!({"unsupported":"active"});
        assert!(
            provider
                .decode_response(invalid.to_string().as_bytes())
                .is_err()
        );
    }
    assert!(
        adapter(Dialect::Nvidia)
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
}
#[test]
fn request_diagnostics_are_source_bound_not_public_facts() {
    let provider = adapter(Dialect::Zhipu);
    let mut value = body();
    value["request_id"] = json!("synthetic-request");
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    assert_eq!(
        provider
            .encode_response(&decoded, &Contract::full())
            .unwrap()["request_id"],
        "synthetic-request"
    );
    let other = Adapter::new(
        Profile::Chat,
        Dialect::Zhipu,
        Some(ReplayOrigin::new("other").unwrap()),
    );
    assert!(
        other
            .encode_response(&decoded, &Contract::full())
            .unwrap()
            .get("request_id")
            .is_none()
    );
    assert!(
        adapter(Dialect::OpenBridge)
            .encode_response(&decoded, &Contract::full())
            .unwrap()
            .get("request_id")
            .is_none()
    );
    let mut edited = decoded.clone();
    let items = edited
        .semantic
        .items()
        .iter()
        .filter(|(_, i)| {
            !matches!(
                i,
                openbridge::semantic::task::generation::Item::Reasoning(_)
            )
        })
        .cloned()
        .collect();
    edited.semantic = edited
        .semantic
        .with_items(
            items,
            openbridge::semantic::task::generation::Completion::Stop,
        )
        .unwrap();
    assert!(
        provider
            .encode_response(&edited, &Contract::full())
            .unwrap()
            .get("request_id")
            .is_none()
    );
    value["request_id"] = json!({"not":"a string"});
    assert!(
        provider
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
}
