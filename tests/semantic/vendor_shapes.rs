//! Vendor wire adaptation (ADR 0008): presence-preserving reported facts,
//! derived-view validation and classified response extras over a stable core IR.
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, ReportedFactPolicy, RepresentationError,
        lower_response,
    },
    protocol::{
        fidelity::FidelityRecords,
        openai::{Profile, chat_envelope, envelope, events::EventDecoder},
    },
    semantic::task::generation::{PartKind, StreamEvent},
};
use serde_json::{Value, json};

/// Vendor-shaped Responses body: no settings echo, usage without cache-write
/// details, a single assistant message.
fn vendor_body() -> Value {
    json!({"id":"r1","object":"response","created_at":2,"status":"completed","model":"m",
        "output":[{"id":"m1","type":"message","role":"assistant","status":"completed",
            "content":[{"type":"output_text","text":"pong","annotations":[]}]}],
        "usage":{"input_tokens":3,"input_tokens_details":{"cached_tokens":0},
            "output_tokens":2,"output_tokens_details":{"reasoning_tokens":1},"total_tokens":5}})
}

#[test]
fn vendor_usage_absence_is_kept_and_never_filled() {
    let decoded = envelope::decode_response_bytes(vendor_body().to_string().as_bytes()).unwrap();
    let usage = decoded.semantic.usage().unwrap();
    assert_eq!(
        usage.input_cache_write_tokens, None,
        "no cache-write billing dimension means no reported count"
    );
    assert_eq!(
        usage.cached_input_tokens,
        Some(0),
        "a reported zero is a count"
    );

    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let output = envelope::encode_response(&target).unwrap();
    assert!(
        output["usage"]["input_tokens_details"]
            .get("cache_write_tokens")
            .is_none(),
        "absence is never filled with 0 or a default"
    );
    assert_eq!(output["usage"]["input_tokens_details"]["cached_tokens"], 0);

    // A reported zero stays a count, distinct from absence.
    let mut with_zero = vendor_body();
    with_zero["usage"]["input_tokens_details"]["cache_write_tokens"] = json!(0);
    let decoded = envelope::decode_response_bytes(with_zero.to_string().as_bytes()).unwrap();
    assert_eq!(
        decoded.semantic.usage().unwrap().input_cache_write_tokens,
        Some(0)
    );

    // Explicit null means no reported count and normalizes to absence.
    let mut with_null = vendor_body();
    with_null["usage"]["input_tokens_details"]["cache_write_tokens"] = Value::Null;
    let decoded = envelope::decode_response_bytes(with_null.to_string().as_bytes()).unwrap();
    assert_eq!(
        decoded.semantic.usage().unwrap().input_cache_write_tokens,
        None
    );
}

#[test]
fn settings_echo_absence_survives_without_invention() {
    let decoded = envelope::decode_response_bytes(vendor_body().to_string().as_bytes()).unwrap();
    assert!(
        decoded.metadata.context.settings.is_none(),
        "an absent echo is a reported fact"
    );
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let output = envelope::encode_response(&target).unwrap();
    for key in [
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "instructions",
    ] {
        assert!(
            output.get(key).is_none(),
            "{key} is never invented from request copies or defaults"
        );
    }
}

#[test]
fn derived_output_text_is_validated_then_dropped() {
    let mut matching = vendor_body();
    matching["output_text"] = json!("pong");
    let decoded = envelope::decode_response_bytes(matching.to_string().as_bytes()).unwrap();
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let output = envelope::encode_response(&target).unwrap();
    assert!(
        output.get("output_text").is_none(),
        "a derived view never enters the wire again"
    );

    let mut mismatching = vendor_body();
    mismatching["output_text"] = json!("p0ng");
    assert!(
        envelope::decode_response_bytes(mismatching.to_string().as_bytes()).is_err(),
        "a derived view must equal the authoritative output text"
    );
}

#[test]
fn classified_response_extras_round_trip_same_origin_only() {
    let mut extras = vendor_body();
    extras["content_filters"] = Value::Null;
    extras["frequency_penalty"] = json!(0);
    extras["presence_penalty"] = json!(0);
    let decoded = envelope::decode_response_bytes(extras.to_string().as_bytes()).unwrap();
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Responses,
        Contract::full(),
    )
    .unwrap();
    let output = envelope::encode_response(&target).unwrap();
    assert_eq!(output["content_filters"], Value::Null);
    assert_eq!(output["frequency_penalty"], json!(0));
    assert_eq!(output["presence_penalty"], json!(0));

    // Cross-profile projection drops vendor extras per portability policy while
    // the semantic core keeps its facts.
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    let chat_output = chat_envelope::encode_response(&target).unwrap();
    for key in ["content_filters", "frequency_penalty", "presence_penalty"] {
        assert!(
            chat_output.get(key).is_none(),
            "{key} stays same-origin only"
        );
    }
    assert_eq!(chat_output["choices"][0]["message"]["content"], "pong");
}

