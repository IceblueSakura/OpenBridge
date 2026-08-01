//! 使用 accepted canonical artifacts 约束双向 Protocol Bridge 的转换结果。
//!
//! 这些测试直接验证 BridgePlan 的请求、非流式响应和流式 renderer；生产 Router 接入由
//! forwarding contract 另行约束。

use bytes::Bytes;
use openbridge::{
    bridge::{BridgePlan, ChatStreamState, ResponsesStreamState},
    core::ApiProtocol,
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
