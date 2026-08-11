//! Constrains bidirectional Protocol Bridge conversion with accepted canonical artifacts.
//!
//! These tests verify BridgePlan requests, non-streaming responses, and stream renderers directly.
//! Production Router integration is covered separately by the forwarding contract.

use bytes::Bytes;
use openbridge::{
    bridge::{BridgePlan, ChatStreamState, ResponsesStreamState},
    core::{ApiProtocol, ReasoningOutput},
    transport::sse::{SseDecoder, SseEvent},
};
use serde_json::Value;

struct NonStreamCase {
    downstream: ApiProtocol,
    upstream: ApiProtocol,
    directory: &'static str,
}

fn fixture(directory: &str, name: &str) -> Bytes {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/cases/bridge")
        .join(directory)
        .join(name);
    Bytes::from(std::fs::read(path).expect("canonical bridge artifact must exist"))
}

fn assert_json_eq(actual: &[u8], expected: &[u8]) {
    let actual: Value = serde_json::from_slice(actual).expect("actual body must be JSON");
    let expected: Value = serde_json::from_slice(expected).expect("fixture body must be JSON");
    assert_eq!(actual, expected);
}

fn decode(document: &[u8]) -> Vec<SseEvent> {
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(document).expect("fixture SSE must decode");
    events.extend(decoder.finish().expect("fixture SSE must finish"));
    events
}

fn assert_sse_semantics(protocol: ApiProtocol, actual: &[u8], expected: &[u8]) {
    match protocol {
        ApiProtocol::ChatCompletions => {
            let mut actual_state = ChatStreamState::new();
            for event in decode(actual) {
                actual_state.ingest(&event).expect("actual Chat stream");
            }
            actual_state.finish().expect("actual Chat terminal");
            let mut expected_state = ChatStreamState::new();
            for event in decode(expected) {
                expected_state.ingest(&event).expect("expected Chat stream");
            }
            expected_state.finish().expect("expected Chat terminal");
            assert_eq!(actual_state.text(), expected_state.text());
            assert_eq!(
                actual_state.reasoning_text(),
                expected_state.reasoning_text()
            );
            assert_eq!(actual_state.tool_calls(), expected_state.tool_calls());
            assert_eq!(actual_state.terminal(), expected_state.terminal());
        }
        ApiProtocol::Responses => {
            let mut actual_state = ResponsesStreamState::new();
            for event in decode(actual) {
                actual_state
                    .ingest(&event)
                    .expect("actual Responses stream");
            }
            actual_state.finish().expect("actual Responses terminal");
            let mut expected_state = ResponsesStreamState::new();
            for event in decode(expected) {
                expected_state
                    .ingest(&event)
                    .expect("expected Responses stream");
            }
            expected_state
                .finish()
                .expect("expected Responses terminal");
            assert_eq!(actual_state.text(), expected_state.text());
            assert_eq!(
                actual_state.reasoning_text(),
                expected_state.reasoning_text()
            );
            assert_eq!(actual_state.tool_calls(), expected_state.tool_calls());
            assert_eq!(actual_state.terminal(), expected_state.terminal());
        }
    }
}

#[test]
fn canonical_non_stream_requests_and_responses_convert_in_both_directions() {
    let cases = [
        NonStreamCase {
            downstream: ApiProtocol::ChatCompletions,
            upstream: ApiProtocol::Responses,
            directory: "chat_to_responses/chat_to_responses.text.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::ChatCompletions,
            upstream: ApiProtocol::Responses,
            directory: "chat_to_responses/chat_to_responses.single_tool.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::ChatCompletions,
            upstream: ApiProtocol::Responses,
            directory: "chat_to_responses/chat_to_responses.tool_result.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::Responses,
            upstream: ApiProtocol::ChatCompletions,
            directory: "responses_to_chat/responses_to_chat.text.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::Responses,
            upstream: ApiProtocol::ChatCompletions,
            directory: "responses_to_chat/responses_to_chat.single_tool.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::Responses,
            upstream: ApiProtocol::ChatCompletions,
            directory: "responses_to_chat/responses_to_chat.tool_result.non_stream",
        },
        NonStreamCase {
            downstream: ApiProtocol::Responses,
            upstream: ApiProtocol::ChatCompletions,
            directory: "responses_to_chat/responses_to_chat.reverse_tool_results.non_stream",
        },
    ];

    // Verify that each accepted case closes bidirectionally through one BridgePlan.
    for case in cases {
        let client_request = fixture(case.directory, "client-request.json");
        let (plan, upstream_request) = BridgePlan::prepare(
            case.downstream,
            case.upstream,
            "public-model",
            "upstream-model",
            client_request,
        )
        .expect("accepted request must be bridgeable");
        assert_json_eq(
            upstream_request.body(),
            &fixture(case.directory, "expected-upstream-request.json"),
        );

        let client_response = plan
            .render_non_stream(fixture(case.directory, "upstream-response.json"))
            .expect("accepted response must be bridgeable");
        assert_json_eq(
            &client_response,
            &fixture(case.directory, "expected-client-response.json"),
        );
    }
}

