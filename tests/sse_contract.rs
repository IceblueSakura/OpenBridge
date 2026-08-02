//! 验证增量 SSE decoder 的分片、UTF-8、CRLF、多行 data 和终态边界。

use openbridge::transport::sse::SseDecoder;

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
