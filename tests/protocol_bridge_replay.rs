//! 使用 canonical corpus 回放 Protocol Bridge 流状态机。
//!
//! 这些测试只读取固定 fixture，不启动真实 Provider，也不把 bridge route 接入生产 ingress。

use openbridge::{
    bridge::{BridgeStreamError, ChatStreamState, ResponsesStreamState, StreamTerminal},
    transport::sse::{SseDecoder, SseEvent},
};

/// 将 canonical SSE wire 解码为保持原始顺序的逻辑事件。
fn decode_fixture(document: &[u8]) -> Vec<SseEvent> {
    // 解码完整 fixture，并在 EOF 时派发已闭合的最后事件。
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(document).expect("fixture SSE must decode");
    events.extend(decoder.finish().expect("fixture SSE must finish"));
    events
}

/// 按 wire 顺序回放 Chat fixture，并显式验证 EOF 边界。
fn replay_chat_fixture(document: &[u8]) -> Result<ChatStreamState, BridgeStreamError> {
    // 解码完整 fixture，并按 wire 顺序驱动 Chat 状态机。
    let mut state = ChatStreamState::new();
    for event in decode_fixture(document) {
        state.ingest(&event)?;
    }

    // 显式结束输入，确保 EOF 不能替代协议 terminal。
    state.finish()?;
    Ok(state)
}

/// 按 wire 顺序回放 Responses fixture，并显式验证 EOF 边界。
fn replay_responses_fixture(document: &[u8]) -> Result<ResponsesStreamState, BridgeStreamError> {
    // 解码完整 fixture，并按 wire 顺序驱动 Responses 状态机。
    let mut state = ResponsesStreamState::new();
    for event in decode_fixture(document) {
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

#[test]
fn responses_failure_terminals_remain_distinct() {
    let failed = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.failed.terminal/upstream-stream.sse"
    ))
    .expect("Responses failed fixture must reach a terminal");
    let incomplete = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.incomplete.terminal/upstream-stream.sse"
    ))
    .expect("Responses incomplete fixture must reach a terminal");
    let error = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.error.terminal/upstream-stream.sse"
    ))
    .expect("Responses error fixture must reach a terminal");

    // 保留三个失败终态，避免 bridge 将 error 或 incomplete 伪装为 completed。
    assert_eq!(failed.terminal(), Some(StreamTerminal::Failed));
    assert_eq!(incomplete.terminal(), Some(StreamTerminal::Incomplete));
    assert_eq!(error.terminal(), Some(StreamTerminal::Error));
}

#[test]
fn bridge_replay_fails_closed_on_event_type_conflict_and_eof() {
    let conflict = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.event_type_conflict/upstream-stream.sse"
    ))
    .expect_err("event/type conflict must fail closed");
    let eof = replay_chat_fixture(include_bytes!(
        "../testdata/cases/faults/chat_native.eof_before_done/upstream-stream.sse"
    ))
    .expect_err("Chat EOF before DONE must fail closed");

    assert_eq!(conflict, BridgeStreamError::EventTypeConflict);
    assert_eq!(eof, BridgeStreamError::EofBeforeTerminal);
}

#[test]
fn responses_replay_rejects_events_and_duplicate_terminal_after_completion() {
    let document = include_bytes!(
        "../testdata/cases/faults/responses_native.terminal_violation/upstream-stream.sse"
    );
    let events = decode_fixture(document);

    // 回放完整 wire，确保首个 terminal 后的普通 event 立即失败关闭。
    let error = replay_responses_fixture(document)
        .expect_err("event after terminal must not be accepted by bridge state");
    assert_eq!(error, BridgeStreamError::UnexpectedEvent);

    // 跳过中间 late event，单独验证第二个 terminal 的稳定错误分类。
    let mut state = ResponsesStreamState::new();
    state.ingest(&events[0]).expect("created event must pass");
    state.ingest(&events[1]).expect("first terminal must pass");
    let error = state
        .ingest(&events[3])
        .expect_err("duplicate terminal must fail closed");
    assert_eq!(error, BridgeStreamError::DuplicateTerminal);
}

#[test]
fn responses_replay_rejects_duplicate_output_identity() {
    let events = decode_fixture(include_bytes!(
        "../testdata/cases/bridge/chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments/upstream-stream.sse"
    ));
    let mut state = ResponsesStreamState::new();

    // 重放同一个 canonical item-added event，验证 output identity 不能被重复注册。
    state.ingest(&events[0]).expect("created event must pass");
    state.ingest(&events[1]).expect("first item must pass");
    let error = state
        .ingest(&events[1])
        .expect_err("duplicate output identity must fail closed");
    assert_eq!(error, BridgeStreamError::DuplicateIdentity);
}