#[test]
fn canonical_text_and_parallel_tool_streams_render_in_both_directions() {
    let cases = [
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "chat_to_responses/chat_to_responses.text.stream",
        ),
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "responses_to_chat/responses_to_chat.text.stream",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "responses_to_chat/responses_to_chat.parallel_tools.fragmented_arguments",
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "responses_to_chat/responses_to_chat.empty_object_escaped_arguments.stream",
        ),
    ];

    // Render each event and validate state at EOF so network chunk boundaries cannot affect semantics.
    for (downstream, upstream, directory) in cases {
        let (plan, upstream_request) = BridgePlan::prepare(
            downstream,
            upstream,
            "public-model",
            "upstream-model",
            fixture(directory, "client-request.json"),
        )
        .expect("accepted stream request must be bridgeable");
        assert_json_eq(
            upstream_request.body(),
            &fixture(directory, "expected-upstream-request.json"),
        );
        let mut renderer = plan.stream_renderer();
        let mut actual = Vec::new();
        for event in decode(&fixture(directory, "upstream-stream.sse")) {
            actual.extend(renderer.render(event).expect("event must render"));
        }
        actual.extend(renderer.finish().expect("stream must finish"));
        assert_sse_semantics(
            downstream,
            &actual,
            &fixture(directory, "expected-client-stream.sse"),
        );
    }
}

#[test]
fn chat_to_responses_ignores_reasoning_only_empty_content_before_tool_terminal() {
    let (plan, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","input":"hello","stream":true,"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}"#,
        ),
        ReasoningOutput::PlainText,
    )
        .expect("Responses request should be bridgeable without reasoning input");
    let upstream = Bytes::from_static(
        br#"data: {"id":"chatcmpl_reasoning","choices":[{"delta":{"role":"assistant","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning","choices":[{"delta":{"reasoning_content":"check args","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning","choices":[{"delta":{"reasoning_content":"call tool","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_lookup","function":{"name":"lookup","arguments":""}}]},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning","choices":[{"delta":{"content":"","tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Hangzhou\"}"}}]},"finish_reason":"tool_calls","index":0}]}

data: [DONE]

"#,
    );
    let mut renderer = plan.stream_renderer();
    let mut actual = Vec::new();
    for event in decode(&upstream) {
        actual.extend(
            renderer
                .render(event)
                .expect("empty reasoning content must not abort tool stream"),
        );
    }
    actual.extend(renderer.finish().expect("tool stream must reach terminal"));

    assert!(!String::from_utf8_lossy(&actual).contains("response.output_text"));
    assert_sse_semantics(
        ApiProtocol::Responses,
        &actual,
        br#"data: {"type":"response.created","response":{"id":"resp_reasoning","model":"public-model","object":"response","output":[],"status":"in_progress"}}

data: {"type":"response.output_item.added","output_index":0,"item":{"content":[],"id":"rs_reasoning","status":"in_progress","summary":[],"type":"reasoning"}}

data: {"type":"response.reasoning_text.delta","content_index":0,"delta":"check args","item_id":"rs_reasoning","output_index":0,"type":"response.reasoning_text.delta"}

data: {"type":"response.reasoning_text.delta","content_index":0,"delta":"call tool","item_id":"rs_reasoning","output_index":0,"type":"response.reasoning_text.delta"}

data: {"type":"response.reasoning_text.done","content_index":0,"item_id":"rs_reasoning","output_index":0,"text":"check argscall tool","type":"response.reasoning_text.done"}

data: {"type":"response.output_item.done","output_index":0,"item":{"content":[{"text":"check argscall tool","type":"reasoning_text"}],"id":"rs_reasoning","status":"completed","summary":[],"type":"reasoning"}}

data: {"type":"response.output_item.added","output_index":1,"item":{"arguments":"","call_id":"call_lookup","id":"fc_lookup","name":"lookup","status":"in_progress","type":"function_call"}}

data: {"type":"response.function_call_arguments.delta","output_index":1,"item_id":"fc_lookup","delta":"{\"city\":\"Hangzhou\"}"}

data: {"type":"response.function_call_arguments.done","output_index":1,"item_id":"fc_lookup","arguments":"{\"city\":\"Hangzhou\"}"}

data: {"type":"response.output_item.done","output_index":1,"item":{"arguments":"{\"city\":\"Hangzhou\"}","call_id":"call_lookup","id":"fc_lookup","name":"lookup","status":"completed","type":"function_call"}}

data: {"type":"response.completed","response":{"id":"resp_reasoning","model":"public-model","object":"response","output":[{"content":[{"text":"check argscall tool","type":"reasoning_text"}],"id":"rs_reasoning","status":"completed","summary":[],"type":"reasoning"},{"arguments":"{\"city\":\"Hangzhou\"}","call_id":"call_lookup","id":"fc_lookup","name":"lookup","status":"completed","type":"function_call"}],"status":"completed"}}

"#,
    );
}

