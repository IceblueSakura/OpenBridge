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
fn redundant_text_usage_views_require_exact_totals_and_are_not_replayed() {
    let provider = adapter(Dialect::Bailian);
    let mut value = body();
    value["usage"]["prompt_tokens_details"] = json!({"cached_tokens":0,"text_tokens":4});
    value["usage"]["completion_tokens_details"] = json!({"reasoning_tokens":1,"text_tokens":2});
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    assert_eq!(decoded.semantic.usage().unwrap().reasoning_tokens, Some(1));
    let encoded = provider
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert!(
        encoded["usage"]["completion_tokens_details"]
            .get("text_tokens")
            .is_none()
    );
    assert!(
        adapter(Dialect::OpenBridge)
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
    for invalid in [json!(1), Value::Null, json!("2"), json!(-1)] {
        let mut bad = value.clone();
        bad["usage"]["completion_tokens_details"]["text_tokens"] = invalid;
        assert!(
            provider
                .decode_response(bad.to_string().as_bytes())
                .is_err()
        );
    }
    let chunk = |delta: Value, finish: Value| json!({"id":"c1","object":"chat.completion.chunk","model":"m","created":1,"choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
    for input_count in [4, 3] {
        let mut decoder = provider.event_decoder();
        decoder
            .push(&chunk(
                json!({"role":"assistant","content":"pong","reasoning_content":"think"}),
                Value::Null,
            ))
            .unwrap();
        decoder.push(&chunk(json!({}), json!("stop"))).unwrap();
        let mut tail = json!({"id":"c1","object":"chat.completion.chunk","model":"m","created":1,"choices":[],"usage":value["usage"]});
        tail["usage"]["prompt_tokens_details"]["text_tokens"] = json!(input_count);
        let result = decoder.push(&tail);
        if input_count == 4 {
            result.unwrap();
            decoder.done().unwrap();
            assert_eq!(decoder.materialize().unwrap().semantic, decoded.semantic);
        } else {
            assert!(result.is_err());
            assert!(decoder.done().is_err());
        }
    }
}

#[test]
fn responses_duplicate_usage_details_are_scoped_and_conflicts_poison_snapshots() {
    let provider = Adapter::new(Profile::Responses, Dialect::LongCat, None);
    let mut value = crate::wire::response(2);
    value["usage"]["prompt_tokens_details"] = value["usage"]["input_tokens_details"].clone();
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    let encoded = provider
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert!(encoded["usage"].get("prompt_tokens_details").is_none());
    assert!(
        Adapter::new(Profile::Responses, Dialect::Standard, None)
            .decode_response(value.to_string().as_bytes())
            .is_err()
    );
    for invalid in [json!({"cached_tokens":99}), Value::Null, json!([])] {
        let mut bad = value.clone();
        bad["usage"]["prompt_tokens_details"] = invalid;
        assert!(
            provider
                .decode_response(bad.to_string().as_bytes())
                .is_err()
        );
    }
    for conflict in [false, true] {
        let mut decoder = provider.event_decoder();
        for mut event in crate::wire::events(2) {
            let terminal = event["type"] == "response.completed";
            if terminal {
                event["response"]["usage"]["prompt_tokens_details"] =
                    event["response"]["usage"]["input_tokens_details"].clone();
                if conflict {
                    event["response"]["usage"]["prompt_tokens_details"]["cached_tokens"] =
                        json!(99);
                }
            }
            let result = decoder.push(&event);
            if terminal && conflict {
                assert!(result.is_err());
            } else {
                result.unwrap();
            }
        }
        if conflict {
            assert!(decoder.finish().is_err());
        } else {
            decoder.finish().unwrap();
            assert_eq!(decoder.materialize().unwrap().semantic, decoded.semantic);
        }
    }
}

#[test]
fn queued_creation_waits_for_real_progress_and_projects_existing_lifecycle() {
    use openbridge::protocol::openai::events::EventEncoder;
    use openbridge::semantic::task::generation::StreamEvent;
    let provider = Adapter::new(Profile::Responses, Dialect::Bailian, None);
    let source = crate::wire::events(2);
    let mut queued = source[0].clone();
    queued["response"]["status"] = json!("queued");
    let mut decoder = provider.event_decoder();
    let mut events = decoder.push(&queued).unwrap();
    assert_eq!(events, vec![StreamEvent::Queued]);
    assert!(decoder.finish().is_err());
    let mut progress = source[0].clone();
    progress["type"] = json!("response.in_progress");
    progress["sequence_number"] = json!(1);
    let started = decoder.push(&progress).unwrap();
    assert_eq!(started, vec![StreamEvent::Started]);
    events.extend(started);
    for mut event in source.into_iter().skip(1) {
        event["sequence_number"] = json!(event["sequence_number"].as_u64().unwrap() + 1);
        events.extend(decoder.push(&event).unwrap());
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    let expected = provider
        .decode_response(crate::wire::response(2).to_string().as_bytes())
        .unwrap();
    assert_eq!(decoded.semantic, expected.semantic);
    let mut encoder = EventEncoder::new(Profile::Responses, decoded.metadata.clone()).unwrap();
    let mut wire = vec![];
    for event in events {
        wire.extend(encoder.encode(&event, &decoded.fidelity).unwrap());
    }
    encoder.finish().unwrap();
    assert_eq!(wire[0]["type"], "response.queued");
    assert_eq!(wire[0]["response"]["status"], "queued");
    assert_eq!(wire[1]["type"], "response.created");
    assert_eq!(wire.last().unwrap()["type"], "response.completed");
}

#[test]
fn queued_creation_cannot_mask_missing_progress_or_relax_standard_events() {
    let provider = Adapter::new(Profile::Responses, Dialect::Bailian, None);
    let source = crate::wire::events(2);
    let mut queued = source[0].clone();
    queued["response"]["status"] = json!("queued");
    assert!(
        Adapter::new(Profile::Responses, Dialect::Standard, None)
            .event_decoder()
            .push(&queued)
            .is_err()
    );
    for next in [
        queued.clone(),
        source[0].clone(),
        source[1].clone(),
        source.last().unwrap().clone(),
    ] {
        let mut decoder = provider.event_decoder();
        decoder.push(&queued).unwrap();
        let mut next = next;
        next["sequence_number"] = json!(1);
        assert!(decoder.push(&next).is_err());
        assert!(decoder.finish().is_err());
        assert!(decoder.materialize().is_err());
    }
    let mut missing = queued.clone();
    missing["response"]
        .as_object_mut()
        .unwrap()
        .shift_remove("output");
    assert!(provider.event_decoder().push(&missing).is_err());
    // A plain queued event is not proof of a created event under the alias rule.
    queued["type"] = json!("response.queued");
    let mut decoder = provider.event_decoder();
    decoder.push(&queued).unwrap();
    let mut progress = source[0].clone();
    progress["type"] = json!("response.in_progress");
    progress["sequence_number"] = json!(1);
    assert!(decoder.push(&progress).is_err());
}

fn summary_alias_fixture() -> (Value, Vec<Value>) {
    let item = json!({"id":"r","type":"reasoning","status":null,"content":null,
        "summary":[{"type":"summary_text","text":"brief"}]});
    let mut expected = crate::wire::response(2);
    expected["output"]
        .as_array_mut()
        .unwrap()
        .insert(0, item.clone());
    let base = crate::wire::events(2);
    let mut events = vec![
        base[0].clone(),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"r","type":"reasoning","status":null,"summary":[],"content":null}}),
        json!({"type":"response.reasoning_text.delta","output_index":0,"item_id":"r","content_index":0,"delta":"brief"}),
        json!({"type":"response.reasoning_text.done","output_index":0,"item_id":"r","content_index":0,"text":"brief"}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
    ];
    for mut event in base.into_iter().skip(1) {
        if event.get("output_index").is_some() {
            event["output_index"] = json!(1);
        }
        if event["type"] == "response.completed" {
            event["response"] = expected.clone();
        }
        events.push(event);
    }
    for (n, event) in events.iter_mut().enumerate() {
        event["sequence_number"] = json!(n);
    }
    (expected, events)
}

#[test]
fn reasoning_event_alias_keeps_summary_kind_and_emits_standard_part_lifecycle() {
    use openbridge::protocol::openai::events::EventEncoder;
    use openbridge::semantic::task::generation::{Item, ReasoningContent};
    let provider = Adapter::new(Profile::Responses, Dialect::Bailian, None);
    let (expected, source) = summary_alias_fixture();
    let mut decoder = provider.event_decoder();
    let mut events = vec![];
    for event in &source {
        events.extend(decoder.push(event).unwrap());
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert_eq!(
        decoded.semantic,
        provider
            .decode_response(expected.to_string().as_bytes())
            .unwrap()
            .semantic
    );
    let Item::Reasoning(reasoning) = &decoded.semantic.items()[0].1 else {
        panic!("reasoning owner");
    };
    assert!(
        matches!(&reasoning.parts[0].1,ReasoningContent::Summary(text) if text.as_str()=="brief")
    );
    let mut encoder = EventEncoder::new(Profile::Responses, decoded.metadata.clone()).unwrap();
    let standard = Adapter::new(Profile::Responses, Dialect::Standard, None);
    let mut consumer = standard.event_decoder();
    let mut types = vec![];
    for event in events {
        for wire in encoder.encode(&event, &decoded.fidelity).unwrap() {
            types.push(wire["type"].as_str().unwrap().to_owned());
            consumer.push(&wire).unwrap();
        }
    }
    encoder.finish().unwrap();
    consumer.finish().unwrap();
    assert_eq!(consumer.materialize().unwrap().semantic, decoded.semantic);
    for kind in [
        "response.reasoning_summary_part.added",
        "response.reasoning_summary_text.delta",
        "response.reasoning_summary_text.done",
        "response.reasoning_summary_part.done",
    ] {
        assert!(types.iter().any(|t| t == kind));
    }
    assert!(!types.iter().any(|t| t == "response.reasoning_text.delta"));
    let mut unadapted = standard.event_decoder();
    assert!(source.iter().any(|event| unadapted.push(event).is_err()));
}

#[test]
fn reasoning_event_alias_rejects_incomplete_or_conflicting_summary_snapshots() {
    let provider = Adapter::new(Profile::Responses, Dialect::Bailian, None);
    let (_, source) = summary_alias_fixture();
    let mut missing_done = source.clone();
    missing_done.remove(3);
    let mut wrong_done = source.clone();
    wrong_done[3]["text"] = json!("changed");
    let mut wrong_snapshot = source.clone();
    wrong_snapshot[4]["item"]["summary"][0]["text"] = json!("changed");
    let mut wrong_domain = source.clone();
    wrong_domain[2]["summary_index"] = json!(0);
    for invalid in [missing_done, wrong_done, wrong_snapshot, wrong_domain] {
        let mut decoder = provider.event_decoder();
        assert!(invalid.iter().any(|event| decoder.push(event).is_err()));
        assert!(decoder.finish().is_err());
        assert!(decoder.materialize().is_err());
    }
}

#[test]
fn inactive_response_state_placeholders_do_not_enable_remote_state() {
    let provider = Adapter::new(Profile::Responses, Dialect::Zhipu, None);
    let mut value = crate::wire::response(2);
    value["previous_response_id"] = json!("");
    value["conversation_id"] = json!("");
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    assert_eq!(decoded.semantic.usage().unwrap().total_tokens, 8);
    let encoded = provider
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert_eq!(encoded["previous_response_id"], Value::Null);
    assert!(encoded.get("conversation_id").is_none());
    for dialect in [Dialect::Standard, Dialect::OpenBridge, Dialect::Bailian] {
        assert!(
            Adapter::new(Profile::Responses, dialect, None)
                .decode_response(value.to_string().as_bytes())
                .is_err()
        );
    }
    assert!(
        provider
            .decode_request(br#"{"model":"m","input":"hi","previous_response_id":""}"#)
            .is_err()
    );
    for field in ["previous_response_id", "conversation_id", "conversation"] {
        for invalid in [json!("synthetic-state"), json!({}), json!(0), json!(false)] {
            let mut bad = value.clone();
            bad[field] = invalid;
            assert!(
                provider
                    .decode_response(bad.to_string().as_bytes())
                    .is_err()
            );
        }
    }
    for conflict in [false, true] {
        use openbridge::protocol::openai::sse::{ResponsesSseDecoder, SseLimits};
        let mut decoder = ResponsesSseDecoder::with_decoder(
            200,
            "text/event-stream",
            SseLimits::default(),
            provider.event_decoder(),
        )
        .unwrap();
        for mut event in crate::wire::events(2) {
            let terminal = event["type"] == "response.completed";
            if let Some(snapshot) = event.get_mut("response") {
                snapshot["previous_response_id"] = json!("");
                snapshot["conversation_id"] = if terminal && conflict {
                    json!("synthetic-state")
                } else {
                    Value::Null
                };
            }
            let frame = format!("data: {event}\n\n");
            let result = decoder.consume(frame.as_bytes());
            if terminal && conflict {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().0, frame.len());
            }
        }
        if conflict {
            assert!(decoder.finish().is_err());
            assert!(decoder.materialize().is_err());
        } else {
            decoder.finish().unwrap();
            assert_eq!(decoder.materialize().unwrap().semantic, decoded.semantic);
        }
    }
}

fn billing_view(value: &mut Value) {
    value["billing"] = Value::Null;
    value["frequency_penalty"] = json!(0.25);
    if value.get("usage").is_some_and(Value::is_object) {
        value["usage"]["x_details"] = json!([{
            "x_billing_type":"response_api", "input_tokens":3,"output_tokens":5,"total_tokens":8,
            "prompt_tokens_details":{"cached_tokens":1,"cache_write_tokens":0},
            "output_tokens_details":{"reasoning_tokens":2}
        }]);
    }
}

#[test]
fn response_billing_view_is_checked_not_a_second_usage_authority() {
    let provider = Adapter::new(
        Profile::Responses,
        Dialect::Bailian,
        Some(ReplayOrigin::new("a").unwrap()),
    );
    let mut value = crate::wire::response(2);
    billing_view(&mut value);
    let decoded = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    let usage = decoded.semantic.usage().unwrap();
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.total_tokens,
            usage.reasoning_tokens
        ),
        (3, 5, 8, Some(2))
    );
    let same = provider
        .encode_response(&decoded, &Contract::full())
        .unwrap();
    assert_eq!(same["frequency_penalty"], 0.25);
    assert!(same.get("billing").is_none());
    assert!(same["usage"].get("x_details").is_none());
    for dialect in [Dialect::Standard, Dialect::OpenBridge, Dialect::Xiaomi] {
        let other = Adapter::new(
            Profile::Responses,
            dialect,
            Some(ReplayOrigin::new("a").unwrap()),
        );
        assert!(other.decode_response(value.to_string().as_bytes()).is_err());
        let projection = other.encode_response(&decoded, &Contract::full()).unwrap();
        assert!(projection.get("frequency_penalty").is_none());
        assert!(projection["usage"].get("x_details").is_none());
    }
    let other_scope = Adapter::new(
        Profile::Responses,
        Dialect::Bailian,
        Some(ReplayOrigin::new("b").unwrap()),
    );
    assert!(
        other_scope
            .encode_response(&decoded, &Contract::full())
            .unwrap()
            .get("frequency_penalty")
            .is_none()
    );
    let mut edited = decoded.clone();
    let mut changed = usage;
    changed.input_tokens = 4;
    changed.total_tokens = 9;
    edited.semantic = edited.semantic.with_usage(changed).unwrap();
    let projected = provider
        .encode_response(&edited, &Contract::full())
        .unwrap();
    assert_eq!(projected["usage"]["input_tokens"], 4);
    assert!(projected["usage"].get("x_details").is_none());
    assert!(projected.get("frequency_penalty").is_none());
    edited.semantic = openbridge::semantic::task::generation::GenerationResponse::new(
        edited.semantic.items().to_vec(),
        edited.semantic.completion().unwrap(),
    )
    .unwrap();
    let projected = provider
        .encode_response(&edited, &Contract::full())
        .unwrap();
    assert!(projected["usage"].is_null());
}

#[test]
fn response_billing_view_rejects_independent_or_conflicting_information() {
    let provider = Adapter::new(
        Profile::Responses,
        Dialect::Bailian,
        Some(ReplayOrigin::new("a").unwrap()),
    );
    let mut value = crate::wire::response(2);
    billing_view(&mut value);
    for (pointer, invalid) in [
        ("/billing", json!({"payer":"synthetic"})),
        ("/billing", json!({})),
        ("/usage/x_details", Value::Null),
        ("/usage/x_details", json!([])),
        (
            "/usage/x_details",
            json!([
                value["usage"]["x_details"][0],
                value["usage"]["x_details"][0]
            ]),
        ),
        ("/usage/x_details/0/input_tokens", json!(4)),
        ("/usage/x_details/0/input_tokens", json!(3.0)),
        ("/usage/x_details/0/input_tokens", json!(-1)),
        ("/usage/x_details/0/x_billing_type", json!("tool")),
        (
            "/usage/x_details/0/prompt_tokens_details/cached_tokens",
            json!(2),
        ),
        (
            "/usage/x_details/0/output_tokens_details/reasoning_tokens",
            json!(3),
        ),
    ] {
        let mut bad = value.clone();
        *bad.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            provider
                .decode_response(bad.to_string().as_bytes())
                .is_err(),
            "{pointer}"
        );
    }
    for field in ["image_tokens", "plugins", "unknown"] {
        let mut bad = value.clone();
        bad["usage"]["x_details"][0][field] = json!(0);
        assert!(
            provider
                .decode_response(bad.to_string().as_bytes())
                .is_err()
        );
    }
    for field in ["input_tokens", "output_tokens", "total_tokens"] {
        let mut bad = value.clone();
        bad["usage"]["x_details"][0]
            .as_object_mut()
            .unwrap()
            .shift_remove(field);
        assert!(
            provider
                .decode_response(bad.to_string().as_bytes())
                .is_err()
        );
    }
}

