//! Independent field and transformation contracts for stateless Responses text.
use crate::wire;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, lower_request, lower_response,
    },
    protocol::{
        fidelity::FidelityRecords,
        openai::{
            DecodedRequest, Profile, envelope,
            events::{EventDecoder, EventEncoder},
            responses,
            sse::*,
        },
    },
    semantic::{
        task::generation::*,
        value::{Presence, ReplayOrigin, Text},
    },
};
use serde_json::{Value, json};
fn origin() -> ReplayOrigin {
    ReplayOrigin::new("synthetic-loopback").unwrap()
}
fn contract() -> Contract {
    let mut c = Contract::full();
    c.replay_origin = Some(origin());
    c
}
fn text(s: &str) -> Text {
    Text::allowing_empty(s, "synthetic", MAX_TEXT_BYTES).unwrap()
}
fn request_wire(d: &DecodedRequest) -> Value {
    responses::encode_generation(
        &lower_request(&d.semantic, &d.fidelity, Profile::Responses, contract()).unwrap(),
    )
    .unwrap()
}
#[test]
fn raw_envelopes_reject_duplicate_keys_before_task_decode() {
    let request = wire::request(false).to_string();
    assert_eq!(
        envelope::decode_request_bytes(request.as_bytes())
            .unwrap()
            .context
            .model,
        "fixture-model"
    );
    let duplicate = format!("{{\"model\":\"discarded\",{}", &request[1..]);
    assert!(envelope::decode_request_bytes(duplicate.as_bytes()).is_err());
    let nested = request.replace(
        "\"suite\":\"v2-local\"",
        "\"suite\":\"discarded\",\"suite\":\"v2-local\"",
    );
    assert_ne!(nested, request);
    assert!(envelope::decode_request_bytes(nested.as_bytes()).is_err());

    let response = wire::response(2).to_string();
    let d = envelope::decode_response_bytes(response.as_bytes()).unwrap();
    assert_eq!(d.fidelity.response_item_id(ItemId::new(1)), Some("answer"));
    let duplicate = response.replace(
        "\"id\":\"answer\"",
        "\"id\":\"discarded\",\"id\":\"answer\"",
    );
    assert_ne!(duplicate, response);
    assert!(envelope::decode_response_bytes(duplicate.as_bytes()).is_err());
    assert!(envelope::decode_response_bytes(format!("{response} null").as_bytes()).is_err());
    assert!(envelope::decode_request_bytes(format!("{request} null").as_bytes()).is_err());
}

#[test]
fn absent_text_container_cannot_hide_present_children() {
    let d = responses::decode_generation(&json!({"input":"hello"})).unwrap();
    for (format, verbosity) in [
        (
            Presence::Value(OutputConstraint::JsonObject),
            Presence::Absent,
        ),
        (Presence::Value(OutputConstraint::Text), Presence::Absent),
        (Presence::Null, Presence::Absent),
        (Presence::Absent, Presence::Value(Verbosity::Low)),
        (Presence::Absent, Presence::Null),
    ] {
        let mut settings = d.semantic.settings().clone();
        settings.text = TextOptions {
            presence: false,
            format,
            verbosity,
        };
        assert_eq!(settings.validate(), Err(GenerationError::InvalidControl));
        assert!(
            GenerationRequest::from_settings(d.semantic.items().to_vec(), settings.clone())
                .is_err()
        );
        assert!(d.semantic.clone().with_settings(settings.clone()).is_err());
        let mut response = envelope::decode_response(&wire::response(2)).unwrap();
        response.metadata.context.settings.as_mut().unwrap().text = settings.text;
        assert!(
            lower_response(
                &response.semantic,
                &response.fidelity,
                &response.metadata,
                Profile::Responses,
                contract()
            )
            .is_err()
        );
    }
}