#[test]
fn chat_to_responses_accepts_one_post_finish_usage_chunk_before_done() {
    let (plan, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(br#"{"model":"public-model","input":"hello","stream":true}"#),
        ReasoningOutput::PlainText,
    )
    .expect("Responses request should be bridgeable");
    let upstream = Bytes::from_static(
        br#"data: {"id":"chatcmpl_usage","choices":[{"delta":{"role":"assistant","content":""},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_usage","choices":[{"delta":{"reasoning_content":"check"},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_usage","choices":[{"delta":{"content":"ok"},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_usage","choices":[{"delta":{"content":""},"finish_reason":"stop","index":0}]}

data: {"id":"chatcmpl_usage","choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}

data: [DONE]

"#,
    );

    // Render the observed Chat lifecycle, including the post-finish usage-only statistics chunk.
    let mut renderer = plan.stream_renderer();
    let mut actual = Vec::new();
    for event in decode(&upstream) {
        actual.extend(
            renderer
                .render(event)
                .expect("one post-finish usage chunk must be accepted"),
        );
    }
    actual.extend(renderer.finish().expect("stream must reach terminal"));

    // Validate that ignored statistics do not disturb reasoning, visible text, or the single Responses terminal.
    let mut state = ResponsesStreamState::new();
    for event in decode(&actual) {
        state.ingest(&event).expect("converted Responses stream");
    }
    state.finish().expect("converted Responses terminal");
    assert_eq!(state.reasoning_text(), "check");
    assert_eq!(state.text(), "ok");
    assert_eq!(
        state.terminal(),
        Some(openbridge::bridge::StreamTerminal::Completed)
    );
}

#[test]
fn chat_to_responses_rejects_non_success_finish_and_late_chunks() {
    let (plan, _) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(br#"{"model":"public-model","input":"hello","stream":true}"#),
    )
    .expect("Responses request should be bridgeable");

    // A Chat finish reason other than stop or tool_calls must not be fabricated as response.completed.
    let mut renderer = plan.stream_renderer();
    let events = decode(
        br#"data: {"id":"chatcmpl_length","choices":[{"delta":{"content":"partial"},"finish_reason":"length","index":0}]}

"#,
    );
    assert!(renderer.render(events[0].clone()).is_err());

    // A normal chunk after the Chat finish reason must not mutate the completed Responses lifecycle.
    let mut renderer = plan.stream_renderer();
    let events = decode(
        br#"data: {"id":"chatcmpl_late","choices":[{"delta":{"content":"done"},"finish_reason":"stop","index":0}]}

data: {"id":"chatcmpl_late","choices":[{"delta":{"content":"late"},"finish_reason":null,"index":0}]}

"#,
    );
    renderer
        .render(events[0].clone())
        .expect("first Chat finish chunk should render");
    assert!(renderer.render(events[1].clone()).is_err());

    // Only one post-finish usage-only chunk is valid; a duplicate remains an unexpected late event.
    let mut renderer = plan.stream_renderer();
    let events = decode(
        br#"data: {"id":"chatcmpl_duplicate_usage","choices":[{"delta":{"content":"done"},"finish_reason":"stop","index":0}]}

data: {"id":"chatcmpl_duplicate_usage","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

data: {"id":"chatcmpl_duplicate_usage","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

"#,
    );
    renderer
        .render(events[0].clone())
        .expect("finish chunk should render");
    renderer
        .render(events[1].clone())
        .expect("first usage-only chunk should be accepted");
    assert!(renderer.render(events[2].clone()).is_err());
}

#[test]
fn reasoning_request_and_non_stream_response_keep_a_separate_channel() {
    // Verify that standard Responses reasoning configuration and history items convert to Chat.
    let responses_request = serde_json::json!({
        "model": "public-model",
        "reasoning": {"effort": "high", "summary": "auto"},
        "input": [
            {"content": [{"text": "lookup weather", "type": "input_text"}], "role": "user", "type": "message"},
            {"content": [{"text": "decide lookup", "type": "reasoning_text"}], "id": "rs_previous", "status": "completed", "summary": [], "type": "reasoning"},
            {"arguments": "{\"city\":\"Hangzhou\"}", "call_id": "call_lookup", "id": "fc_previous", "name": "lookup", "type": "function_call"}
        ],
        "tools": [{"name": "lookup", "parameters": {"type": "object"}, "type": "function"}]
    });
    let (responses_plan, chat_request) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&responses_request).unwrap()),
        ReasoningOutput::PlainText,
    )
    .expect("Responses reasoning request should convert to Chat");
    let chat_request: Value = serde_json::from_slice(chat_request.body()).unwrap();
    assert_eq!(chat_request["reasoning_effort"], "high");
    assert!(chat_request.get("reasoning").is_none());
    assert!(chat_request.get("summary").is_none());
    assert_eq!(
        chat_request["messages"][1]["reasoning_content"],
        "decide lookup"
    );
    assert_eq!(
        chat_request["messages"][1]["tool_calls"][0]["id"],
        "call_lookup"
    );

    // Verify that Chat reasoning_content still creates a separate Responses item in non-streaming tool responses.
    let responses = responses_plan
        .render_non_stream(Bytes::from_static(
            br#"{"id":"chatcmpl_reasoning_roundtrip","object":"chat.completion","model":"upstream-model","choices":[{"index":0,"message":{"role":"assistant","content":null,"reasoning_content":"decide lookup","tool_calls":[{"id":"call_lookup","type":"function","function":{"name":"lookup","arguments":"{\"city\":\"Hangzhou\"}"}}]},"finish_reason":"tool_calls"}]}"#,
        ))
        .expect("Chat reasoning response should convert to Responses");
    let responses: Value = serde_json::from_slice(&responses).unwrap();
    assert_eq!(responses["output"][0]["type"], "reasoning");
    assert_eq!(
        responses["output"][0]["content"][0]["text"],
        "decide lookup"
    );
    assert_eq!(responses["output"][0]["summary"], serde_json::json!([]));
    assert_eq!(responses["output"][1]["type"], "function_call");
    assert_eq!(
        responses["output"][1]["arguments"],
        "{\"city\":\"Hangzhou\"}"
    );
    assert!(
        responses["output"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["type"] != "message")
    );

    // Verify that standard Chat reasoning_effort and assistant reasoning_content enter Responses.
    let chat_request = serde_json::json!({
        "model": "public-model",
        "reasoning_effort": "high",
        "messages": [
            {"role": "user", "content": "lookup weather"},
            {"content": null, "reasoning_content": "decide lookup", "role": "assistant", "tool_calls": [{"id": "call_lookup", "type": "function", "function": {"name": "lookup", "arguments": "{\"city\":\"Hangzhou\"}"}}]},
            {"role": "tool", "tool_call_id": "call_lookup", "content": "{\"temperature_c\":25}"}
        ],
        "tools": [{"type": "function", "function": {"name": "lookup", "parameters": {"type": "object"}}}]
    });
    let (_, responses_request) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&chat_request).unwrap()),
        ReasoningOutput::PlainText,
    )
    .expect("Chat reasoning request should convert to Responses");
    let responses_request: Value = serde_json::from_slice(responses_request.body()).unwrap();
    assert_eq!(responses_request["reasoning"]["effort"], "high");
    assert_eq!(responses_request["input"][1]["type"], "reasoning");
    assert_eq!(
        responses_request["input"][1]["content"][0]["text"],
        "decide lookup"
    );
    assert_eq!(responses_request["input"][2]["type"], "function_call");
    assert_eq!(
        responses_request["input"][3]["type"],
        "function_call_output"
    );
}

