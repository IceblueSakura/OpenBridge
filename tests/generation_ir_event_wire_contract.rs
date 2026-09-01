//! Wire stream materialization parity with the non-stream Static decoder.

use bytes::Bytes;
use openbridge::{
    bridge::{StaticBridgePlan, StaticCodecLimits, StaticEventBridge, StaticEventCodecError},
    core::{ApiProtocol, ReasoningOutput},
    ir::generation::EventLimits,
    transport::sse::SseDecoder,
};
use serde_json::{Value, json};

fn decode(document: &[u8]) -> Vec<openbridge::transport::sse::SseEvent> {
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(document).expect("SSE document must decode");
    events.extend(decoder.finish().expect("SSE document must finish"));
    events
}

fn responses_bridge_after(document: &[u8]) -> StaticEventBridge {
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Summary,
        false,
        EventLimits::new(4096, 4096, 16 * 1024).unwrap(),
    )
    .unwrap();
    for event in decode(document) {
        bridge.render(event).unwrap();
    }
    bridge
}

#[test]
fn event_bridge_bounds_cross_protocol_output_expansion() {
    let source = b"data: {\"id\":\"chatcmpl_bound\",\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null,\"index\":0}]}\n\n";
    let event = decode(source).remove(0);
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(256, 256, 1).unwrap(),
    )
    .unwrap();

    assert_eq!(
        bridge.render(event),
        Err(StaticEventCodecError::LimitExceeded)
    );
}

#[test]
fn chat_event_decoder_rejects_named_sse_events() {
    let source = b"event: response.delta\ndata: {\"id\":\"chatcmpl_named\",\"choices\":[{\"delta\":{\"content\":\"x\"},\"finish_reason\":null,\"index\":0}]}\n\n";
    let event = decode(source).remove(0);
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(256, 256, 1024).unwrap(),
    )
    .unwrap();

    assert_eq!(
        bridge.render(event),
        Err(StaticEventCodecError::IdentityConflict)
    );
}

#[test]
fn chat_tool_index_overflow_is_rejected_before_canonical_identity_creation() {
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::PlainText,
        false,
        EventLimits::new(4096, 4096, 16 * 1024).unwrap(),
    )
    .unwrap();
    let reasoning = b"data: {\"id\":\"chatcmpl_overflow\",\"choices\":[{\"delta\":{\"reasoning_content\":\"x\"},\"finish_reason\":null,\"index\":0}]}\n\n";
    bridge.render(decode(reasoning).remove(0)).unwrap();
    let tool = b"data: {\"id\":\"chatcmpl_overflow\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":18446744073709551615,\"id\":\"call_overflow\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{}\"}}]},\"finish_reason\":null,\"index\":0}]}\n\n";
    assert_eq!(
        bridge.render(decode(tool).remove(0)),
        Err(StaticEventCodecError::LimitExceeded)
    );
}

#[test]
fn responses_start_events_cannot_hide_preexisting_output() {
    let limits = EventLimits::new(1024, 1024, 4096).unwrap();
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        limits,
    )
    .unwrap();
    let created_with_output = b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_hidden\",\"status\":\"in_progress\",\"output\":[{}]}}\n\n";
    assert!(
        bridge
            .render(decode(created_with_output).remove(0))
            .is_err()
    );

    let mut bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        limits,
    )
    .unwrap();
    let created = b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_hidden\",\"status\":\"in_progress\",\"output\":[]}}\n\n";
    bridge.render(decode(created).remove(0)).unwrap();
    let item_with_content = b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_hidden\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"in_progress\",\"content\":[{\"type\":\"output_text\",\"text\":\"hidden\",\"annotations\":[]}]}}\n\n";
    assert!(bridge.render(decode(item_with_content).remove(0)).is_err());
}

#[test]
fn responses_child_indexes_are_required_and_must_select_part_zero() {
    let message_start = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_index","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"msg_index","type":"message","role":"assistant","status":"in_progress","content":[]}}

"#;
    for delta in [
        r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_index","output_index":0,"delta":"x"}

"#,
        r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_index","output_index":0,"content_index":1,"delta":"x"}

"#,
    ] {
        let mut bridge = responses_bridge_after(message_start);
        assert!(bridge.render(decode(delta.as_bytes()).remove(0)).is_err());
    }

    let reasoning_start = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_summary_index","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"rs_index","type":"reasoning","status":"in_progress","content":[],"summary":[]}}

