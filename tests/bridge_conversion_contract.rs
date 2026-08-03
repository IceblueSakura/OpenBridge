//! 使用 accepted canonical artifacts 约束双向 Protocol Bridge 的转换结果。
//!
//! 这些测试直接验证 BridgePlan 的请求、非流式响应和流式 renderer；生产 Router 接入由
//! forwarding contract 另行约束。

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

    // 对每个 accepted case 验证请求与响应均使用同一 BridgePlan 双向闭合。
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

    // 逐 event 渲染并在 EOF 完成状态校验，避免网络 chunk 边界影响逻辑结果。
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
fn chat_to_responses_rejects_non_success_finish_and_late_chunks() {
    let (plan, _) = BridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(br#"{"model":"public-model","input":"hello","stream":true}"#),
    )
    .expect("Responses request should be bridgeable");

    // 非 stop/tool_calls 的 Chat finish reason 不能被伪造成 response.completed。
    let mut renderer = plan.stream_renderer();
    let events = decode(
        br#"data: {"id":"chatcmpl_length","choices":[{"delta":{"content":"partial"},"finish_reason":"length","index":0}]}

"#,
    );
    assert!(renderer.render(events[0].clone()).is_err());

    // Chat finish reason 后的普通 chunk 不能继续污染已结束的 Responses 生命周期。
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
}

#[test]
fn reasoning_request_and_non_stream_response_keep_a_separate_channel() {
    // 验证标准 Responses reasoning 配置和历史 reasoning item 可转换到 Chat。
    let responses_request = serde_json::json!({
        "model": "public-model",
        "reasoning": {"effort": "high"},
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
    assert_eq!(
        chat_request["messages"][1]["reasoning_content"],
        "decide lookup"
    );
    assert_eq!(
        chat_request["messages"][1]["tool_calls"][0]["id"],
        "call_lookup"
    );

    // 验证 Chat reasoning_content 在非流式 tool response 中仍生成独立 Responses item。
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

    // 验证 Chat 标准 reasoning_effort 和 assistant reasoning_content 可进入 Responses。
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
    let (plan, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":true}"#,
        ),
        ReasoningOutput::Summary,
    )
    .expect("Chat request should be bridgeable");
    let upstream = Bytes::from_static(
        br#"data: {"type":"response.created","response":{"id":"resp_summary","model":"upstream-model","object":"response","output":[],"status":"in_progress"}}

data: {"type":"response.output_item.added","output_index":0,"item":{"content":[],"id":"rs_summary","status":"in_progress","summary":[],"type":"reasoning"}}

data: {"type":"response.reasoning_summary_part.added","item_id":"rs_summary","output_index":0,"part":{"text":"","type":"summary_text"},"type":"response.reasoning_summary_part.added"}

data: {"type":"response.reasoning_summary_text.delta","content_index":0,"delta":"decide ","item_id":"rs_summary","output_index":0,"type":"response.reasoning_summary_text.delta"}

data: {"type":"response.reasoning_summary_text.delta","content_index":0,"delta":"tool","item_id":"rs_summary","output_index":0,"type":"response.reasoning_summary_text.delta"}

data: {"type":"response.reasoning_summary_text.done","content_index":0,"item_id":"rs_summary","output_index":0,"text":"decide tool","type":"response.reasoning_summary_text.done"}

data: {"type":"response.output_item.done","output_index":0,"item":{"content":[],"id":"rs_summary","status":"completed","summary":[{"text":"decide tool","type":"summary_text"}],"type":"reasoning"}}

data: {"type":"response.output_item.added","output_index":1,"item":{"content":[],"id":"msg_summary","role":"assistant","status":"in_progress","type":"message"}}

data: {"type":"response.output_text.delta","content_index":0,"delta":"answer","item_id":"msg_summary","output_index":1,"type":"response.output_text.delta"}

data: {"type":"response.output_item.done","output_index":1,"item":{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_summary","role":"assistant","status":"completed","type":"message"}}

data: {"type":"response.completed","response":{"id":"resp_summary","model":"upstream-model","object":"response","output":[{"content":[],"id":"rs_summary","status":"completed","summary":[{"text":"decide tool","type":"summary_text"}],"type":"reasoning"},{"content":[{"annotations":[],"text":"answer","type":"output_text"}],"id":"msg_summary","role":"assistant","status":"completed","type":"message"}],"status":"completed"}}

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
fn bridge_rejects_opaque_or_unsupported_reasoning() {
    // 未声明 reasoning 能力时，不允许将 provider reasoning 静默丢弃。
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

    // Responses 只接受标准 reasoning.effort，不接受 Chat 的顶层 reasoning_effort 别名。
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

    // Chat 只接受标准 reasoning_effort，不接受 Responses 的 reasoning 对象。
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

    // Responses reasoning 的未建模子字段不能被静默丢弃。
    assert!(BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","input":"hello","reasoning":{"effort":"high","summary":false}}"#,
        ),
        ReasoningOutput::PlainText,
    )
    .is_err());

    // reasoning_effort 的非标准布尔形状不能在 Bridge 中静默丢弃。
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

    // reasoning_content 只能出现在 assistant history，不能因 stream 简写而被跳过。
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

    // Responses encrypted continuation 不是可转换的明文 reasoning，必须在 egress 前拒绝。
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

    // 非流式响应同样不能把 opaque reasoning 降级为 Chat reasoning_content。
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
    assert!(plan
        .render_non_stream(Bytes::from_static(
            br#"{"id":"resp_opaque","model":"upstream-model","object":"response","output":[{"encrypted_content":"opaque","id":"rs_opaque","status":"completed","summary":[],"type":"reasoning"}],"status":"completed"}"#,
        ))
        .is_err());
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

    // 所有拒绝样本必须在生成 ApiRequest 前失败，不能依赖上游碰运气拒绝。
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

    // 未建模的普通顶层字段也不能在转换时静默消失。
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