#[test]
fn responses_message_shorthand_preserves_a_tool_result_round_trip() {
    // Build a standard stateless Responses continuation whose message items omit the optional type discriminator.
    let responses_request = serde_json::json!({
        "model": "public-model",
        "input": [
            {"role": "user", "content": "look up the synthetic value"},
            {"type": "function_call", "id": "fc_lookup", "call_id": "call_lookup", "name": "lookup", "arguments": "{\"key\":\"value\"}"},
            {"type": "function_call_output", "call_id": "call_lookup", "output": "{\"value\":42}"},
            {"role": "user", "content": "return DONE"}
        ],
        "tools": [{"name": "lookup", "parameters": {"type": "object"}, "type": "function"}],
        "tool_choice": "none"
    });

    // Convert the shorthand through the same ledger used by explicit message items.
    let (_, chat_request) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&responses_request).unwrap()),
    )
    .expect("Responses message shorthand should convert to Chat");
    let chat_request: Value = serde_json::from_slice(chat_request.body()).unwrap();
    assert_eq!(chat_request["messages"][0]["role"], "user");
    assert_eq!(
        chat_request["messages"][1]["tool_calls"][0]["id"],
        "call_lookup"
    );
    assert_eq!(chat_request["messages"][2]["tool_call_id"], "call_lookup");
    assert_eq!(chat_request["messages"][3]["content"], "return DONE");

    // Keep missing discriminators closed when the object is not exactly the standard shorthand.
    let ambiguous = Bytes::from_static(
        br#"{"model":"public-model","input":[{"role":"user","content":"hello","future":true}]}"#,
    );
    assert!(
        BridgePlan::prepare(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            ambiguous,
        )
        .is_err()
    );
}

