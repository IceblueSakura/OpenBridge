//! Verifies incremental SSE decoder fragmentation, UTF-8, CRLF, multiline data, and terminal boundaries.

use openbridge::transport::sse::{SseDecodeError, SseDecoder};

#[test]
fn decoder_handles_fragmented_utf8_crlf_and_multiline_data() {
    let payload = "event: response.output_text.delta\r\nid: evt-1\r\ndata: {\"delta\":\"A😊\"}\r\ndata: second\r\n\r\n";
    let split = payload.find('😊').unwrap() + 1;
    let mut decoder = SseDecoder::new(1024);

    assert!(
        decoder
            .push(&payload.as_bytes()[..split])
            .unwrap()
            .is_empty()
    );
    let events = decoder.push(&payload.as_bytes()[split..]).unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event(), Some("response.output_text.delta"));
    assert_eq!(events[0].id(), Some("evt-1"));
    assert_eq!(events[0].data(), "{\"delta\":\"A😊\"}\nsecond");
}

#[test]
fn event_size_limit_is_applied_per_event_not_per_chunk() {
    let mut decoder = SseDecoder::new(12);

    let events = decoder.push(b"data: one\n\ndata: two\n\n").unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].data(), "one");
    assert_eq!(events[1].data(), "two");
}

#[test]
fn finish_preserves_a_complete_event_without_a_trailing_blank_line() {
    let mut decoder = SseDecoder::new(64);

    assert!(decoder.push(b"data: final").unwrap().is_empty());
    let events = decoder.finish().unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data(), "final");
}

#[test]
fn decoder_parses_retry_and_ignores_invalid_retry_and_nul_ids() {
    let mut decoder = SseDecoder::new(256);

    // Combine valid and invalid control fields in one complete event.
    let events = decoder
        .push(b"id: stable\nretry: invalid\nretry: 1500\nid: rejected\0id\ndata: ready\n\n")
        .unwrap();

    // Preserve the last valid values while ignoring malformed replacements.
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id(), Some("stable"));
    assert_eq!(events[0].retry_ms(), Some(1500));
    assert_eq!(events[0].data(), "ready");
}

#[test]
fn decoder_rejects_invalid_utf8_in_complete_and_terminal_lines() {
    // Reject invalid UTF-8 as soon as a newline completes the field.
    let mut complete = SseDecoder::new(64);
    assert_eq!(
        complete.push(b"data: \xff\n").unwrap_err(),
        SseDecodeError::InvalidUtf8
    );

    // Retain an incomplete field until EOF and reject it at finalization.
    let mut terminal = SseDecoder::new(64);
    assert!(terminal.push(b"data: \xff").unwrap().is_empty());
    assert_eq!(terminal.finish().unwrap_err(), SseDecodeError::InvalidUtf8);
}

#[test]
fn decoder_rejects_complete_events_and_incomplete_lines_over_the_limit() {
    // Enforce the limit while consuming a complete line.
    let mut complete = SseDecoder::new(9);
    assert_eq!(
        complete.push(b"data: one\n").unwrap_err(),
        SseDecodeError::EventTooLarge
    );

    // Enforce the same limit before an attacker supplies any newline boundary.
    let mut incomplete = SseDecoder::new(4);
    assert_eq!(
        incomplete.push(b"data:").unwrap_err(),
        SseDecodeError::EventTooLarge
    );
}
