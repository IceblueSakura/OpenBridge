//! Chat wire admission for readable reasoning text (`reasoning_content`) and the
//! standard `system_fingerprint` reported fact, including Static/Event consistency.
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, RepresentationError, lower_request,
        lower_response,
    },
    protocol::openai::{
        DecodedResponse, Profile, chat, chat_envelope, events::EventDecoder, events::EventEncoder,
    },
    semantic::{
        task::generation::*,
        value::{Presence, Text},
    },
};
use serde_json::{Value, json};

fn response_with_reasoning(fingerprint: Option<&str>) -> Value {
    let mut wire = json!({"id":"c1","object":"chat.completion","created":1,"model":"m",
        "choices":[{"index":0,"message":{"role":"assistant","content":"pong","reasoning_content":"think"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}});
    if let Some(fingerprint) = fingerprint {
        wire["system_fingerprint"] = json!(fingerprint);
    }
    wire
}

fn message_text(decoded: &DecodedResponse) -> String {
    decoded
        .semantic
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Message(m) => match &m.parts[0].content {
                ContentPart::Text(t) => Some(t.as_str().to_owned()),
                _ => None,
            },
            _ => None,
        })
        .expect("one assistant message")
}

#[test]
fn chat_reasoning_content_is_typed_and_round_trips_its_message() {
    let wire = response_with_reasoning(None);
    let decoded = chat_envelope::decode_response_bytes(wire.to_string().as_bytes()).unwrap();
    let kinds: Vec<&str> = decoded
        .semantic
        .items()
        .iter()
        .map(|(_, item)| match item {
            Item::Reasoning(reasoning) => {
                assert!(matches!(
                    &reasoning.parts[0].1,
                    ReasoningContent::Text(text) if text.as_str() == "think"
                ));
                "reasoning"
            }
            Item::Message(_) => "message",
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        kinds,
        ["reasoning", "message"],
        "reasoning stays ahead of its carrier message"
    );

    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    let output = chat_envelope::encode_response(&target).unwrap();
    assert_eq!(
        output["choices"][0]["message"]["reasoning_content"], "think",
        "native re-encode keeps the reported reasoning text"
    );
    assert_eq!(output["choices"][0]["message"]["content"], "pong");
}

#[test]
fn request_history_reasoning_content_replays_through_chat_encoding() {
    let wire = json!({"messages":[
        {"role":"user","content":"hi"},
        {"role":"assistant","content":"checking","reasoning_content":"think",
         "tool_calls":[{"id":"call_a","type":"function","function":{"name":"lookup","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"call_a","content":"ok"}
    ],"tools":[{"type":"function","function":{"name":"lookup","description":"","parameters":{"type":"object","properties":{"k":{"type":"string"}}},"strict":false}}]});
    let decoded = chat::decode_generation(&wire).unwrap();
    let target = lower_request(
        &decoded.semantic,
        &decoded.fidelity,
        Profile::Chat,
        Contract::full(),
    )
    .expect("chat-representable reasoning shape");
    let output = chat::encode_generation(&target).unwrap();
    assert_eq!(output["messages"][1]["reasoning_content"], "think");
    assert_eq!(output["messages"][1]["content"], "checking");
    assert_eq!(output["messages"][1]["tool_calls"][0]["id"], "call_a");
}

#[test]
fn chat_reasoning_stream_orders_ahead_of_the_message_consistently() {
    let chunks = [
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":null,"reasoning_content":""},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"reasoning_content":"think"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}),
    ];
    let mut decoder = EventDecoder::new(Profile::Chat);
    let mut events = vec![];
    for chunk in &chunks {
        events.extend(decoder.push(chunk).unwrap());
    }
    events.extend(decoder.done().unwrap());
    decoder.finish().unwrap();
    let kinds: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ItemStarted { kind, .. } => Some(format!("{kind:?}")),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Reasoning".to_string(),
            "Message { phase: None }".to_string()
        ],
        "readable reasoning stays ahead of its carrier message in event order"
    );
    let streamed = decoder.materialize().unwrap();
    let static_decoded =
        chat_envelope::decode_response_bytes(response_with_reasoning(None).to_string().as_bytes())
            .unwrap();
    assert_eq!(
        streamed.semantic.items().len(),
        static_decoded.semantic.items().len(),
        "Static/Event item closure agrees"
    );
    assert_eq!(message_text(&streamed), message_text(&static_decoded));
}

#[test]
fn encoded_chat_reasoning_frames_decode_back_to_the_same_semantics() {
    let wire = response_with_reasoning(None);
    let decoded = chat_envelope::decode_response_bytes(wire.to_string().as_bytes()).unwrap();

    let mut decoder = EventDecoder::new(Profile::Chat);
    let mut events = vec![];
    for chunk in [
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"think"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}),
    ] {
        events.extend(decoder.push(&chunk).unwrap());
    }
    events.extend(decoder.done().unwrap());
    decoder.finish().unwrap();

    let mut encoder = EventEncoder::new(Profile::Chat, decoded.metadata.clone())
        .unwrap()
        .with_contract(Contract::full());
    let mut frames = vec![];
    for event in &events {
        frames.extend(encoder.encode(event, decoder.fidelity()).unwrap());
    }
    encoder.finish().unwrap();
    assert!(
        frames
            .iter()
            .any(|frame| frame.to_string().contains("reasoning_content")),
        "reasoning fragments render as reasoning deltas"
    );

    let mut replay = EventDecoder::new(Profile::Chat);
    for frame in &frames {
        replay.push(frame).unwrap();
    }
    replay.done().unwrap();
    replay.finish().unwrap();
    assert_eq!(replay.materialize().unwrap().semantic, decoded.semantic);
}

#[test]
fn reasoning_shapes_outside_the_chat_mapping_are_rejected() {
    for wire in [
        json!({"messages":[{"role":"user","content":"x","reasoning_content":"bad"}]}),
        json!({"messages":[{"role":"assistant","content":"x","reasoning_content":5}]}),
    ] {
        assert!(
            chat::decode_generation(&wire).is_err(),
            "reasoning_content belongs to assistant text only: {wire}"
        );
    }

    // Summary-only or unpaired reasoning has no Chat carrier and must not be dropped.
    let wire = json!({"messages":[
        {"role":"user","content":"hi"},
        {"role":"assistant","content":"a","reasoning_content":"think"}
    ]});
    let decoded = chat::decode_generation(&wire).unwrap();
    let mut items = decoded.semantic.items().to_vec();
    for (_, item) in &mut items {
        if let Item::Reasoning(reasoning) = item {
            reasoning.parts[0].1 =
                ReasoningContent::Summary(Text::new("think", "fixture", 128).unwrap());
        }
    }
    let request = GenerationRequest::new(items, GenerationControls::default()).unwrap();
    assert!(matches!(
        lower_request(&request, &decoded.fidelity, Profile::Chat, Contract::full()),
        Err(RepresentationError::Reasoning)
    ));

    let mut items = decoded.semantic.items().to_vec();
    items.retain(|(_, item)| !matches!(item, Item::Message(m) if m.role == MessageRole::Assistant));
    let request = GenerationRequest::new(items, GenerationControls::default()).unwrap();
    assert!(
        matches!(
            lower_request(&request, &decoded.fidelity, Profile::Chat, Contract::full()),
            Err(RepresentationError::Reasoning)
        ),
        "reasoning without its carrier message cannot be silently dropped"
    );
}

#[test]
fn system_fingerprint_is_a_chat_reported_fact_with_stream_identity() {
    let decoded = chat_envelope::decode_response_bytes(
        response_with_reasoning(Some("fp-1")).to_string().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        decoded.metadata.context.system_fingerprint,
        Presence::Value("fp-1".to_string())
    );
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    let output = chat_envelope::encode_response(&target).unwrap();
    assert_eq!(output["system_fingerprint"], "fp-1");

    // No fingerprint reported means none is invented downstream.
    let plain =
        chat_envelope::decode_response_bytes(response_with_reasoning(None).to_string().as_bytes())
            .unwrap();
    let target = lower_response(
        &plain.semantic,
        &plain.fidelity,
        &plain.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    assert!(
        chat_envelope::encode_response(&target)
            .unwrap()
            .get("system_fingerprint")
            .is_none()
    );

    // The first reported fingerprint and creation time bind the stream; providers
    // re-stamp these scalars per chunk and later drift is normalized away, while
    // response identity (id/model) stays strict.
    let chunk = |fingerprint: &str, created: u64| {
        json!({"id":"c1","object":"chat.completion.chunk","created":created,"model":"m","system_fingerprint":fingerprint,
            "choices":[{"index":0,"delta":{"content":"x"},"finish_reason":null}],"usage":null})
    };
    let mut decoder = EventDecoder::new(Profile::Chat);
    decoder.push(&chunk("fp-1", 1)).unwrap();
    decoder.push(&chunk("fp-2", 2)).unwrap();
    let metadata = decoder.metadata().unwrap();
    assert_eq!(
        metadata.context.system_fingerprint,
        Presence::Value("fp-1".to_string())
    );
    assert_eq!(metadata.created.as_u64(), Some(1));
    let mut mixed = EventDecoder::new(Profile::Chat);
    mixed.push(&chunk("fp-1", 1)).unwrap();
    assert!(
        mixed
            .push(&json!({"id":"other","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"x"},"finish_reason":null}],"usage":null}))
            .is_err(),
        "response identity stays strict"
    );

    // Responses wire has no position for the fact: projection rejects, never drops.
    assert!(matches!(
        lower_response(
            &decoded.semantic,
            &decoded.fidelity,
            &decoded.metadata,
            Profile::Responses,
            Contract::full(),
        ),
        Err(RepresentationError::UnmigratedSemantic)
    ));
}

#[test]
fn usage_aliases_are_normalized_and_disagreement_is_rejected() {
    let mut wire = response_with_reasoning(None);
    wire["usage"] = json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":5,
        "prompt_tokens_details":{"cached_tokens":1},
        "completion_tokens_details":{"reasoning_tokens":1},
        "prompt_cache_hit_tokens":1,"prompt_cache_miss_tokens":2});
    let decoded = chat_envelope::decode_response_bytes(wire.to_string().as_bytes()).unwrap();
    let usage = decoded.semantic.usage().unwrap();
    assert_eq!(
        usage.cached_input_tokens,
        Some(1),
        "alias normalizes to the standard count"
    );
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    let output = chat_envelope::encode_response(&target).unwrap();
    assert_eq!(
        output["usage"].get("prompt_cache_hit_tokens"),
        None,
        "aliases never re-emit beside their standard form"
    );
    assert_eq!(output["usage"]["prompt_tokens_details"]["cached_tokens"], 1);

    let mut disagreeing = wire.clone();
    disagreeing["usage"]["prompt_cache_hit_tokens"] = json!(0);
    assert!(chat_envelope::decode_response_bytes(disagreeing.to_string().as_bytes()).is_err());
    let mut inconsistent = wire.clone();
    inconsistent["usage"]["prompt_cache_miss_tokens"] = json!(9);
    assert!(chat_envelope::decode_response_bytes(inconsistent.to_string().as_bytes()).is_err());
    let mut unknown = wire.clone();
    unknown["usage"]["mystery_tokens"] = json!(1);
    assert!(
        chat_envelope::decode_response_bytes(unknown.to_string().as_bytes()).is_err(),
        "unknown usage keys are rejected, not silently dropped"
    );
}

#[test]
fn usage_belongs_to_the_terminal_chunk_or_the_trailing_tail() {
    let finish_with_usage = json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m",
        "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}});
    let mut decoder = EventDecoder::new(Profile::Chat);
    decoder
        .push(&json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":null}],"usage":null}))
        .unwrap();
    let mut events = decoder.push(&finish_with_usage).unwrap();
    events.extend(decoder.done().unwrap());
    decoder.finish().unwrap();
    assert!(
        events.iter().any(|e| matches!(e, StreamEvent::Usage(_))),
        "usage on the finish chunk is the terminal tail"
    );

    let early = json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m",
        "choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":null}],
        "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}});
    let mut decoder = EventDecoder::new(Profile::Chat);
    assert!(
        decoder.push(&early).is_err(),
        "mid-stream usage is never guessed"
    );
}

#[test]
fn tool_call_index_and_null_roles_follow_provider_stream_shapes() {
    let wire = json!({"messages":[
        {"role":"user","content":"hi"},
        {"role":"assistant","content":null,"tool_calls":[
            {"index":0,"id":"call_a","type":"function","function":{"name":"lookup","arguments":"{}"}},
            {"index":1,"id":"call_b","type":"function","function":{"name":"lookup","arguments":"{}"}}
        ]}
    ],"tools":[{"type":"function","function":{"name":"lookup","description":"","parameters":{"type":"object","properties":{"k":{"type":"string"}}},"strict":false}}]});
    let decoded = chat::decode_generation(&wire).unwrap();
    assert_eq!(
        decoded
            .semantic
            .items()
            .iter()
            .filter(|(_, i)| matches!(i, Item::ToolCall(_)))
            .count(),
        2
    );
    let mut wrong = wire.clone();
    wrong["messages"][1]["tool_calls"][1]["index"] = json!(7);
    assert!(chat::decode_generation(&wrong).is_err());

    // Provider streams reuse null for absent delta fields.
    let mut decoder = EventDecoder::new(Profile::Chat);
    decoder
        .push(&json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":"","tool_calls":null,"reasoning_content":null},"finish_reason":null}],"usage":null}))
        .unwrap();
    decoder
        .push(&json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":null,"role":null,"tool_calls":null,"reasoning_content":"think"},"finish_reason":null}],"usage":null}))
        .unwrap();
    decoder
        .push(&json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}))
        .unwrap();
    decoder.done().unwrap();
    decoder.finish().unwrap();
    let decoded = decoder.materialize().unwrap();
    assert_eq!(message_text(&decoded), "");
}

#[test]
fn opening_empty_fragments_defer_the_carrier_message() {
    // Provider streams open with `content: ""` before their thinking text; the
    // empty fragment defers the message so reasoning keeps its static order.
    let chunks = [
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"","role":"assistant","tool_calls":null,"reasoning_content":null},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":null,"role":null,"tool_calls":null,"reasoning_content":"think"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":"pong"},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}),
    ];
    let mut decoder = EventDecoder::new(Profile::Chat);
    let mut events = vec![];
    for chunk in &chunks {
        events.extend(decoder.push(chunk).unwrap());
    }
    events.extend(decoder.done().unwrap());
    decoder.finish().unwrap();
    let kinds: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::ItemStarted { kind, .. } => Some(format!("{kind:?}")),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Reasoning".to_string(),
            "Message { phase: None }".to_string()
        ]
    );

    // A stream that only opens an empty part materializes it at the terminal,
    // matching static `content: ""` messages (e.g. tool-call carriers).
    let mut decoder = EventDecoder::new(Profile::Chat);
    let mut events = vec![];
    for chunk in [
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"content":""},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"lookup","arguments":"{}"}}]},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":null}),
    ] {
        events.extend(decoder.push(&chunk).unwrap());
    }
    events.extend(decoder.done().unwrap());
    decoder.finish().unwrap();
    let materialized = decoder.materialize().unwrap();
    let message = materialized
        .semantic
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Message(m) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        message.parts.len(),
        1,
        "the deferred empty part materializes"
    );
    assert!(
        matches!(&message.parts[0].content, ContentPart::Text(t) if t.as_str().is_empty()),
        "static `content: \"\"` and its stream agree"
    );
}