#[test]
fn chat_reasoning_stream_offsets_visible_message_output_index() {
    let (plan, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(br#"{"model":"public-model","input":"hello","stream":true}"#),
        ReasoningOutput::PlainText,
    )
    .expect("Responses request should be bridgeable");
    let upstream = Bytes::from_static(
        br#"data: {"id":"chatcmpl_reasoning_text","choices":[{"delta":{"role":"assistant","reasoning_content":"decide"},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning_text","choices":[{"delta":{"content":"answer"},"finish_reason":null,"index":0}]}

data: {"id":"chatcmpl_reasoning_text","choices":[{"delta":{},"finish_reason":"stop","index":0}]}

data: [DONE]

"#,
    );
    let mut renderer = plan.stream_renderer();
    let mut actual = Vec::new();
    for event in decode(&upstream) {
        actual.extend(
            renderer
                .render(event)
                .expect("reasoning text stream should render"),
        );
    }
    actual.extend(
        renderer
            .finish()
            .expect("reasoning text stream should finish"),
    );

    assert_sse_semantics(
        ApiProtocol::Responses,
        &actual,
        br#"data: {"type":"response.created","response":{"id":"resp_reasoning_text","model":"public-model","object":"response","output":[],"status":"in_progress"}}

data: {"type":"response.output_item.added","output_index":0,"item":{"content":[],"id":"rs_reasoning_text","status":"in_progress","summary":[],"type":"reasoning"}}

data: {"type":"response.reasoning_text.delta","content_index":0,"delta":"decide","item_id":"rs_reasoning_text","output_index":0,"type":"response.reasoning_text.delta"}

data: {"type":"response.reasoning_text.done","content_index":0,"item_id":"rs_reasoning_text","output_index":0,"text":"decide","type":"response.reasoning_text.done"}

data: {"type":"response.output_item.done","output_index":0,"item":{"content":[{"text":"decide","type":"reasoning_text"}],"id":"rs_reasoning_text","status":"completed","summary":[],"type":"reasoning"}}

data: {"type":"response.output_item.added","output_index":1,"item":{"content":[],"id":"msg_reasoning_text","role":"assistant","status":"in_progress","type":"message"}}

data: {"type":"response.output_text.delta","content_index":0,"delta":"answer","item_id":"msg_reasoning_text","output_index":1,"type":"response.output_text.delta"}

data: {"type":"response.output_item.done","output_index":1,"item":{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_reasoning_text","role":"assistant","status":"completed","type":"message"}}

data: {"type":"response.completed","response":{"id":"resp_reasoning_text","model":"public-model","object":"response","output":[{"content":[{"text":"decide","type":"reasoning_text"}],"id":"rs_reasoning_text","status":"completed","summary":[],"type":"reasoning"},{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_reasoning_text","role":"assistant","status":"completed","type":"message"}],"status":"completed"}}

"#,
    );
}

#[test]
fn responses_reasoning_summary_stream_maps_to_chat_reasoning_channel() {
    let (plan, request) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"high","stream":true}"#,
        ),
        ReasoningOutput::Summary,
    )
        .expect("Chat request should be bridgeable");
    let request: Value = serde_json::from_slice(request.body()).unwrap();
    assert_eq!(
        request["reasoning"],
        serde_json::json!({"effort": "high", "summary": "auto"})
    );
    let upstream = Bytes::from_static(
        br#"data: {"type":"response.created","response":{"id":"resp_summary","model":"upstream-model","object":"response","output":[],"status":"in_progress"}}

data: {"type":"response.output_item.added","output_index":0,"item":{"content":[],"encrypted_content":null,"id":"rs_summary","status":"in_progress","summary":[],"type":"reasoning"}}

data: {"type":"response.reasoning_summary_part.added","item_id":"rs_summary","output_index":0,"part":{"text":"","type":"summary_text"},"type":"response.reasoning_summary_part.added"}

data: {"type":"response.reasoning_summary_text.delta","content_index":0,"delta":"decide ","item_id":"rs_summary","output_index":0,"type":"response.reasoning_summary_text.delta"}

data: {"type":"response.reasoning_summary_text.delta","content_index":0,"delta":"tool","item_id":"rs_summary","output_index":0,"type":"response.reasoning_summary_text.delta"}

data: {"type":"response.reasoning_summary_text.done","content_index":0,"item_id":"rs_summary","output_index":0,"text":"decide tool","type":"response.reasoning_summary_text.done"}

data: {"type":"response.output_item.done","output_index":0,"item":{"content":[],"encrypted_content":"opaque-continuation","id":"rs_summary","status":"completed","summary":[{"text":"decide tool","type":"summary_text"}],"type":"reasoning"}}

data: {"type":"response.output_item.added","output_index":1,"item":{"content":[],"id":"msg_summary","role":"assistant","status":"in_progress","type":"message"}}

data: {"type":"response.output_text.delta","content_index":0,"delta":"answer","item_id":"msg_summary","output_index":1,"type":"response.output_text.delta"}

data: {"type":"response.output_item.done","output_index":1,"item":{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_summary","role":"assistant","status":"completed","type":"message"}}

data: {"type":"response.completed","response":{"id":"resp_summary","model":"upstream-model","object":"response","output":[{"content":[],"encrypted_content":"opaque-continuation","id":"rs_summary","status":"completed","summary":[{"text":"decide tool","type":"summary_text"}],"type":"reasoning"},{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_summary","role":"assistant","status":"completed","type":"message"}],"status":"completed"}}

"#,
    );
    let mut renderer = plan.stream_renderer();
    let mut actual = Vec::new();
    for event in decode(&upstream) {
        actual.extend(
            renderer
                .render(event)
                .expect("reasoning summary stream should render"),
        );
    }
    actual.extend(
        renderer
            .finish()
            .expect("reasoning summary stream should finish"),
    );

    assert_sse_semantics(
        ApiProtocol::ChatCompletions,
        &actual,
        br#"data: {"choices":[{"delta":{"reasoning_content":"decide ","role":"assistant"},"finish_reason":null,"index":0}],"id":"chatcmpl_summary","model":"public-model","object":"chat.completion.chunk"}

data: {"choices":[{"delta":{"reasoning_content":"tool"},"finish_reason":null,"index":0}],"id":"chatcmpl_summary","model":"public-model","object":"chat.completion.chunk"}

data: {"choices":[{"delta":{"content":"answer"},"finish_reason":null,"index":0}],"id":"chatcmpl_summary","model":"public-model","object":"chat.completion.chunk"}

data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}],"id":"chatcmpl_summary","model":"public-model","object":"chat.completion.chunk"}

data: [DONE]

"#,
    );
}

#[test]
fn bridge_rejects_opaque_inputs_or_unsupported_reasoning() {
    // Provider reasoning must not be silently discarded when reasoning capability is undeclared.
    assert!(BridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"high"}"#,
        ),
    )
        .is_err());

    // Responses accepts only standard reasoning.effort, not Chat's top-level reasoning_effort alias.
    assert!(
        BridgePlan::prepare_with_reasoning_output(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            Bytes::from_static(
                br#"{"model":"public-model","input":"hello","reasoning_effort":"high"}"#,
            ),
            ReasoningOutput::PlainText,
        )
        .is_err()
    );

    // Chat accepts only standard reasoning_effort, not the Responses reasoning object.
    assert!(BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning":{"effort":"high"}}"#,
        ),
        ReasoningOutput::PlainText,
    )
        .is_err());

    // The Bridge consumes the supported summary hints while preserving reasoning effort.
    for summary in [serde_json::json!(false), serde_json::json!("auto")] {
        let request = serde_json::json!({
            "model": "public-model",
            "input": "hello",
            "reasoning": {"effort": "high", "summary": summary}
        });
        let (_, request) = BridgePlan::prepare_with_reasoning_output(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            Bytes::from(serde_json::to_vec(&request).unwrap()),
            ReasoningOutput::PlainText,
        )
        .expect("auto and false summary hints should be consumed by the Chat bridge");
        let request: Value = serde_json::from_slice(request.body()).unwrap();
        assert_eq!(request["reasoning_effort"], "high");
        assert!(request.get("reasoning").is_none());
        assert!(request.get("summary").is_none());
    }

    // A non-standard boolean reasoning_effort shape must not be silently discarded by the Bridge.
    assert!(BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":false}"#,
        ),
        ReasoningOutput::Summary,
    )
        .is_err());

    // reasoning_content is valid only in assistant history and must not be skipped by stream shorthand.
    assert!(BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello","reasoning_content":"invalid"}],"stream":true}"#,
        ),
        ReasoningOutput::PlainText,
    )
        .is_err());

    // Encrypted Responses continuation is not convertible plaintext reasoning and must be rejected before egress.
    let opaque_request = Bytes::from_static(
        br#"{"model":"public-model","input":[{"encrypted_content":"opaque","id":"rs_opaque","status":"completed","summary":[],"type":"reasoning"}]}"#,
    );
    assert!(
        BridgePlan::prepare_with_reasoning_output(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            opaque_request,
            ReasoningOutput::PlainText,
        )
        .is_err()
    );
}