"#;
    for part in [
        r#"event: response.reasoning_summary_part.added
data: {"type":"response.reasoning_summary_part.added","item_id":"rs_index","output_index":0,"part":{"type":"summary_text","text":""}}

"#,
        r#"event: response.reasoning_summary_part.added
data: {"type":"response.reasoning_summary_part.added","item_id":"rs_index","output_index":0,"summary_index":1,"part":{"type":"summary_text","text":""}}

"#,
    ] {
        let mut bridge = responses_bridge_after(reasoning_start);
        assert!(bridge.render(decode(part.as_bytes()).remove(0)).is_err());
    }

    let summary_open = format!(
        "{}{}{}",
        std::str::from_utf8(reasoning_start).unwrap(),
        "event: response.reasoning_summary_part.added\ndata: {\"type\":\"response.reasoning_summary_part.added\",\"item_id\":\"rs_index\",\"output_index\":0,\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"\"}}\n\n",
        "event: response.reasoning_summary_text.delta\ndata: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"rs_index\",\"output_index\":0,\"summary_index\":0,\"delta\":\"x\"}\n\n"
    );
    for done in [
        r#"event: response.reasoning_summary_text.done
data: {"type":"response.reasoning_summary_text.done","item_id":"rs_index","output_index":0,"text":"x"}

"#,
        r#"event: response.reasoning_summary_text.done
data: {"type":"response.reasoning_summary_text.done","item_id":"rs_index","output_index":0,"summary_index":1,"text":"x"}

"#,
    ] {
        let mut bridge = responses_bridge_after(summary_open.as_bytes());
        assert!(bridge.render(decode(done.as_bytes()).remove(0)).is_err());
    }
}

#[test]
fn responses_child_events_after_parent_completion_are_rejected() {
    let message_done = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_late_message","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"msg_late","type":"message","role":"assistant","status":"in_progress","content":[]}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_late","output_index":0,"content_index":0,"delta":"x"}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"msg_late","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"x","annotations":[]}]}}

"#;
    for late in [
        r#"event: response.output_text.done
data: {"type":"response.output_text.done","item_id":"msg_late","output_index":0,"content_index":0,"text":"x"}

"#,
        r#"event: response.content_part.done
data: {"type":"response.content_part.done","item_id":"msg_late","output_index":0,"content_index":0,"part":{"type":"output_text","text":"x","annotations":[]}}

"#,
    ] {
        let mut bridge = responses_bridge_after(message_done);
        assert!(bridge.render(decode(late.as_bytes()).remove(0)).is_err());
    }

    let reasoning_done = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_late_reasoning","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"rs_late","type":"reasoning","status":"in_progress","content":[],"summary":[]}}

event: response.reasoning_summary_part.added
data: {"type":"response.reasoning_summary_part.added","item_id":"rs_late","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":""}}

event: response.reasoning_summary_text.delta
data: {"type":"response.reasoning_summary_text.delta","item_id":"rs_late","output_index":0,"summary_index":0,"delta":"x"}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"rs_late","type":"reasoning","status":"completed","content":[],"summary":[{"type":"summary_text","text":"x"}]}}

"#;
    for late in [
        r#"event: response.reasoning_summary_text.done
data: {"type":"response.reasoning_summary_text.done","item_id":"rs_late","output_index":0,"summary_index":0,"text":"x"}

"#,
        r#"event: response.reasoning_summary_part.done
data: {"type":"response.reasoning_summary_part.done","item_id":"rs_late","output_index":0,"summary_index":0,"part":{"type":"summary_text","text":"x"}}

"#,
    ] {
        let mut bridge = responses_bridge_after(reasoning_done);
        assert!(bridge.render(decode(late.as_bytes()).remove(0)).is_err());
    }

    let tool_done = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_late_tool","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"fc_late","type":"function_call","status":"in_progress","call_id":"call_late","name":"lookup","arguments":"{}"}}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"fc_late","type":"function_call","status":"completed","call_id":"call_late","name":"lookup","arguments":"{}"}}

"#;
    let late_arguments = br#"event: response.function_call_arguments.done
data: {"type":"response.function_call_arguments.done","item_id":"fc_late","output_index":0,"arguments":"{}"}

"#;
    let mut bridge = responses_bridge_after(tool_done);
    assert!(bridge.render(decode(late_arguments).remove(0)).is_err());
}

