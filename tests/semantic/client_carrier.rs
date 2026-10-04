//! Independent client wire expectations; extensions never select credentials or origins.
use crate::events_support::text;
use openbridge::{
    adapter::{Adapter, Dialect},
    protocol::openai::Profile,
    semantic::{task::generation::*, value::ReplayOrigin},
};
use serde_json::{Value, json};
fn client() -> Adapter {
    Adapter::new(
        Profile::Responses,
        Dialect::OpenBridge,
        Some(ReplayOrigin::new("synthetic-client").unwrap()),
    )
}
#[path = "../support/client_carrier.rs"]
mod client_wire;
fn fixture() -> Value {
    client_wire::response()
}
#[test]
fn client_json_decodes_typed_values_and_reference_owners_after_inserted_input() {
    let response = client()
        .decode_response(&serde_json::to_vec(&fixture()).unwrap())
        .unwrap();
    assert_eq!(
        response.semantic.progress(),
        InteractionProgress::AwaitingToolResults
    );
    assert!(
        matches!(&response.semantic.items()[0].1,Item::Reasoning(r) if r.replay.as_ref().unwrap().format()==ReplayFormat::GoogleInteractionsV1Thought)
    );
    assert!(
        matches!(&response.semantic.items()[2].1,Item::ToolCall(c) if c.arguments.as_structured().unwrap().value()["n"].as_number().and_then(serde_json::Number::as_u128)==Some(18446744073709551616001) && c.message==Some(ItemId::new(2)))
    );
    let mut history = fixture()["output"].as_array().unwrap().clone();
    history.insert(0, json!({"role":"user","content":"initial"}));
    history.extend([
      json!({"type":"function_call_output","call_id":"c1","output":"{\"ok\":true}","_openbridge":{"version":1,"output":"json","execution":{"status":"succeeded"}}}),
      json!({"type":"function_call_output","call_id":"c2","output":"{\"detail\":2}","_openbridge":{"version":1,"output":"json","execution":{"status":"failed","code":"synthetic"}}}),
    ]);
    let request = client()
        .decode_request(
            &serde_json::to_vec(&json!({"model":"fixture-model","input":history})).unwrap(),
        )
        .unwrap();
    assert!(
        matches!(&request.task.semantic.items()[3].1,Item::ToolCall(c) if c.message==Some(ItemId::new(3)))
    );
    assert!(
        matches!(&request.task.semantic.items()[6].1,Item::ToolResult(r) if matches!(r.output,ToolOutput::Structured(_)) && matches!(r.execution,Some(ToolExecution::Failed {..})))
    );
    assert_eq!(
        request.task.semantic.continuation(),
        Continuation::Unreported
    );
    let mut contract = openbridge::lowering::generation::GenerationRepresentationContract::full();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    let encoded = client()
        .encode_request(&request, "fixture-model", &contract)
        .unwrap();
    assert_eq!(encoded["input"][3]["_openbridge"]["message"], "owner");
    assert_eq!(
        encoded["input"][6]["_openbridge"]["execution"],
        json!({"status":"failed","code":"synthetic"})
    );
    assert_eq!(
        client()
            .decode_request(&serde_json::to_vec(&encoded).unwrap())
            .unwrap()
            .task
            .semantic,
        request.task.semantic
    );
}
#[test]
fn final_ir_not_carrier_records_governs_reencoding_and_standard_profiles_stay_closed() {
    let mut response = client()
        .decode_response(&serde_json::to_vec(&fixture()).unwrap())
        .unwrap();
    let mut contract = openbridge::lowering::generation::GenerationRepresentationContract::full();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    let encoded = client().encode_response(&response, &contract).unwrap();
    assert_eq!(encoded["output"], fixture()["output"]);
    assert_eq!(encoded["_openbridge"], fixture()["_openbridge"]);
    let mut items = response.semantic.items().to_vec();
    let Item::Reasoning(reasoning) = &mut items[0].1 else {
        unreachable!()
    };
    reasoning.replay = None;
    let Item::ToolCall(call) = &mut items[2].1 else {
        unreachable!()
    };
    call.arguments = ToolArguments::Structured(StructuredValue::new(json!({"n":7})).unwrap());
    response.semantic = response.semantic.with_items(items).unwrap();
    let changed = client().encode_response(&response, &contract).unwrap();
    assert!(changed["output"][0].get("_openbridge").is_none());
    assert_eq!(changed["output"][2]["arguments"], "{\"n\":7}");
    let standard = Adapter::new(Profile::Responses, Dialect::Standard, None);
    assert!(
        standard
            .decode_response(&serde_json::to_vec(&fixture()).unwrap())
            .is_err()
    );
    assert!(standard.encode_response(&response, &contract).is_err());
}
#[test]
fn client_events_publish_typed_attachment_before_deltas_and_materialize_the_same_response() {
    use crate::events_support as event;
    let mut metadata = event::metadata();
    metadata.id = "response-client".into();
    let mut contract = event::contract();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    let mut encoder =
        openbridge::protocol::openai::events::EventEncoder::new(Profile::Responses, metadata)
            .unwrap()
            .with_contract(client().contract(&contract));
    let source = Default::default();
    let mut payloads = vec![];
    let input = vec![
        StreamEvent::Started,
        StreamEvent::ItemStarted {
            item: ItemId::new(1),
            kind: ItemKind::Reasoning,
            replay: Some(ReasoningReplay {
                origin: Some(ReplayOrigin::new("synthetic-client").unwrap()),
                value: ReplayValue::final_value(
                    ReplayFormat::GoogleInteractionsV1Thought,
                    text("opaque"),
                ),
            }),
        },
        StreamEvent::ItemFinished {
            item: ItemId::new(1),
            status: ItemLifecycle::Completed,
            replay: Some(ReasoningReplay {
                origin: Some(ReplayOrigin::new("synthetic-client").unwrap()),
                value: ReplayValue::final_value(
                    ReplayFormat::GoogleInteractionsV1Thought,
                    text("opaque"),
                ),
            }),
        },
        event::start(2, ItemKind::Message { phase: None }),
        event::close(2, ItemLifecycle::Completed),
        event::start(
            3,
            ItemKind::ToolCall {
                call_id: text("c"),
                name: text("lookup"),
                message: Some(ItemId::new(2)),
                context: Default::default(),
                format: ArgumentFormat::Json,
            },
        ),
        StreamEvent::PartStarted {
            item: ItemId::new(3),
            part: PartId::new(4),
            kind: PartKind::StructuredArguments,
        },
        StreamEvent::Delta {
            item: ItemId::new(3),
            part: PartId::new(4),
            fragment: " { \"n\" : 18446744073709551616001 } ".into(),
            logprobs: vec![],
        },
        StreamEvent::ValueFinished {
            item: ItemId::new(3),
            part: PartId::new(4),
        },
        StreamEvent::PartFinished {
            item: ItemId::new(3),
            part: PartId::new(4),
        },
        event::close(3, ItemLifecycle::Completed),
        StreamEvent::Progress(InteractionProgress::AwaitingToolResults),
        event::terminal(StreamTerminal::Completed),
    ];
    for value in &input {
        payloads.extend(
            encoder
                .encode(value, &source)
                .unwrap_or_else(|error| panic!("synthetic event {value:?}: {error:?}")),
        );
    }
    let opening = payloads
        .iter()
        .find(|value| value.pointer("/item/type").and_then(Value::as_str) == Some("function_call"))
        .unwrap();
    assert_eq!(opening["item"]["_openbridge"]["arguments"], "json_partial");
    let mut decoder = client().event_decoder();
    for value in &payloads {
        decoder
            .push(value)
            .unwrap_or_else(|error| panic!("synthetic payload {value}: {error:?}"));
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert_eq!(
        decoded.semantic,
        event::apply(&input)
            .and_then(|state| materialize(&state))
            .unwrap()
    );
    let mut broken = payloads.clone();
    broken.last_mut().unwrap()["response"]["output"][2]["_openbridge"]["arguments"] =
        json!("json_partial");
    let mut decoder = client().event_decoder();
    assert!(broken.iter().any(|value| decoder.push(value).is_err()));
    assert!(decoder.finish().is_err());
}

#[test]
fn independent_client_stream_fixture_agrees_with_static_authorities() {
    let mut decoder = client().event_decoder();
    for value in client_wire::events() {
        decoder.push(&value).unwrap();
    }
    assert_eq!(
        decoder.materialize().unwrap().semantic,
        client()
            .decode_response(&serde_json::to_vec(&fixture()).unwrap())
            .unwrap()
            .semantic
    );
}

#[test]
fn scoped_usage_owns_one_payload_and_refs_follow_wire_ids() {
    let mut value = fixture();
    value["usage"] = Value::Null;
    let reports = json!([
      {"scope":{"item":"call-one"},"basis":"final","output_relation":"includes_reasoning","total_relation":"input_and_output","input_tokens":2,"output_tokens":3,"total_tokens":5},
      {"scope":"operation","basis":"cumulative","output_relation":"excludes_reasoning","total_relation":"input_output_and_reasoning","input_tokens":5,"output_tokens":3,"reasoning_tokens":2,"total_tokens":10}
    ]);
    value["_openbridge"]["usage"] = reports.clone();
    let decoded = client()
        .decode_response(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    assert_eq!(
        decoded.semantic.usage_reports()[0].scope,
        UsageScope::Item(ItemId::new(3))
    );
    assert_eq!(
        decoded.semantic.usage_reports()[1].basis,
        UsageBasis::Cumulative
    );
    assert!(decoded.semantic.usage().is_none());
    let mut contract = openbridge::lowering::generation::GenerationRepresentationContract::full();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    let encoded = client().encode_response(&decoded, &contract).unwrap();
    assert!(encoded["usage"].is_null());
    assert_eq!(encoded["_openbridge"]["usage"], reports);
    let mut conflict = value.clone();
    conflict["usage"] = fixture()["usage"].clone();
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&conflict).unwrap())
            .is_err()
    );
    let mut dangling = value;
    dangling["_openbridge"]["usage"][0]["scope"]["item"] = json!("absent");
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&dangling).unwrap())
            .is_err()
    );
    let items = decoded
        .semantic
        .items()
        .iter()
        .filter(|(id, _)| *id != ItemId::new(3))
        .cloned()
        .collect();
    assert!(decoded.semantic.clone().with_items(items).is_err());
}