#[test]
fn tool_call_continuation_chunks_reuse_null_identity() {
    // Providers announce call identity once and continue with explicit nulls.
    let chunks = [
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"lookup","arguments":""}}]},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":null,"type":"function","function":{"name":null,"arguments":"{\"k\":1}"}}]},"finish_reason":null}],"usage":null}),
        json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":null}),
    ];
    let mut decoder = EventDecoder::new(Profile::Chat);
    for chunk in &chunks {
        decoder.push(chunk).unwrap();
    }
    decoder.done().unwrap();
    decoder.finish().unwrap();
    let materialized = decoder.materialize().unwrap();
    let call = materialized
        .semantic
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::ToolCall(call) => Some(call),
            _ => None,
        })
        .unwrap();
    assert_eq!(call.call_id.as_str(), "call_a");
    assert_eq!(call.arguments, "{\"k\":1}");

    let mut conflicting = EventDecoder::new(Profile::Chat);
    conflicting.push(&chunks[0]).unwrap();
    conflicting.push(&chunks[1]).unwrap();
    assert!(
        conflicting
            .push(&json!({"id":"c1","object":"chat.completion.chunk","created":1,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_b","type":"function","function":{"name":"lookup","arguments":""}}]},"finish_reason":null}],"usage":null}))
            .is_err(),
        "conflicting non-null identity is still rejected"
    );
}

#[test]
fn an_empty_tool_calls_array_declares_no_calls() {
    // Vendor shape: the model hit its budget and declared an explicit empty call
    // list while emitting its attempt as message text.
    let wire = json!({"id":"c1","object":"chat.completion","created":1,"model":"m",
        "choices":[{"index":0,"message":{"role":"assistant","content":"<tool_call>","tool_calls":[]},"finish_reason":"length"}],
        "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}});
    let decoded = chat_envelope::decode_response_bytes(wire.to_string().as_bytes()).unwrap();
    assert!(
        !decoded
            .semantic
            .items()
            .iter()
            .any(|(_, item)| matches!(item, Item::ToolCall(_))),
        "an empty declaration invents no calls"
    );
    assert_eq!(message_text(&decoded), "<tool_call>");
    assert_eq!(
        decoded.semantic.details().incomplete,
        Some(IncompleteReason::MaxOutputTokens)
    );

    // Re-encoding never fabricates a call list, and the truncation stays visible.
    let target = lower_response(
        &decoded.semantic,
        &decoded.fidelity,
        &decoded.metadata,
        Profile::Chat,
        Contract::full(),
    )
    .unwrap();
    let out = chat_envelope::encode_response(&target).unwrap();
    assert!(out["choices"][0]["message"].get("tool_calls").is_none());
    assert_eq!(out["choices"][0]["finish_reason"], "length");
}