#[test]
fn responses_to_chat_non_stream_drops_completed_opaque_continuation() {
    // Build one response-side Bridge that declares readable Responses reasoning output.
    let (plan, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
        ),
        ReasoningOutput::Summary,
    )
    .unwrap();

    // Drop output-only opaque continuation while preserving the completed text message.
    let converted = plan
        .render_non_stream(Bytes::from_static(
            br#"{"id":"resp_opaque","model":"upstream-model","object":"response","output":[{"content":[],"encrypted_content":"synthetic-opaque","id":"rs_opaque","summary":[],"type":"reasoning"},{"content":[{"text":"answer","type":"output_text"}],"id":"msg_opaque","role":"assistant","status":"completed","type":"message"}],"status":"completed"}"#,
        ))
        .expect("output-only opaque continuation should not invalidate stateless Chat output");
    let converted: Value = serde_json::from_slice(&converted).unwrap();
    assert_eq!(converted["choices"][0]["message"]["content"], "answer");
    assert!(
        converted["choices"][0]["message"]
            .get("reasoning_content")
            .is_none()
    );
    let encoded = serde_json::to_string(&converted).unwrap();
    assert!(!encoded.contains("encrypted_content"));
    assert!(!encoded.contains("synthetic-opaque"));

    // Preserve readable summary text while discarding the separate opaque continuation value.
    let converted = plan
        .render_non_stream(Bytes::from_static(
            br#"{"id":"resp_summary","model":"upstream-model","object":"response","output":[{"content":[],"encrypted_content":"synthetic-opaque","id":"rs_summary","summary":[{"text":"decide","type":"summary_text"}],"type":"reasoning"},{"content":[{"text":"answer","type":"output_text"}],"id":"msg_summary","role":"assistant","status":"completed","type":"message"}],"status":"completed"}"#,
        ))
        .expect("readable summary should remain bridgeable");
    let converted: Value = serde_json::from_slice(&converted).unwrap();
    assert_eq!(
        converted["choices"][0]["message"]["reasoning_content"],
        "decide"
    );
    assert_eq!(converted["choices"][0]["message"]["content"], "answer");

    // Continue to reject malformed readable fields instead of treating them as absent.
    assert!(
        plan.render_non_stream(Bytes::from_static(
            br#"{"id":"resp_malformed","model":"upstream-model","object":"response","output":[{"content":{},"encrypted_content":"synthetic-opaque","id":"rs_malformed","summary":[],"type":"reasoning"},{"content":[{"text":"answer","type":"output_text"}],"id":"msg_malformed","role":"assistant","status":"completed","type":"message"}],"status":"completed"}"#,
        ))
        .is_err()
    );
}