#[test]
fn responses_stream_materializes_to_the_same_static_response_as_non_stream() {
    let limits = StaticCodecLimits::new(256 * 1024, 256 * 1024).unwrap();
    let (static_plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":false}"#,
        ),
        limits,
    )
    .unwrap();
    let non_stream = static_plan
        .render_non_stream(Bytes::from_static(
            br#"{"id":"resp_equivalent","object":"response","status":"completed","output":[{"id":"msg_equivalent","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"hello","annotations":[]}]}],"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}"#,
        ))
        .unwrap();

    let stream = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_equivalent","status":"in_progress","output":[]}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"id":"msg_equivalent","type":"message","role":"assistant","status":"in_progress","content":[]}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_equivalent","output_index":0,"content_index":0,"delta":"hello"}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"id":"msg_equivalent","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"hello","annotations":[]}]}}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_equivalent","status":"completed","output":[{"id":"msg_equivalent","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"hello","annotations":[]}]}],"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}}

"#;

    let mut event_bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(256 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    for event in decode(stream) {
        event_bridge.render(event).unwrap();
    }
    event_bridge.finish().unwrap();

    assert_eq!(
        event_bridge.materialized_response().unwrap(),
        *non_stream.semantic()
    );

    let mut native = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(256 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .expect("Native Responses events must pass through canonical Event IR");
    let mut encoded = Vec::new();
    for event in decode(stream) {
        encoded.extend_from_slice(&native.render(event).unwrap());
    }
    native.finish().unwrap();
    assert!(encoded.is_empty());
    assert_eq!(
        native.materialized_response().unwrap(),
        *non_stream.semantic()
    );
}

#[test]
fn chatgpt_responses_stream_replays_through_the_production_event_profile() {
    let stream = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_synthetic","model":"gpt-5.6-luna","object":"response","output":[],"status":"in_progress"}}

event: response.in_progress
data: {"type":"response.in_progress","response":{"id":"resp_synthetic","status":"in_progress"}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,"item":{"content":[],"encrypted_content":null,"id":"rs_synthetic","summary":[],"type":"reasoning"}}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":0,"item":{"content":[],"encrypted_content":"synthetic-opaque-continuation","id":"rs_synthetic","summary":[],"type":"reasoning"}}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":1,"item":{"id":"msg_synthetic","type":"message","role":"assistant","content":[]}}

event: response.content_part.added
data: {"type":"response.content_part.added","output_index":1,"item_id":"msg_synthetic","content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","output_index":1,"item_id":"msg_synthetic","content_index":0,"delta":"hello"}

event: response.output_text.done
data: {"type":"response.output_text.done","output_index":1,"item_id":"msg_synthetic","content_index":0,"text":"hello"}

event: response.content_part.done
data: {"type":"response.content_part.done","output_index":1,"item_id":"msg_synthetic","content_index":0,"part":{"type":"output_text","text":"hello","annotations":[]}}

event: response.output_item.done
data: {"type":"response.output_item.done","output_index":1,"item":{"id":"msg_synthetic","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"hello","annotations":[]}]}}

event: response.completed
data: {"type":"response.completed","response":{"id":"resp_synthetic","status":"completed","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}

"#;
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Summary,
        false,
        EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    for event in decode(stream) {
        let kind = event.event().unwrap_or("data-only").to_owned();
        bridge
            .render(event)
            .unwrap_or_else(|error| panic!("{kind} must render: {error:?}"));
    }
    bridge
        .finish()
        .expect("completed ChatGPT stream must finish");
}

#[test]
fn chat_usage_snapshot_requires_complete_consistent_totals() {
    let stream = br#"data: {"id":"chat_usage","choices":[{"delta":{"role":"assistant","content":"ok"},"finish_reason":"stop","index":0}]}

data: {"id":"chat_usage","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1}}

data: [DONE]

"#;
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    let mut events = decode(stream);
    bridge.render(events.remove(0)).unwrap();
    assert!(bridge.render(events.remove(0)).is_err());
}

#[test]
fn event_usage_rejects_malformed_detail_containers() {
    for field in ["completion_tokens_details", "prompt_tokens_details"] {
        let mut bridge = StaticEventBridge::new(
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "public-model",
            ReasoningOutput::Unsupported,
            false,
            EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
        )
        .unwrap();
        let terminal = b"data: {\"id\":\"chat_usage\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":\"stop\",\"index\":0}]}\n\n";
        bridge.render(decode(terminal).remove(0)).unwrap();
        let mut usage = json!({"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2});
        usage[field] = json!(false);
        let event = format!(
            "data: {}\n\n",
            json!({"id": "chat_usage", "choices": [], "usage": usage})
        );
        assert!(bridge.render(decode(event.as_bytes()).remove(0)).is_err());
    }

    for field in ["output_tokens_details", "input_tokens_details"] {
        let mut bridge = StaticEventBridge::new(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            ReasoningOutput::Unsupported,
            false,
            EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
        )
        .unwrap();
        let mut usage = json!({"input_tokens": 1, "output_tokens": 1, "total_tokens": 2});
        usage[field] = json!([]);
        let event = format!(
            "event: response.completed\ndata: {}\n\n",
            json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_usage",
                    "status": "completed",
                    "output": [],
                    "usage": usage
                }
            })
        );
        assert!(bridge.render(decode(event.as_bytes()).remove(0)).is_err());
    }
}

#[test]
fn native_chat_stream_is_canonically_validated_without_requiring_reencoded_bytes() {
    for stream in [
        "data: {\"id\":\"chat_asr\",\"object\":\"chat.completion.chunk\",\"model\":\"mimo-v2.5-asr\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"transcript\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_asr\",\"object\":\"chat.completion.chunk\",\"model\":\"mimo-v2.5-asr\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        "data: {\"id\":\"chat_audio\",\"object\":\"chat.completion.chunk\",\"model\":\"mimo-v2.5-tts\",\"choices\":[{\"index\":0,\"delta\":{\"audio\":{\"data\":\"UklG\"}},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_audio\",\"object\":\"chat.completion.chunk\",\"model\":\"mimo-v2.5-tts\",\"choices\":[{\"index\":0,\"delta\":{\"audio\":{\"data\":\"Rg==\"}},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_audio\",\"object\":\"chat.completion.chunk\",\"model\":\"mimo-v2.5-tts\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
    ] {
        let mut bridge = StaticEventBridge::new(
            ApiProtocol::ChatCompletions,
            ApiProtocol::ChatCompletions,
            "public-model",
            ReasoningOutput::Unsupported,
            false,
            EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
        )
        .unwrap();
        for (index, event) in decode(stream.as_bytes()).into_iter().enumerate() {
            bridge
                .render(event)
                .unwrap_or_else(|error| panic!("event {index} failed: {error:?}"));
        }
        bridge.finish().unwrap();
    }
}

#[test]
fn native_chat_stream_accepts_standard_incomplete_finish_reasons() {
    for finish_reason in ["length", "content_filter"] {
        let stream = format!(
            "data: {{\"id\":\"chat_incomplete\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"partial\"}},\"finish_reason\":null}}]}}\n\ndata: {{\"id\":\"chat_incomplete\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"{finish_reason}\"}}]}}\n\ndata: [DONE]\n\n"
        );
        let mut bridge = StaticEventBridge::new(
            ApiProtocol::ChatCompletions,
            ApiProtocol::ChatCompletions,
            "public-model",
            ReasoningOutput::Unsupported,
            false,
            EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
        )
        .unwrap();
        for event in decode(stream.as_bytes()) {
            bridge
                .render(event)
                .expect("Native Chat must retain a standard incomplete finish reason");
        }
        bridge.finish().unwrap();
        assert!(bridge.materialized_response().is_err());
    }
}

#[test]
fn native_chat_stream_preserves_multiple_unknown_reasoning_deltas() {
    let stream = b"data: {\"id\":\"chat_unknown_reasoning\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"first\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_unknown_reasoning\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"second\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"chat_unknown_reasoning\",\"object\":\"chat.completion.chunk\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::ChatCompletions,
        ApiProtocol::ChatCompletions,
        "public-model",
        ReasoningOutput::Unknown,
        false,
        EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    for event in decode(stream) {
        bridge.render(event).unwrap();
    }
    bridge.finish().unwrap();
}

#[test]
fn native_responses_accepts_a_sparse_terminal_only_completed_lifecycle() {
    let stream = b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_success\",\"status\":\"completed\"}}\n\n";
    let mut bridge = StaticEventBridge::new(
        ApiProtocol::Responses,
        ApiProtocol::Responses,
        "public-model",
        ReasoningOutput::Unsupported,
        false,
        EventLimits::new(64 * 1024, 256 * 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    for event in decode(stream) {
        bridge.render(event).unwrap();
    }
    bridge.finish().unwrap();
}