#[test]
fn unknown_response_keys_stay_rejected_and_structure_stays_strict() {
    let mut unknown = vendor_body();
    unknown["mystery_field"] = json!(1);
    assert!(
        envelope::decode_response_bytes(unknown.to_string().as_bytes()).is_err(),
        "classified extras are an admit list, not opaque passthrough"
    );

    let mut no_output = vendor_body();
    no_output.as_object_mut().unwrap().remove("output");
    assert!(
        envelope::decode_response_bytes(no_output.to_string().as_bytes()).is_err(),
        "structural snapshot requirements are untouched"
    );
}

#[test]
fn stream_snapshots_adapt_like_static_bodies() {
    // Start from the independent events fixture: strip the unreported cache-write
    // count and add classified extras on the response snapshots.
    let mut frames = crate::wire::events(2);
    for event in &mut frames {
        let Some(response) = event.get_mut("response") else {
            continue;
        };
        if let Some(details) = response
            .pointer_mut("/usage/input_tokens_details")
            .and_then(Value::as_object_mut)
        {
            details.remove("cache_write_tokens");
        }
        response["content_filters"] = Value::Null;
    }
    let completed = frames.last_mut().unwrap();
    completed["response"]["output_text"] = json!("{\"ok\":false}");

    let mut decoder = EventDecoder::new(Profile::Responses);
    for event in &frames {
        decoder.push(event).unwrap();
    }
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert_eq!(
        decoded.semantic.usage().unwrap().input_cache_write_tokens,
        None,
        "absence survives the stream without being filled"
    );
    assert!(
        decoded
            .fidelity
            .response_extras()
            .contains_key("content_filters"),
        "classified extras are captured from stream snapshots"
    );

    // Structural terminal requirements stay strict on streams as well.
    let mut broken = frames.clone();
    broken.last_mut().unwrap()["response"]
        .as_object_mut()
        .unwrap()
        .remove("output");
    let mut decoder = EventDecoder::new(Profile::Responses);
    for event in &broken[..broken.len() - 1] {
        decoder.push(event).unwrap();
    }
    assert!(
        decoder.push(broken.last().unwrap()).is_err(),
        "the structural output array is still required at the terminal"
    );
}

#[test]
fn reasoning_text_parts_are_admitted_in_content_part_events() {
    // Vendor streams announce the reasoning text part via content_part events;
    // the static item shape (`content: [{type:"reasoning_text"}]`) already
    // models it. No terminal is needed to pin the admission.
    let mut decoder = EventDecoder::new(Profile::Responses);
    decoder
        .push(&json!({"type":"response.created","sequence_number":0,"response":{
            "id":"r1","object":"response","created_at":2,"model":"m","status":"in_progress","output":[]}}))
        .unwrap();
    decoder
        .push(&json!({"type":"response.output_item.added","sequence_number":1,"output_index":0,
            "item":{"id":"rs1","type":"reasoning","summary":[],"content":[],"status":"in_progress"}}))
        .unwrap();
    let events = decoder
        .push(
            &json!({"type":"response.content_part.added","sequence_number":2,"output_index":0,
            "item_id":"rs1","content_index":0,"part":{"type":"reasoning_text","text":""}}),
        )
        .unwrap();
    assert!(
        matches!(
            &events[0],
            StreamEvent::PartStarted {
                kind: PartKind::ReasoningText,
                ..
            }
        ),
        "reasoning_text is a first-class part kind on streams"
    );
    decoder
        .push(
            &json!({"type":"response.reasoning_text.delta","sequence_number":3,"output_index":0,
            "item_id":"rs1","content_index":0,"delta":"think"}),
        )
        .unwrap();
    decoder
        .push(
            &json!({"type":"response.reasoning_text.done","sequence_number":4,"output_index":0,
            "item_id":"rs1","content_index":0,"text":"think"}),
        )
        .unwrap();
    let events = decoder
        .push(
            &json!({"type":"response.content_part.done","sequence_number":5,"output_index":0,
            "item_id":"rs1","content_index":0,"part":{"type":"reasoning_text","text":"think"}}),
        )
        .unwrap();
    assert!(!events.is_empty());
}

