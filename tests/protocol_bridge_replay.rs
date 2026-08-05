//! Replays the Protocol Bridge stream state machines with the canonical corpus.
//!
//! These tests read fixed fixtures only; they do not start a real Provider or attach a bridge route to production ingress.

use openbridge::{
    bridge::{BridgeStreamError, ChatStreamState, ResponsesStreamState, StreamTerminal},
    transport::sse::{SseDecoder, SseEvent},
};

/// Decodes canonical SSE wire data into logical events while preserving wire order.
fn decode_fixture(document: &[u8]) -> Vec<SseEvent> {
    // Decode the complete fixture and dispatch the final closed event at EOF.
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(document).expect("fixture SSE must decode");
    events.extend(decoder.finish().expect("fixture SSE must finish"));
    events
}

/// Replays a Chat fixture in wire order and explicitly verifies the EOF boundary.
fn replay_chat_fixture(document: &[u8]) -> Result<ChatStreamState, BridgeStreamError> {
    // Decode the complete fixture and drive the Chat state machine in wire order.
    let mut state = ChatStreamState::new();
    for event in decode_fixture(document) {
        state.ingest(&event)?;
    }

    // End input explicitly so EOF cannot replace the protocol terminal.
    state.finish()?;
    Ok(state)
}

/// Replays a Responses fixture in wire order and explicitly verifies the EOF boundary.
fn replay_responses_fixture(document: &[u8]) -> Result<ResponsesStreamState, BridgeStreamError> {
    // Decode the complete fixture and drive the Responses state machine in wire order.
    let mut state = ResponsesStreamState::new();
    for event in decode_fixture(document) {
        state.ingest(&event)?;
    }

    // End input explicitly so the terminal and all tool arguments are closed.
    state.finish()?;
    Ok(state)
}

#[test]
fn responses_stream_replay_preserves_parallel_tool_identity_and_arguments() {
    let state = replay_responses_fixture(include_bytes!(
        "../testdata/cases/bridge/chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments/upstream-stream.sse"
    ))
        .expect("canonical Responses fixture must complete");

    // Verify that output-item, call, and argument identities are not conflated.
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

    // Verify that the Chat index links fragments without replacing the stable call ID.
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

    // Preserve three failure terminals so the bridge cannot present error or incomplete as completed.
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

    // Replay the complete wire so an ordinary event after the first terminal fails immediately.
    let error = replay_responses_fixture(document)
        .expect_err("event after terminal must not be accepted by bridge state");
    assert_eq!(error, BridgeStreamError::UnexpectedEvent);

    // Skip the intermediate late event and check the stable classification of a second terminal.
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

    // Replay the same canonical item-added event and verify that output identity cannot be registered twice.
    state.ingest(&events[0]).expect("created event must pass");
    state.ingest(&events[1]).expect("first item must pass");
    let error = state
        .ingest(&events[1])
        .expect_err("duplicate output identity must fail closed");
    assert_eq!(error, BridgeStreamError::DuplicateIdentity);
}