#[test]
fn response_billing_view_events_match_static_and_conflicts_poison_terminal() {
    let provider = Adapter::new(
        Profile::Responses,
        Dialect::Bailian,
        Some(ReplayOrigin::new("a").unwrap()),
    );
    let mut value = crate::wire::response(2);
    billing_view(&mut value);
    let expected = provider
        .decode_response(value.to_string().as_bytes())
        .unwrap();
    for conflict in [false, true] {
        let mut decoder = provider.event_decoder();
        for mut event in crate::wire::events(2) {
            let terminal = event["type"] == "response.completed";
            if let Some(response) = event.get_mut("response") {
                billing_view(response);
                if terminal && conflict {
                    response["usage"]["x_details"][0]["total_tokens"] = json!(99);
                }
            }
            let result = decoder.push(&event);
            if terminal && conflict {
                assert!(result.is_err());
            } else {
                result.unwrap();
            }
        }
        if conflict {
            assert!(decoder.finish().is_err());
            assert!(decoder.materialize().is_err());
        } else {
            decoder.finish().unwrap();
            let actual = decoder.materialize().unwrap();
            assert_eq!(actual.semantic, expected.semantic);
            let projection = provider
                .encode_response(&actual, &Contract::full())
                .unwrap();
            assert_eq!(projection["frequency_penalty"], 0.25);
            assert!(projection["usage"].get("x_details").is_none());
        }
    }
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
