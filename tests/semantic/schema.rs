//! Schema order is asserted independently of Value equality, which ignores object order.
use crate::wire;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, lower_request, lower_response,
    },
    protocol::openai::{
        Profile, chat, envelope, responses,
        sse::{Obfuscation, ResponsesSseDecoder, ResponsesSseEncoder, SseLimits, encode_frame},
    },
    semantic::{
        task::generation::{OutputConstraint, ToolDefinition},
        value::Presence,
    },
};
use serde_json::{Value, json};

const SCHEMA: &str = r#"{"type":"object","properties":{"zeta":{"type":"object","properties":{"omega":{"type":"string"},"beta":{"type":"integer"}},"required":["omega","beta"],"additionalProperties":false},"middle":{"type":"boolean"},"alpha":{"type":"string"}},"required":["zeta","middle","alpha"],"additionalProperties":false}"#;

fn assert_order(schema: &Value, expected: &[&str]) {
    assert_eq!(
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        expected
    );
}
fn assert_original(schema: &Value) {
    assert_order(schema, &["zeta", "middle", "alpha"]);
    assert_order(&schema["properties"]["zeta"], &["omega", "beta"]);
    assert_eq!(schema.to_string(), SCHEMA);
}
fn request() -> envelope::DecodedResponsesRequest {
    let bytes = format!(
        r#"{{"model":"fixture-model","input":"hello","text":{{"format":{{"type":"json_schema","name":"answer","strict":true,"schema":{SCHEMA}}}}},"tools":[{{"type":"function","name":"f","strict":true,"parameters":{SCHEMA},"output_schema":{SCHEMA}}}]}}"#
    );
    envelope::decode_request_bytes(bytes.as_bytes()).unwrap()
}

#[test]
fn raw_schema_order_survives_ir_and_independent_protocol_projections() {
    let mut d = request();
    let OutputConstraint::JsonSchema { schema, .. } = d.task.semantic.output() else {
        panic!()
    };
    assert_original(schema);
    let ToolDefinition::Function(tool) = &d.task.semantic.tools()[0] else {
        panic!()
    };
    assert_original(tool.parameters.as_ref().unwrap());
    assert_original(tool.output_schema.as_ref().unwrap());
    let encoded = envelope::encode_request(
        &lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Responses,
            Contract::full(),
        )
        .unwrap(),
        &d.context,
    )
    .unwrap();
    assert_original(&encoded["text"]["format"]["schema"]);
    assert_original(&encoded["tools"][0]["parameters"]);
    assert_original(&encoded["tools"][0]["output_schema"]);

    // Keep existing Chat admission: output constraints/output_schema are not mapped.
    assert!(
        lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Chat,
            Contract::full()
        )
        .is_err()
    );
    let mut settings = d.task.semantic.settings().clone();
    settings.text = Default::default();
    let ToolDefinition::Function(tool) = &mut settings.tools.as_mut().unwrap()[0] else {
        panic!()
    };
    tool.output_schema = None;
    d.task.semantic = d.task.semantic.with_settings(settings).unwrap();
    let encoded = chat::encode_generation(
        &lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Chat,
            Contract::full(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(encoded.get("response_format").is_none());
    assert_original(&encoded["tools"][0]["function"]["parameters"]);
}

#[test]
fn final_schema_edits_own_order_and_deleted_constraints_do_not_return() {
    let mut d = request();
    let mut settings = d.task.semantic.settings().clone();
    let Presence::Value(OutputConstraint::JsonSchema { schema, .. }) = &mut settings.text.format
    else {
        panic!()
    };
    let properties = schema["properties"].as_object_mut().unwrap();
    properties.insert("middle".into(), json!({"type":"integer"}));
    properties.insert("new_first".into(), json!({"type":"string"}));
    // Retain surviving properties in their declared order.
    properties.retain(|key, _| key != "zeta");
    schema["required"] = json!(["middle", "alpha", "new_first"]);
    assert_order(schema, &["middle", "alpha", "new_first"]);
    let replacement = schema.clone();
    let ToolDefinition::Function(tool) = &mut settings.tools.as_mut().unwrap()[0] else {
        panic!()
    };
    tool.parameters = Some(replacement.clone());
    tool.output_schema = Some(replacement);
    d.task.semantic = d.task.semantic.with_settings(settings).unwrap();
    let encoded = responses::encode_generation(
        &lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Responses,
            Contract::full(),
        )
        .unwrap(),
    )
    .unwrap();
    for schema in [
        &encoded["text"]["format"]["schema"],
        &encoded["tools"][0]["parameters"],
        &encoded["tools"][0]["output_schema"],
    ] {
        assert_order(schema, &["middle", "alpha", "new_first"]);
        assert_eq!(schema["properties"]["middle"], json!({"type":"integer"}));
        assert!(schema["properties"].get("zeta").is_none());
    }
    let mut settings = d.task.semantic.settings().clone();
    settings.text = Default::default();
    settings.tools = None;
    d.task.semantic = d.task.semantic.with_settings(settings).unwrap();
    let encoded = responses::encode_generation(
        &lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Responses,
            Contract::full(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(encoded.get("text").is_none());
    assert!(encoded.get("tools").is_none());
}

#[test]
fn reported_schema_order_closes_across_static_and_sse() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let mut response = wire::response(2);
    response["text"]["format"]["schema"] = schema.clone();
    response["tools"][0]["parameters"] = schema.clone();
    let d = envelope::decode_response_bytes(&serde_json::to_vec(&response).unwrap()).unwrap();
    let encoded = envelope::encode_response(
        &lower_response(
            &d.semantic,
            &d.fidelity,
            &d.metadata,
            Profile::Responses,
            Contract::full(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_original(&encoded["text"]["format"]["schema"]);
    assert_original(&encoded["tools"][0]["parameters"]);

    let mut decoder =
        ResponsesSseDecoder::new(200, "text/event-stream", SseLimits::default(), None).unwrap();
    let mut events = vec![];
    for mut payload in wire::events(2) {
        if let Some(response) = payload.get_mut("response") {
            response["text"]["format"]["schema"] = schema.clone();
            response["tools"][0]["parameters"] = schema.clone();
        }
        let bytes = encode_frame(&payload, SseLimits::default().max_event_bytes).unwrap();
        for byte in bytes.iter() {
            let (used, next) = decoder.consume(std::slice::from_ref(byte)).unwrap();
            assert_eq!(used, 1);
            events.extend(next);
        }
    }
    decoder.finish().unwrap();
    let d = decoder.materialize().unwrap();
    let OutputConstraint::JsonSchema { schema, .. } =
        d.metadata.context.settings.as_ref().unwrap().output()
    else {
        panic!()
    };
    assert_original(schema);
    let mut encoder = ResponsesSseEncoder::new(
        d.metadata,
        Contract::full(),
        SseLimits::default(),
        Obfuscation::Disabled,
    )
    .unwrap();
    let mut terminal = None;
    for event in &events {
        for frame in encoder.encode(event, &d.fidelity).unwrap() {
            // Inspect serialized output independently of the Responses decoder/materializer.
            let text = std::str::from_utf8(&frame).unwrap();
            let data = text
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap();
            let value: Value = serde_json::from_str(data).unwrap();
            if value["type"] == "response.completed" {
                terminal = Some(value);
            }
        }
    }
    encoder.finish().unwrap();
    let terminal = terminal.unwrap();
    assert_original(&terminal["response"]["text"]["format"]["schema"]);
    assert_original(&terminal["response"]["tools"][0]["parameters"]);
}
