//! Protects high-risk Responses bridge terminal and output-identity state transitions.

use openbridge::{
    bridge::{BridgeStreamError, ResponsesStreamState, StreamTerminal},
    transport::sse::{SseDecoder, SseEvent},
};

fn decode_fixture(document: &[u8]) -> Vec<SseEvent> {
    let mut decoder = SseDecoder::new(256 * 1024);
    let mut events = decoder.push(document).expect("fixture SSE must decode");
    events.extend(decoder.finish().expect("fixture SSE must finish"));
    events
}

fn replay_responses_fixture(document: &[u8]) -> Result<ResponsesStreamState, BridgeStreamError> {
    let mut state = ResponsesStreamState::new();
    for event in decode_fixture(document) {
        state.ingest(&event)?;
    }
    state.finish()?;
    Ok(state)
}

#[test]
fn responses_failure_terminals_remain_distinct() {
    let failed = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.failed.terminal/upstream-stream.sse"
    ))
    .expect("failed fixture must reach a terminal");
    let incomplete = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.incomplete.terminal/upstream-stream.sse"
    ))
    .expect("incomplete fixture must reach a terminal");
    let error = replay_responses_fixture(include_bytes!(
        "../testdata/cases/faults/responses_native.error.terminal/upstream-stream.sse"
    ))
    .expect("error fixture must reach a terminal");
    let cancelled = replay_responses_fixture(
        br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_cancelled","status":"in_progress"}}

event: response.cancelled
data: {"type":"response.cancelled","response":{"id":"resp_cancelled","status":"cancelled"}}

"#,
    )
    .expect("cancelled stream must reach a terminal");

    assert_eq!(failed.terminal(), Some(StreamTerminal::Failed));
    assert_eq!(incomplete.terminal(), Some(StreamTerminal::Incomplete));
    assert_eq!(error.terminal(), Some(StreamTerminal::Error));
    assert_eq!(cancelled.terminal(), Some(StreamTerminal::Cancelled));
}

#[test]
fn responses_reject_events_and_duplicate_terminal_after_completion() {
    let document = include_bytes!(
        "../testdata/cases/faults/responses_native.terminal_violation/upstream-stream.sse"
    );
    let events = decode_fixture(document);

    let error =
        replay_responses_fixture(document).expect_err("an event after terminal must fail closed");
    assert_eq!(error, BridgeStreamError::UnexpectedEvent);

    let mut state = ResponsesStreamState::new();
    state.ingest(&events[0]).expect("created event must pass");
    state.ingest(&events[1]).expect("first terminal must pass");
    let error = state
        .ingest(&events[3])
        .expect_err("duplicate terminal must fail closed");
    assert_eq!(error, BridgeStreamError::DuplicateTerminal);
}

#[test]
fn responses_reject_duplicate_output_identity() {
    let events = decode_fixture(include_bytes!(
        "../testdata/cases/bridge/chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments/upstream-stream.sse"
    ));
    let mut state = ResponsesStreamState::new();

    state.ingest(&events[0]).expect("created event must pass");
    state.ingest(&events[1]).expect("first item must pass");
    let error = state
        .ingest(&events[1])
        .expect_err("duplicate output identity must fail closed");
    assert_eq!(error, BridgeStreamError::DuplicateIdentity);
}