#[test]
fn text_presence_and_final_requirements_match_independent_wire_expectations() {
    let mut d = responses::decode_generation(&json!({"input":"hello"})).unwrap();
    for (options, expected, structured) in [
        (
            TextOptions {
                presence: true,
                ..Default::default()
            },
            Some(json!({})),
            false,
        ),
        (
            TextOptions {
                presence: true,
                format: Presence::Null,
                verbosity: Presence::Null,
            },
            Some(json!({"format":null,"verbosity":null})),
            false,
        ),
        (
            TextOptions {
                presence: true,
                format: Presence::Value(OutputConstraint::Text),
                verbosity: Presence::Value(Verbosity::Medium),
            },
            Some(json!({"format":{"type":"text"},"verbosity":"medium"})),
            false,
        ),
        (
            TextOptions {
                presence: true,
                format: Presence::Value(OutputConstraint::JsonObject),
                verbosity: Presence::Absent,
            },
            Some(json!({"format":{"type":"json_object"}})),
            true,
        ),
        (TextOptions::default(), None, false),
    ] {
        let mut source = json!({"input":"hello"});
        if let Some(expected) = &expected {
            source["text"] = expected.clone();
        }
        assert_eq!(
            responses::decode_generation(&source)
                .unwrap()
                .semantic
                .text_options(),
            &options
        );
        let mut settings = d.semantic.settings().clone();
        settings.text = options;
        d.semantic = d.semantic.with_settings(settings).unwrap();
        assert_eq!(
            GenerationRequirements::derive(&d.semantic).structured_output,
            structured
        );
        assert_eq!(request_wire(&d).get("text"), expected.as_ref());
        let mut limited = contract();
        limited.structured_output = false;
        assert_eq!(
            lower_request(&d.semantic, &d.fidelity, Profile::Responses, limited).is_err(),
            structured
        );
    }
}