#[test]
fn encrypted_replay_binds_to_a_trusted_scope_at_the_execution_boundary() {
    let mut body = vendor_body();
    body["output"].as_array_mut().unwrap().insert(
        0,
        json!({"id":"rs1","type":"reasoning","status":"completed","summary":[],
            "content":[{"type":"reasoning_text","text":"think"}],
            "encrypted_content":"token-1"}),
    );
    let mut decoded = envelope::decode_response_bytes(body.to_string().as_bytes()).unwrap();
    let scope = openbridge::semantic::value::ReplayOrigin::new("deepseek").unwrap();

    // Unbound replay cannot be lowered: no scope, no delivery.
    assert!(
        lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            Contract {
                replay_origin: Some(scope.clone()),
                ..Contract::full()
            },
        )
        .is_err()
    );

    // The trusted execution boundary binds the endpoint's scope; native
    // delivery then renders with the same target scope.
    decoded.fidelity.bind_replay_origin(&scope).unwrap();
    assert!(
        lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            Contract {
                replay_origin: Some(scope.clone()),
                ..Contract::full()
            },
        )
        .is_ok()
    );
    assert!(
        lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            Contract::full(),
        )
        .is_err(),
        "a target without a matching scope still refuses the replay"
    );
}

#[test]
fn strict_complete_targets_refuse_absent_facts_instead_of_omitting_them() {
    let vendor = envelope::decode_response_bytes(vendor_body().to_string().as_bytes()).unwrap();
    let strict = Contract {
        reported_facts: ReportedFactPolicy::StrictComplete,
        ..Contract::full()
    };
    assert!(
        lower_response(
            &vendor.semantic,
            &vendor.fidelity,
            &vendor.metadata,
            Profile::Responses,
            Contract::full(),
        )
        .is_ok(),
        "the faithful default delivers absence as a fact"
    );
    assert!(
        matches!(
            lower_response(
                &vendor.semantic,
                &vendor.fidelity,
                &vendor.metadata,
                Profile::Responses,
                strict.clone(),
            ),
            Err(RepresentationError::ReportedFacts)
        ),
        "a strict target refuses to omit absent facts"
    );

    // A body that reports the facts satisfies the same target.
    let complete =
        envelope::decode_response_bytes(crate::wire::response(2).to_string().as_bytes()).unwrap();
    assert!(
        lower_response(
            &complete.semantic,
            &complete.fidelity,
            &complete.metadata,
            Profile::Responses,
            strict,
        )
        .is_ok(),
        "reported facts satisfy a strict target"
    );
}

#[test]
fn classified_extras_portability_is_declared_per_profile() {
    let extras =
        json!({"content_filters": Value::Null, "frequency_penalty": 0, "presence_penalty": 0});
    let extras = extras.as_object().unwrap();
    let mut responses = FidelityRecords::default();
    responses
        .record_response_extras(Profile::Responses, extras)
        .unwrap();
    assert_eq!(responses.response_extras().len(), 3);
    let mut chat = FidelityRecords::default();
    chat.record_response_extras(Profile::Chat, extras).unwrap();
    assert!(
        chat.response_extras().is_empty(),
        "a profile that declares no extras never records them"
    );
}

#[test]
fn derived_output_text_is_checked_where_the_view_claims_finality() {
    // Non-terminal snapshots only exist on streams: a partial view is admitted
    // (still string-typed) while items are still streaming.
    let created = json!({"type":"response.created","sequence_number":0,"response":{
        "id":"r1","object":"response","created_at":2,"model":"m","status":"in_progress",
        "output":[],"output_text":"po"}});
    let mut decoder = EventDecoder::new(Profile::Responses);
    assert!(decoder.push(&created).is_ok());

    // The type rule holds everywhere, terminal or not.
    let mut typed = created.clone();
    typed["response"]["output_text"] = json!(5);
    let mut decoder = EventDecoder::new(Profile::Responses);
    assert!(decoder.push(&typed).is_err());

    // A terminal snapshot must match the authoritative output text.
    let mut terminal = vendor_body();
    terminal["output_text"] = json!("po");
    assert!(envelope::decode_response_bytes(terminal.to_string().as_bytes()).is_err());
}