#[test]
fn client_complete_operation_projection_never_panics_or_loses_unknown_and_prediction_counts() {
    let mut decoded = client()
        .decode_response(&serde_json::to_vec(&fixture()).unwrap())
        .unwrap();
    let mut report = Usage::operation(5, 3, 8);
    report.accepted_prediction_tokens = Some(1);
    let mut contract = openbridge::lowering::generation::GenerationRepresentationContract::full();
    contract.replay_origin = Some(ReplayOrigin::new("synthetic-client").unwrap());
    for report in [
        report,
        Usage {
            output_tokens: None,
            total_tokens: None,
            ..report
        },
    ] {
        decoded.semantic = decoded.semantic.clone().with_usage(report).unwrap();
        let value = client().encode_response(&decoded, &contract).unwrap();
        assert_eq!(
            value["_openbridge"]["usage"][0]["accepted_prediction_tokens"],
            1
        );
        assert!(value["usage"].is_null());
        assert_eq!(
            client()
                .decode_response(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .semantic,
            decoded.semantic
        );
    }
}
#[test]
fn carrier_conflicts_terminal_omissions_and_budgets_poison_without_repair() {
    let mut value = fixture();
    value["output"][0]["encrypted_content"] = json!("competing");
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    let mut value = fixture();
    value["output"][2]["_openbridge"]["arguments"] = json!("json_partial");
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    let mut value = fixture();
    value["output"][0]["_openbridge"]["replay"]["value"] = json!("x".repeat(MAX_TEXT_BYTES + 1));
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    for partial in [false, true] {
        let mut events = client_wire::events();
        if partial {
            events.last_mut().unwrap()["response"]["output"][0]["_openbridge"]["replay"]["phase"] =
                json!("partial");
        } else {
            events.last_mut().unwrap()["response"]["output"][0]
                .as_object_mut()
                .unwrap()
                .remove("_openbridge");
        }
        let mut decoder = client().event_decoder();
        assert!(events.iter().any(|event| decoder.push(event).is_err()));
        assert!(decoder.finish().is_err());
    }
}
#[test]
fn carrier_versions_refs_domains_and_duplicate_keys_fail_closed() {
    for bad in [
        json!({"version":2}),
        json!({"version":1,"origin":"never-public"}),
        json!({"version":1,"replay":{"format":"unknown","phase":"final","value":"opaque"}}),
    ] {
        let mut value = fixture();
        value["output"][0]["_openbridge"] = bad;
        assert!(
            client()
                .decode_response(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
    let mut value = fixture();
    value["output"][2]["_openbridge"]["message"] = json!("thought");
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    let mut value = fixture();
    value["output"][2]["arguments"] = json!("{\"n\":1,\"n\":2}");
    assert!(
        client()
            .decode_response(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
}