#[test]
fn complete_messages_require_headers_but_task_snapshots_keep_abbreviations() {
    let source = wire::response(2);
    for key in ["type", "id", "status", "role", "content"] {
        for replacement in [None, Some(Value::Null), Some(json!(false))] {
            let mut bad = source.clone();
            let item = bad["output"][0].as_object_mut().unwrap();
            if let Some(value) = replacement {
                item.insert(key.into(), value);
            } else {
                item.remove(key);
            }
            assert!(envelope::decode_response(&bad).is_err(), "{key}");
        }
    }
    for (key, value) in [("id", ""), ("status", "unknown"), ("role", "user")] {
        let mut bad = source.clone();
        bad["output"][0][key] = json!(value);
        assert!(envelope::decode_response(&bad).is_err(), "{key}");
    }
    for (status, lifecycle) in [
        ("completed", ItemLifecycle::Completed),
        ("incomplete", ItemLifecycle::Incomplete),
        ("in_progress", ItemLifecycle::InProgress),
    ] {
        let mut value = source.clone();
        value["status"] = json!("incomplete");
        value["incomplete_details"] = json!({"reason":"max_output_tokens"});
        value["output"][0]["status"] = json!(status);
        let d = envelope::decode_response(&value).unwrap();
        assert_eq!(d.semantic.items()[0].1.lifecycle(), Some(lifecycle));
        let encoded = envelope::encode_response(
            &lower_response(
                &d.semantic,
                &d.fidelity,
                &d.metadata,
                Profile::Responses,
                contract(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(encoded["output"][0]["id"], "answer");
        assert_eq!(encoded["output"][0]["status"], status);
    }
    let mut abbreviated = source;
    for key in ["id", "status"] {
        abbreviated["output"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    let decoded = responses::decode_response(&abbreviated).unwrap();
    let encoded = envelope::encode_response(
        &lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(encoded["output"][0]["id"], "item_1");
    assert_eq!(encoded["output"][0]["status"], "completed");
    let input = responses::decode_generation(&json!({"input":[{"role":"user","content":"hello"}]}))
        .unwrap();
    assert_eq!(
        request_wire(&input)["input"],
        json!([{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}])
    );
}

#[test]
fn instruction_status_is_checked_in_history_and_reported_echoes() {
    for role in ["system", "developer"] {
        for status in [
            None,
            Some(json!("completed")),
            Some(Value::Null),
            Some(json!(false)),
            Some(json!("unknown")),
            Some(json!("in_progress")),
            Some(json!("incomplete")),
        ] {
            let accepted = status.is_none() || status == Some(json!("completed"));
            let mut item = json!({"type":"message","role":role,"content":[{"type":"input_text","text":"Be precise"}]});
            if let Some(status) = status {
                item["status"] = status;
            }
            let decoded = responses::decode_generation(&json!({"input":[item.clone()]}));
            assert_eq!(decoded.is_ok(), accepted, "{item}");
            if let Ok(d) = decoded {
                assert_eq!(
                    request_wire(&d)["input"],
                    json!([{"role":role,"content":"Be precise"}])
                );
            }
            let mut response = wire::response(2);
            response["instructions"] = json!([item]);
            let decoded = envelope::decode_response(&response);
            assert_eq!(decoded.is_ok(), accepted);
            if let Ok(d) = decoded {
                let output = envelope::encode_response(
                    &lower_response(
                        &d.semantic,
                        &d.fidelity,
                        &d.metadata,
                        Profile::Responses,
                        contract(),
                    )
                    .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    output["instructions"],
                    json!([{"role":role,"content":"Be precise"}])
                );
            }
        }
    }
}

#[test]
fn annotation_payload_rejection_cannot_be_followed_by_success() {
    for annotation in [
        None,
        Some(Value::Null),
        Some(json!({})),
        Some(json!("invalid")),
    ] {
        let values = wire::events(2);
        let index = values
            .iter()
            .position(|v| v["type"] == "response.output_text.annotation.added")
            .unwrap();
        let mut decoder = EventDecoder::new(Profile::Responses);
        for value in &values[..index] {
            decoder.push(value).unwrap();
        }
        let mut bad = values[index].clone();
        if let Some(annotation) = annotation {
            bad["annotation"] = annotation;
        } else {
            bad.as_object_mut().unwrap().remove("annotation");
        }
        assert!(decoder.push(&bad).is_err(), "{bad}");
        for value in &values[index..] {
            assert!(decoder.push(value).is_err());
        }
        assert!(decoder.finish().is_err());
        assert!(decoder.materialize().is_err());
    }
}

#[test]
fn string_input_and_top_level_instructions_are_task_semantics() {
    let mut d = responses::decode_generation(&json!({"input":"hello","instructions":"Be precise"}))
        .unwrap();
    assert_eq!(
        d.semantic.instructions().value().unwrap().as_str(),
        "Be precise"
    );
    assert_eq!(d.semantic.items().len(), 1);
    let mut settings = d.semantic.settings().clone();
    settings.instructions = Presence::Value(text("New instruction"));
    d.semantic = d.semantic.with_settings(settings).unwrap();
    assert_eq!(request_wire(&d)["instructions"], "New instruction");
    let mut settings = d.semantic.settings().clone();
    settings.instructions = Presence::Absent;
    d.semantic = d.semantic.with_settings(settings).unwrap();
    assert!(request_wire(&d).get("instructions").is_none());
    let d = responses::decode_generation(&json!({"instructions":"Only instructions"})).unwrap();
    assert!(d.semantic.items().is_empty());
}
#[test]
fn custom_tools_and_text_array_results_are_not_function_arguments() {
    let mut d=responses::decode_generation(&json!({"input":[{"type":"custom_tool_call","call_id":"c","name":"sql","input":"SELECT 1"},{"type":"custom_tool_call_output","call_id":"c","output":[{"type":"input_text","text":"1"},{"type":"input_text","text":""}]}],"tools":[{"type":"custom","name":"sql","format":{"type":"grammar","syntax":"regex","definition":"SELECT [0-9]+"}}],"tool_choice":{"type":"custom","name":"sql"}})).unwrap();
    assert!(matches!(&d.semantic.items()[0].1,Item::CustomCall(c) if c.input=="SELECT 1"));
    assert!(GenerationRequirements::derive(&d.semantic).custom_tools);
    let mut items = d.semantic.items().to_vec();
    let Item::CustomCall(c) = &mut items[0].1 else {
        panic!()
    };
    c.input = "not JSON, not executed".into();
    let Item::CustomResult(r) = &mut items[1].1 else {
        panic!()
    };
    r.output = ToolOutput::Text("changed".into());
    d.semantic = d.semantic.with_items(items).unwrap();
    let out = request_wire(&d);
    assert_eq!(out["input"][0]["input"], "not JSON, not executed");
    assert!(out["input"][0].get("arguments").is_none());
    assert_eq!(out["input"][1]["output"], "changed");
    assert!(lower_request(&d.semantic, &d.fidelity, Profile::Chat, contract()).is_err());
    let mut mismatch = out;
    mismatch["input"][1]["type"] = json!("function_call_output");
    assert!(responses::decode_generation(&mismatch).is_err());
}
#[test]
fn annotated_output_and_probabilities_are_owned_by_the_current_text() {
    let original = wire::response(2);
    let mut d = envelope::decode_response(&original).unwrap();
    let mut items = d.semantic.items().to_vec();
    let Item::Message(m) = &mut items[0].1 else {
        panic!()
    };
    let ContentPart::Text(t) = &m.parts[0].content else {
        panic!()
    };
    assert_eq!(t.annotations().len(), 1);
    assert_eq!(
        t.logprobs().value().unwrap()[0].bytes.as_ref().unwrap(),
        b"{\"ok\":false}"
    );
    m.parts[0].content = ContentPart::Text(t.clone().replace_text(text("{\"ok\":true}")));
    let completion = d.semantic.completion().unwrap();
    d.semantic = d.semantic.with_items(items, completion).unwrap();
    let out = envelope::encode_response(
        &lower_response(
            &d.semantic,
            &d.fidelity,
            &d.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        out["output"][0]["content"][0],
        json!({"type":"output_text","text":"{\"ok\":true}","annotations":[]})
    );
    assert_eq!(out["created_at"], json!(1.25));
    assert_eq!(out["completed_at"], json!(2.5));
    assert!(envelope::decode_response(&json!({"id":"r","object":"response","created_at":1,"model":"m","status":"completed","output":[]})).is_err());
}
#[test]
fn structured_output_and_reasoning_context_are_real_controls() {
    let mut d=responses::decode_generation(&json!({"input":[{"role":"user","content":"hello"}],"top_p":0.8,"top_logprobs":2,"text":{"format":{"type":"json_schema","name":"answer","schema":{"type":"object","properties":{},"required":[],"additionalProperties":false},"strict":true},"verbosity":"low"},"reasoning":{"effort":"max","context":"all_turns","mode":"pro"}})).unwrap();
    assert!(matches!(
        d.semantic.output(),
        OutputConstraint::JsonSchema {
            strict: Some(true),
            ..
        }
    ));
    assert_eq!(
        d.semantic.reasoning().context,
        Presence::Value(ReasoningContext::AllTurns)
    );
    assert!(GenerationRequirements::derive(&d.semantic).structured_output);
    let mut s = d.semantic.settings().clone();
    s.text.format = Presence::Value(OutputConstraint::JsonObject);
    s.text.verbosity = Presence::Value(Verbosity::High);
    s.reasoning = ReasoningRequest::absent();
    s.controls = GenerationControls::default();
    d.semantic = d.semantic.with_settings(s).unwrap();
    let out = request_wire(&d);
    assert_eq!(
        out["text"],
        json!({"format":{"type":"json_object"},"verbosity":"high"})
    );
    assert!(out.get("reasoning").is_none());
    assert!(out.get("top_p").is_none());
    assert!(out.get("top_logprobs").is_none());
}
#[test]
fn complete_envelope_separates_model_delivery_execution_and_task() {
    let mut request = wire::request(true);
    request["stream_options"] = json!({"include_obfuscation":false});
    let d = envelope::decode_request(&request).unwrap();
    assert_eq!(d.context.model, "fixture-model");
    assert!(d.context.delivery.streaming());
    assert_eq!(
        d.context.execution.metadata.value().unwrap()["suite"],
        "v2-local"
    );
    let out = envelope::encode_request(
        &lower_request(
            &d.task.semantic,
            &d.task.fidelity,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
        &d.context,
    )
    .unwrap();
    assert_eq!(out["stream_options"], request["stream_options"]);
    assert_eq!(out["model"], "fixture-model");
    assert_eq!(out["store"], false);
    assert_eq!(out["text"], request["text"]);
    assert_eq!(out["input"][0]["content"][0]["text"], "hello 🧪");
    let mut no_store = request.clone();
    no_store.as_object_mut().unwrap().remove("store");
    let d = envelope::decode_request(&no_store).unwrap();
    assert_eq!(
        envelope::encode_request(
            &lower_request(
                &d.task.semantic,
                &d.task.fidelity,
                Profile::Responses,
                contract()
            )
            .unwrap(),
            &d.context
        )
        .unwrap()["store"],
        false
    );
    for (key, value) in [
        ("store", json!(true)),
        ("background", json!(true)),
        ("previous_response_id", json!("remote-state")),
        ("conversation", json!({"id":"remote"})),
        ("prompt", json!({"id":"template"})),
        ("context_management", json!([{"type":"compaction"}])),
        ("base_url", json!("https://example.test")),
    ] {
        let mut bad = request.clone();
        bad[key] = value;
        assert!(envelope::decode_request(&bad).is_err(), "{key}");
    }
}
#[test]
fn instruction_parts_and_cache_breakpoints_follow_surviving_owners() {
    let mut d=responses::decode_generation(&json!({"input":[{"type":"message","id":"instruction","role":"developer","content":[{"type":"input_text","text":"a","prompt_cache_breakpoint":{"mode":"explicit"}},{"type":"input_text","text":"b"}]},{"role":"assistant","content":[{"type":"input_text","text":"prefix","prompt_cache_breakpoint":{"mode":"explicit"}}]}]})).unwrap();
    let out = request_wire(&d);
    assert_eq!(
        out["input"][0]["content"][0]["prompt_cache_breakpoint"],
        json!({"mode":"explicit"})
    );
    assert_eq!(out["input"][1]["content"][0]["type"], "input_text");
    let mut items = d.semantic.items().to_vec();
    let Item::Instruction(i) = &mut items[0].1 else {
        panic!()
    };
    i.parts.remove(0);
    items.pop();
    d.semantic = d.semantic.with_items(items).unwrap();
    let out = request_wire(&d);
    assert_eq!(out["input"][0]["content"], "b");
    assert!(!out.to_string().contains("prompt_cache_breakpoint"));
    let mut response = wire::response(2);
    response["instructions"] = json!([{"type":"message","id":"instruction","role":"developer","content":[{"type":"input_text","text":"a"},{"type":"input_text","text":"b"}]}]);
    let d = envelope::decode_response(&response).unwrap();
    let encoded = envelope::encode_response(
        &lower_response(
            &d.semantic,
            &d.fidelity,
            &d.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(encoded["instructions"], response["instructions"]);
}
#[test]
fn null_empty_false_and_default_controls_are_deliberate_not_unknown_passthrough() {
    let d=responses::decode_generation(&json!({"input":"","instructions":null,"reasoning":{"effort":null,"summary":null},"tools":[],"tool_choice":"none","parallel_tool_calls":false,"text":{"format":{"type":"text"},"verbosity":null}})).unwrap();
    let out = request_wire(&d);
    assert_eq!(out["instructions"], Value::Null);
    assert_eq!(out["reasoning"], json!({"effort":null,"summary":null}));
    assert_eq!(out["parallel_tool_calls"], false);
    assert_eq!(out["tools"], json!([]));
    for bad in [
        json!({"top_logprobs":21}),
        json!({"top_p":1.1}),
        json!({"temperature":-1}),
        json!({"text":{"format":{"type":"json_schema","name":"bad name","schema":{}}}}),
        json!({"tools":[{"type":"custom","name":"x","format":{"type":"grammar","syntax":"unknown","definition":"x"}}]}),
        json!({"tools":[{"type":"function","name":"x","async":true}]}),
        json!({"reasoning":{"unknown":true}}),
    ] {
        let mut value = json!({"input":"hello"});
        value
            .as_object_mut()
            .unwrap()
            .extend(bad.as_object().unwrap().clone());
        assert!(responses::decode_generation(&value).is_err());
    }
}
#[test]
fn sdk_parsed_arguments_is_a_checked_derived_view_not_a_second_owner() {
    let source = json!({"input":[{"type":"function_call","id":"f","call_id":"c","name":"lookup","arguments":"{\"n\":1}","parsed_arguments":{"n":1}}]});
    let decoded = responses::decode_generation(&source).unwrap();
    let encoded = request_wire(&decoded);
    assert_eq!(encoded["input"][0]["arguments"], "{\"n\":1}");
    assert!(encoded["input"][0].get("parsed_arguments").is_none());
    let mut mismatched = source;
    mismatched["input"][0]["parsed_arguments"] = json!({"n":2});
    assert!(responses::decode_generation(&mismatched).is_err());
}

#[test]
fn static_file_path_is_not_an_annotation_added_event() {
    let mut response = wire::response(2);
    response["output"][0]["content"][0]["annotations"] =
        json!([{"type":"file_path","file_id":"synthetic-file","index":0}]);
    envelope::decode_response(&response).unwrap();
    let mut raw = wire::events(2);
    let added = raw
        .iter_mut()
        .find(|v| v["type"] == "response.output_text.annotation.added")
        .unwrap();
    added["annotation"] = json!({"type":"file_path","file_id":"synthetic-file","index":0});
    let mut decoder = EventDecoder::new(Profile::Responses);
    assert!(raw.iter().any(|v| decoder.push(v).is_err()));
    assert!(decoder.finish().is_err());

    let mut accepted = EventDecoder::new(Profile::Responses).with_replay_origin(origin());
    let mut events = vec![];
    for value in wire::events(2) {
        events.extend(accepted.push(&value).unwrap());
    }
    accepted.finish().unwrap();
    let mut encoder = EventEncoder::new(Profile::Responses, accepted.metadata().unwrap().clone())
        .unwrap()
        .with_contract(contract());
    let mut rejected = false;
    for mut event in events {
        if let StreamEvent::AnnotationAdded { annotation, .. } = &mut event {
            *annotation = Annotation::FilePath {
                file_id: "synthetic-file".into(),
                index: 0,
            };
        }
        if encoder.encode(&event, accepted.fidelity()).is_err() {
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert!(encoder.finish().is_err());
}

#[test]
fn reported_cache_write_usage_is_typed_and_never_invented_for_chat() {
    let mut source = wire::response(2);
    source["usage"]["input_tokens_details"]["cache_write_tokens"] = json!(2);
    let decoded = envelope::decode_response(&source).unwrap();
    assert_eq!(
        decoded.semantic.usage().unwrap().input_cache_write_tokens,
        Some(2)
    );
    assert!(
        lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Chat,
            contract()
        )
        .is_err()
    );
    let output = envelope::encode_response(
        &lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(output["usage"], source["usage"]);
    source["usage"]["input_tokens_details"]
        .as_object_mut()
        .unwrap()
        .remove("cache_write_tokens");
    assert!(envelope::decode_response(&source).is_err());
}

#[test]
fn custom_annotations_and_logprobs_close_independent_sse_and_static_snapshots() {
    let mut decoder = EventDecoder::new(Profile::Responses).with_replay_origin(origin());
    let mut events = vec![];
    for v in wire::events(1) {
        events.extend(decoder.push(&v).unwrap());
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        StreamEvent::ItemStarted {
            kind: ItemKind::CustomCall { .. },
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::AnnotationAdded { .. }))
    );
    let rendered = envelope::encode_response(
        &lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(rendered["output"], wire::response(1)["output"]);
    assert_eq!(rendered["usage"], wire::response(1)["usage"]);
    let mut encoder = ResponsesSseEncoder::new(
        decoded.metadata,
        contract(),
        SseLimits::default(),
        Obfuscation::Seeded([7; 32]),
    )
    .unwrap();
    let mut bytes = vec![];
    for event in events {
        for frame in encoder.encode(&event, &decoded.fidelity).unwrap() {
            bytes.extend(frame);
        }
    }
    encoder.finish().unwrap();
    let mut round = ResponsesSseDecoder::new(
        200,
        "text/event-stream; charset=\"utf-8\"",
        SseLimits::default(),
        Some(origin()),
    )
    .unwrap();
    let mut seen = 0;
    for b in &bytes {
        let (used, events) = round.consume(std::slice::from_ref(b)).unwrap();
        assert_eq!(used, 1);
        seen += events.len();
    }
    round.finish().unwrap();
    assert!(seen > 10);
    let actual = round.materialize().unwrap();
    let out = envelope::encode_response(
        &lower_response(
            &actual.semantic,
            &actual.fidelity,
            &actual.metadata,
            Profile::Responses,
            contract(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(out["output"], wire::response(1)["output"]);
}
#[test]
fn probabilities_annotations_and_snapshots_cannot_rewrite_observed_values() {
    for field in ["probability", "annotation", "terminal"] {
        let mut values = wire::events(1);
        match field {
            "probability" => {
                let v = values
                    .iter_mut()
                    .find(|v| v["type"] == "response.content_part.done")
                    .unwrap();
                v["part"]["logprobs"][0]["logprob"] = json!(-0.75);
            }
            "annotation" => {
                let v = values
                    .iter_mut()
                    .find(|v| v["type"] == "response.output_text.annotation.added")
                    .unwrap();
                v["annotation_index"] = json!(1);
            }
            _ => {
                values.last_mut().unwrap()["response"]["output"][3]["content"][0]["text"] =
                    json!("resurrected");
            }
        }
        let mut d = EventDecoder::new(Profile::Responses).with_replay_origin(origin());
        assert!(values.iter().any(|v| d.push(v).is_err()), "{field}");
        assert!(d.finish().is_err());
    }
}
#[test]
fn absent_error_code_and_unspecified_incomplete_reason_are_not_success() {
    let mut d = EventDecoder::new(Profile::Responses);
    let events = d
        .push(&json!({"type":"error","message":"synthetic","code":null,"param":null}))
        .unwrap();
    assert!(
        matches!(&events[0],StreamEvent::Terminal{terminal:StreamTerminal::Error,details} if details.error.as_ref().unwrap().code.is_none())
    );
    d.finish().unwrap();
    assert!(d.materialize().is_err());
    let mut value = wire::response(2);
    value["status"] = json!("incomplete");
    value["incomplete_details"] = json!({"reason":null});
    let d = envelope::decode_response(&value).unwrap();
    assert_eq!(
        d.semantic.details().incomplete,
        Some(IncompleteReason::Unspecified)
    );
    assert!(
        lower_response(
            &d.semantic,
            &d.fidelity,
            &d.metadata,
            Profile::Chat,
            contract()
        )
        .is_err()
    );
}
#[test]
fn metadata_budget_and_ranges_fail_without_expanding_or_reusing_old_values() {
    let ann = Annotation::UrlCitation {
        start_index: 0,
        end_index: 2,
        title: "source".into(),
        url: "https://example.test".into(),
    };
    assert!(TextContent::new(text("x"), vec![ann], Presence::Absent).is_err());
    let mut value = wire::response(2);
    value["output"][0]["content"][0]["logprobs"][0]["bytes"] = json!([256]);
    assert!(envelope::decode_response(&value).is_err());
    let mut metadata = wire::request(false);
    metadata["metadata"] = json!({"k":"a".repeat(513)});
    assert!(envelope::decode_request(&metadata).is_err());
    let mut hints = FidelityRecords::default();
    hints.record_cache_breakpoint(PartId::new(99)).unwrap();
    let d = responses::decode_generation(&json!({"input":"hello"})).unwrap();
    assert!(lower_request(&d.semantic, &hints, Profile::Chat, contract()).is_ok());
}
