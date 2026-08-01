//! 通过真实 loopback HTTP socket 回放 canonical corpus case。

mod support;

use http::StatusCode;

#[tokio::test]
async fn responses_rate_limit_case_replays_two_attempts_and_final_safe_error() {
    let observation =
        support::process_replay::replay_rate_limit_case("responses_native.rate_limit.non_stream")
            .await;

    // 对照 corpus 固定最终 HTTP 语义和请求级 attempt observation。
    assert_eq!(observation.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(observation.retry_after.as_deref(), Some("1"));
    assert_eq!(observation.upstream_attempts, 2);
    assert_eq!(observation.upstream_request_matches, vec![true, true]);
    assert!(observation.downstream_body_matches);
}
