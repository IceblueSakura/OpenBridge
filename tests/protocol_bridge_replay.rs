//! 使用 canonical corpus 回放 Protocol Bridge 流状态机。
//!
//! 这些测试只读取固定 fixture，不启动真实 Provider，也不把 bridge route 接入生产 ingress。

use openbridge::{
    bridge::{BridgeStreamError, ChatStreamState, ResponsesStreamState, StreamTerminal},
    transport::sse::SseDecoder,
};

fn replay_chat_fixture(document: &[u8]) -> Result<ChatStreamState, BridgeStreamError> {
    // 解码完整 fixture，并按 wire 顺序驱动 Chat 状态机。
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut state = ChatStreamState::new();
    for event in decoder.push(document).expect("fixture SSE must decode") {
        state.ingest(&event)?;
    }
    for event in decoder.finish().expect("fixture SSE must finish") {
        state.ingest(&event)?;
    }

    // 显式结束输入，确保 EOF 不能替代协议 terminal。
    state.finish()?;
    Ok(state)
}

fn replay_responses_fixture(document: &[u8]) -> Result<ResponsesStreamState, BridgeStreamError> {
    // 解码完整 fixture，并按 wire 顺序驱动 Responses 状态机。
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut state = ResponsesStreamState::new();
    for event in decoder.push(document).expect("fixture SSE must decode") {
        state.ingest(&event)?;
    }
    for event in decoder.finish().expect("fixture SSE must finish") {
        state.ingest(&event)?;
    }

    // 显式结束输入，确保 terminal 和所有 tool arguments 均已闭合。
    state.finish()?;
    Ok(state)
}

#[test]
fn responses_stream_replay_preserves_parallel_tool_identity_and_arguments() {
    let state = replay_responses_fixture(include_bytes!(
        "../testdata/cases/bridge/chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments/upstream-stream.sse"
    ))
    .expect("canonical Responses fixture must complete");

    // 验证 output item、call 与 arguments 三类 identity 未被混用。
    assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
    assert_eq!(state.tool_calls().len(), 2);
    assert_eq!(state.tool_calls()[0].item_id(), Some("fc_weather_p2"));
    assert_eq!(state.tool_calls()[0].call_id(), "call_weather_p2");
    assert_eq!(state.tool_calls()[0].name(), "get_weather");
    assert_eq!(state.tool_calls()[0].arguments(), r#"{"city":"上海"}"#);
    assert_eq!(state.tool_calls()[1].item_id(), Some("fc_time_p2"));
    assert_eq!(state.tool_calls()[1].call_id(), "call_time_p2");
    assert_eq!(state.tool_calls()[1].name(), "get_time");
    assert_eq!(
        state.tool_calls()[1].arguments(),
        r#"{"zone":"Asia/Shanghai"}"#
    );
}

#[test]
fn chat_stream_replay_preserves_parallel_tool_identity_and_arguments() {
    let state = replay_chat_fixture(include_bytes!(
        "../testdata/cases/bridge/responses_to_chat/responses_to_chat.parallel_tools.fragmented_arguments/upstream-stream.sse"
    ))
    .expect("canonical Chat fixture must complete");

    // 验证 Chat index 只负责分片关联，不会替代稳定 call id。
    assert_eq!(state.terminal(), Some(StreamTerminal::Completed));
    assert_eq!(state.tool_calls().len(), 2);
    assert_eq!(state.tool_calls()[0].call_id(), "call_weather_p1");
    assert_eq!(state.tool_calls()[0].name(), "get_weather");
    assert_eq!(state.tool_calls()[0].arguments(), r#"{"city":"上海"}"#);
    assert_eq!(state.tool_calls()[1].call_id(), "call_time_p1");
    assert_eq!(state.tool_calls()[1].name(), "get_time");
    assert_eq!(
        state.tool_calls()[1].arguments(),
        r#"{"zone":"Asia/Shanghai"}"#
    );
}

#[test]
fn stream_replay_rejects_incomplete_tool_arguments_at_terminal() {
    let error = replay_chat_fixture(include_bytes!(
        "../testdata/cases/bridge/responses_to_chat/responses_to_chat.incomplete_arguments.stream/upstream-stream.sse"
    ))
    .expect_err("incomplete arguments must fail closed");

    assert_eq!(error, BridgeStreamError::InvalidToolArguments);
}

#[test]
fn text_stream_replay_requires_one_explicit_terminal() {
    let responses = replay_responses_fixture(include_bytes!(
        "../testdata/cases/bridge/chat_to_responses/chat_to_responses.text.stream/upstream-stream.sse"
    ))
    .expect("Responses text fixture must complete");
    let chat = replay_chat_fixture(include_bytes!(
        "../testdata/cases/bridge/responses_to_chat/responses_to_chat.text.stream/upstream-stream.sse"
    ))
    .expect("Chat text fixture must complete");

    assert_eq!(responses.text(), "你好");
    assert_eq!(chat.text(), "你好");
    assert_eq!(responses.terminal(), Some(StreamTerminal::Completed));
    assert_eq!(chat.terminal(), Some(StreamTerminal::Completed));
}