#[test]
fn bridge_rejects_provider_bound_or_unmodeled_requests_before_egress() {
    let cases = [
        "responses_to_chat/responses_to_chat.continuation.reject",
        "responses_to_chat/responses_to_chat.unsupported_hosted_tool.reject",
        "responses_to_chat/responses_to_chat.duplicate_call_id.reject",
        "responses_to_chat/responses_to_chat.empty_arguments.reject",
        "chat_to_responses/chat_to_responses.unknown_tool_result.reject",
        "responses_to_chat/responses_to_chat.unknown_tool_result.reject",
    ];

    // Every reject case must fail before ApiRequest creation instead of relying on upstream rejection.
    for directory in cases {
        let request = fixture(directory, "client-request.json");
        let downstream = if directory.starts_with("responses_to_chat") {
            ApiProtocol::Responses
        } else {
            ApiProtocol::ChatCompletions
        };
        let upstream = if downstream == ApiProtocol::Responses {
            ApiProtocol::ChatCompletions
        } else {
            ApiProtocol::Responses
        };
        assert!(
            BridgePlan::prepare(
                downstream,
                upstream,
                "public-model",
                "upstream-model",
                request,
            )
            .is_err(),
            "{directory} must fail before egress"
        );
    }

    // Unmodeled ordinary top-level fields must not silently disappear during conversion.
    assert!(
        BridgePlan::prepare(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            Bytes::from_static(br#"{"model":"public-model","input":"hello","seed":7}"#,),
        )
        .is_err()
    );
}

#[test]
fn structured_outputs_convert_between_chat_and_responses_request_shapes() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": false
    });
    let chat_request = serde_json::json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "return JSON"}],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "answer",
                "description": "A short answer",
                "schema": schema,
                "strict": true
            }
        }
    });
    let (_, responses_request) = BridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&chat_request).unwrap()),
    )
    .expect("Chat structured output should convert to Responses");
    let responses_request: Value = serde_json::from_slice(responses_request.body()).unwrap();
    assert_eq!(
        responses_request["text"]["format"],
        serde_json::json!({
            "type": "json_schema",
            "name": "answer",
            "description": "A short answer",
            "schema": schema,
            "strict": true
        })
    );

    let responses_request = serde_json::json!({
        "model": "public-model",
        "input": "return JSON",
        "text": {
            "format": {
                "type": "json_schema",
                "name": "answer",
                "schema": schema,
                "strict": true
            }
        }
    });
    let (_, chat_request) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&responses_request).unwrap()),
    )
    .expect("Responses structured output should convert to Chat");
    let chat_request: Value = serde_json::from_slice(chat_request.body()).unwrap();
    assert_eq!(
        chat_request["response_format"],
        serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "answer",
                "schema": schema,
                "strict": true
            }
        })
    );

    // Unknown format fields must not be silently discarded during the conversion.
    let unsupported = serde_json::json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "return JSON"}],
        "response_format": {"type": "json_object", "future": true}
    });
    assert!(
        BridgePlan::prepare(
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "public-model",
            "upstream-model",
            Bytes::from(serde_json::to_vec(&unsupported).unwrap()),
        )
        .is_err()
    );
}

#[test]
fn incomplete_stream_arguments_fail_without_a_fabricated_terminal() {
    let directory = "responses_to_chat/responses_to_chat.incomplete_arguments.stream";
    let (plan, _) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        fixture(directory, "client-request.json"),
    )
    .expect("request shape itself is bridgeable");
    let mut renderer = plan.stream_renderer();
    let mut failed = false;
    for event in decode(&fixture(directory, "upstream-stream.sse")) {
        if renderer.render(event).is_err() {
            failed = true;
            break;
        }
    }
    assert!(failed || renderer.finish().is_err());
}

