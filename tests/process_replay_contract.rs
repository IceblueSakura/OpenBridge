//! Replays a canonical corpus case through a real loopback HTTP socket.

mod support;

use http::StatusCode;

#[tokio::test]
async fn responses_rate_limit_case_does_not_replay_an_exhausted_single_member() {
    let observation =
        support::process_replay::replay_rate_limit_case("responses_native.rate_limit.non_stream")
            .await;

    // Compare the fixed corpus HTTP semantics and request-level attempt observation.
    assert_eq!(observation.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(observation.retry_after.as_deref(), Some("1"));
    assert_eq!(observation.upstream_attempts, 1);
    assert_eq!(observation.upstream_request_matches, vec![true]);
    assert!(observation.downstream_body_matches);
}