#[test]
fn named_function_tool_choice_is_converted_in_both_directions() {
    let (_, responses_request) = BridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}],"tool_choice":{"type":"function","function":{"name":"lookup"}}}"#,
        ),
    )
        .unwrap();
    let responses: Value = serde_json::from_slice(responses_request.body()).unwrap();
    assert_eq!(
        responses["tool_choice"],
        serde_json::json!({"name": "lookup", "type": "function"})
    );

    let (_, chat_request) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","input":"hi","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],"tool_choice":{"type":"function","name":"lookup"}}"#,
        ),
    )
        .unwrap();
    let chat: Value = serde_json::from_slice(chat_request.body()).unwrap();
    assert_eq!(
        chat["tool_choice"],
        serde_json::json!({"function": {"name": "lookup"}, "type": "function"})
    );
}

#[test]
fn chat_to_responses_promotes_only_the_first_plain_instruction_message() {
    let source = serde_json::json!({
        "model": "public-model",
        "messages": [
            {"role": "developer", "content": "client instruction"},
            {"role": "user", "content": "hello"},
            {"role": "system", "content": "later transcript guidance"}
        ]
    });

    let (_, request) = BridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&source).unwrap()),
    )
    .expect("the first developer message must be bridgeable as instructions");
    let request: Value = serde_json::from_slice(request.body()).unwrap();

    assert_eq!(request["instructions"], "client instruction");
    assert_eq!(request["store"], false);
    assert_eq!(request["input"].as_array().unwrap().len(), 2);
    assert_eq!(request["input"][0]["role"], "user");
    assert_eq!(request["input"][1]["role"], "system");
    assert!(
        !request["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["content"].to_string().contains("client instruction"))
    );
}

#[test]
fn chat_to_responses_forwards_instruction_only_requests_without_fabricating_input() {
    for role in ["system", "developer"] {
        let source = serde_json::json!({
            "model": "public-model",
            "messages": [{"role": role, "content": "instruction only"}]
        });
        let (_, request) = BridgePlan::prepare(
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "public-model",
            "upstream-model",
            Bytes::from(serde_json::to_vec(&source).unwrap()),
        )
        .expect("an instruction-only Chat request must remain forwardable");
        let request: Value = serde_json::from_slice(request.body()).unwrap();

        assert_eq!(request["instructions"], "instruction only");
        assert_eq!(request["input"], serde_json::json!([]));
        assert_eq!(request["store"], false);
    }
}

#[test]
fn chat_to_responses_does_not_promote_a_leading_user_message() {
    let source = serde_json::json!({
        "model": "public-model",
        "messages": [
            {"role": "user", "content": "hello"},
            {"role": "system", "content": "later transcript guidance"}
        ]
    });
    let (_, request) = BridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&source).unwrap()),
    )
    .unwrap();
    let request: Value = serde_json::from_slice(request.body()).unwrap();

    assert!(request.get("instructions").is_none());
    assert_eq!(request["input"][0]["role"], "user");
    assert_eq!(request["input"][1]["role"], "system");
    assert_eq!(request["store"], false);
}

#[test]
fn responses_to_chat_prepends_instructions_once_and_preserves_input_order() {
    let source = serde_json::json!({
        "model": "public-model",
        "instructions": "client instruction",
        "input": [
            {"type": "message", "role": "user", "content": "hello"},
            {"type": "message", "role": "developer", "content": "later transcript guidance"}
        ],
        "store": false
    });

    let (_, request) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from(serde_json::to_vec(&source).unwrap()),
    )
    .expect("Responses instructions must be bridgeable to Chat");
    let request: Value = serde_json::from_slice(request.body()).unwrap();

    assert_eq!(
        request["messages"],
        serde_json::json!([
            {"role": "system", "content": "client instruction"},
            {"role": "user", "content": "hello"},
            {"role": "developer", "content": "later transcript guidance"}
        ])
    );
    assert!(request.get("store").is_none());
}

#[test]
fn responses_bridge_rejects_invalid_instruction_and_store_shapes() {
    for instructions in [
        Value::Null,
        Value::String(String::new()),
        Value::String("   ".to_owned()),
        serde_json::json!([]),
        serde_json::json!({}),
    ] {
        let body = serde_json::json!({
            "model": "public-model",
            "input": "hello",
            "instructions": instructions,
            "store": false
        });
        assert!(
            BridgePlan::prepare(
                ApiProtocol::Responses,
                ApiProtocol::ChatCompletions,
                "public-model",
                "upstream-model",
                Bytes::from(serde_json::to_vec(&body).unwrap()),
            )
            .is_err()
        );
    }

    for store in [
        Value::Null,
        Value::Bool(true),
        Value::String("false".to_owned()),
    ] {
        let body = serde_json::json!({
            "model": "public-model",
            "input": "hello",
            "instructions": "client instruction",
            "store": store
        });
        assert!(
            BridgePlan::prepare(
                ApiProtocol::Responses,
                ApiProtocol::ChatCompletions,
                "public-model",
                "upstream-model",
                Bytes::from(serde_json::to_vec(&body).unwrap()),
            )
            .is_err()
        );
    }
}
