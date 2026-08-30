//! Replays a canonical corpus case through a real loopback HTTP socket.

mod support;

use http::StatusCode;

fn assert_http_failure_metrics(
    observation: &support::process_replay::ReplayObservation,
    expected_attempts: u64,
    expected_retries: u64,
    case_id: &str,
) {
    // Assert the request terminal and routing decisions once per canonical replay.
    assert_eq!(observation.gateway_metrics.requests_started, 1, "{case_id}");
    assert_eq!(
        observation.gateway_metrics.requests_completed, 0,
        "{case_id}"
    );
    assert_eq!(
        observation.gateway_metrics.requests_http_failed, 1,
        "{case_id}"
    );
    assert_eq!(observation.gateway_metrics.requests_failed, 0, "{case_id}");
    assert_eq!(
        observation.gateway_metrics.requests_cancelled, 0,
        "{case_id}"
    );
    assert_eq!(
        observation.gateway_metrics.upstream_retries, expected_retries,
        "{case_id}"
    );
    assert_eq!(
        observation.gateway_metrics.credential_rotations, 0,
        "{case_id}"
    );
    assert_eq!(observation.gateway_metrics.route_fallbacks, 0, "{case_id}");

    // Assert every actual Provider attempt finishes exactly once as an HTTP failure.
    assert_eq!(observation.provider_metrics.len(), 1, "{case_id}");
    let provider = &observation.provider_metrics[0];
    assert_eq!(provider.attempts_started, expected_attempts, "{case_id}");
    assert_eq!(
        provider.attempts_http_failed, expected_attempts,
        "{case_id}"
    );
    assert_eq!(provider.attempts_completed, 0, "{case_id}");
    assert_eq!(provider.attempts_transport_failed, 0, "{case_id}");
    assert_eq!(provider.attempts_stream_failed, 0, "{case_id}");
    assert_eq!(provider.attempts_cancelled, 0, "{case_id}");
}

#[tokio::test]
async fn responses_http_error_with_sse_content_type_stays_an_http_error() {
    let observation = support::process_replay::replay_http_error_case(
        "responses_native.http_error.sse_content_type",
    )
    .await;

    // Prove HTTP status classification wins over the misleading SSE Content-Type on every attempt.
    assert_eq!(observation.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        observation.content_type.as_deref(),
        Some("text/event-stream")
    );
    assert_eq!(observation.upstream_attempts, 2);
    assert_eq!(observation.upstream_request_matches, vec![true, true]);
    assert!(observation.downstream_body_matches);
}

#[tokio::test]
async fn canonical_non_retryable_client_errors_stop_after_one_attempt() {
    for (case_id, expected_status) in [
        (
            "chat_native.invalid_request.non_stream",
            StatusCode::BAD_REQUEST,
        ),
        (
            "chat_native.permission_denied.non_stream",
            StatusCode::FORBIDDEN,
        ),
        (
            "chat_native.unprocessable_entity.non_stream",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "responses_native.authentication_error.non_stream",
            StatusCode::UNAUTHORIZED,
        ),
        (
            "responses_native.not_found.non_stream",
            StatusCode::NOT_FOUND,
        ),
    ] {
        let observation = support::process_replay::replay_http_error_case(case_id).await;

        // Preserve each canonical HTTP response without expanding a non-retryable client failure.
        assert_eq!(observation.status, expected_status, "{case_id}");
        assert_eq!(
            observation.content_type.as_deref(),
            Some("application/json"),
            "{case_id}"
        );
        assert_eq!(observation.retry_after, None, "{case_id}");
        assert_eq!(observation.rate_limit_remaining_requests, None, "{case_id}");
        assert_eq!(observation.upstream_attempts, 1, "{case_id}");
        assert_eq!(
            observation.upstream_request_matches,
            vec![true],
            "{case_id}"
        );
        assert!(observation.downstream_body_matches, "{case_id}");
        assert_http_failure_metrics(&observation, 1, 0, case_id);
    }
}

#[tokio::test]
async fn canonical_rate_limits_preserve_retry_after_formats() {
    for (case_id, expected_retry_after, expected_remaining) in [
        ("chat_native.rate_limit.non_stream", "1", None),
        ("responses_native.rate_limit.non_stream", "1", None),
        (
            "responses_native.rate_limit.http_date.non_stream",
            "Wed, 21 Oct 2037 07:28:00 GMT",
            Some("0"),
        ),
    ] {
        let observation = support::process_replay::replay_http_error_case(case_id).await;

        // Keep the declared cooldown hint while a single-member pool prevents credential replay.
        assert_eq!(
            observation.status,
            StatusCode::TOO_MANY_REQUESTS,
            "{case_id}"
        );
        assert_eq!(
            observation.content_type.as_deref(),
            Some("application/json"),
            "{case_id}"
        );
        assert_eq!(
            observation.retry_after.as_deref(),
            Some(expected_retry_after),
            "{case_id}"
        );
        assert_eq!(
            observation.rate_limit_remaining_requests.as_deref(),
            expected_remaining,
            "{case_id}"
        );
        assert_eq!(observation.upstream_attempts, 1, "{case_id}");
        assert_eq!(
            observation.upstream_request_matches,
            vec![true],
            "{case_id}"
        );
        assert!(observation.downstream_body_matches, "{case_id}");
        assert_http_failure_metrics(&observation, 1, 0, case_id);
    }
}

#[tokio::test]
async fn canonical_server_errors_retry_once_and_preserve_http_classification() {
    for (case_id, expected_status, expected_content_type, expect_body_match) in [
        (
            "chat_native.bad_gateway.non_stream",
            StatusCode::BAD_GATEWAY,
            "text/plain",
            false,
        ),
        (
            "responses_native.gateway_timeout.non_stream",
            StatusCode::GATEWAY_TIMEOUT,
            "application/json",
            true,
        ),
        (
            "responses_native.server_error.malformed_json.non_stream",
            StatusCode::INTERNAL_SERVER_ERROR,
            "application/json",
            false,
        ),
    ] {
        let observation = support::process_replay::replay_http_error_case(case_id).await;

        // Classify status before body parsing, then stop after the candidate's bounded local retry.
        assert_eq!(observation.status, expected_status, "{case_id}");
        assert_eq!(
            observation.content_type.as_deref(),
            Some(expected_content_type),
            "{case_id}"
        );
        assert_eq!(observation.retry_after, None, "{case_id}");
        assert_eq!(observation.rate_limit_remaining_requests, None, "{case_id}");
        assert_eq!(observation.upstream_attempts, 2, "{case_id}");
        assert_eq!(
            observation.upstream_request_matches,
            vec![true, true],
            "{case_id}"
        );
        if expect_body_match {
            assert!(observation.downstream_body_matches, "{case_id}");
        }
        assert_http_failure_metrics(&observation, 2, 1, case_id);
    }
}

#[tokio::test]
async fn responses_transport_error_after_output_does_not_retry_or_append_terminal() {
    let observation = support::process_replay::replay_transport_error_after_output_case(
        "responses_native.transport_error.after_output",
    )
    .await;

    // Preserve only the canonical visible events, then terminate the downstream stream as failed.
    assert_eq!(observation.status, StatusCode::OK);
    assert_eq!(
        observation.content_type.as_deref(),
        Some("text/event-stream")
    );
    assert!(observation.downstream_stream_matches_upstream);
    assert!(observation.downstream_transport_error);
    assert_eq!(observation.upstream_attempts, 1);
    assert_eq!(observation.upstream_request_matches, vec![true]);

    // Submit exactly one failed request and one stream-failed Provider attempt without retry or fallback.
    assert_eq!(observation.gateway_metrics.requests_completed, 0);
    assert_eq!(observation.gateway_metrics.requests_failed, 1);
    assert_eq!(observation.gateway_metrics.requests_cancelled, 0);
    assert_eq!(observation.gateway_metrics.upstream_retries, 0);
    assert_eq!(observation.gateway_metrics.route_fallbacks, 0);
    assert_eq!(observation.provider_metrics.len(), 1);
    assert_eq!(observation.provider_metrics[0].attempts_started, 1);
    assert_eq!(observation.provider_metrics[0].attempts_stream_failed, 1);
    assert_eq!(observation.provider_metrics[0].attempts_completed, 0);
}

#[tokio::test]
async fn responses_cancel_after_visible_delta_drops_upstream_without_retry() {
    let observation = support::process_replay::replay_cancel_after_output_case(
        "responses_native.cancel.after_output",
    )
    .await;

    // Consume through the first lifecycle-valid visible delta, then prove client drop reaches the upstream body.
    assert_eq!(observation.status, StatusCode::OK);
    assert_eq!(
        observation.content_type.as_deref(),
        Some("text/event-stream")
    );
    assert!(observation.downstream_stream_matches_upstream);
    assert!(observation.upstream_cancelled);
    assert_eq!(observation.upstream_attempts, 1);
    assert_eq!(observation.upstream_request_matches, vec![true]);

    // Submit exactly one cancelled request and Provider attempt without retry, fallback, or stream failure.
    assert_eq!(observation.gateway_metrics.requests_completed, 0);
    assert_eq!(observation.gateway_metrics.requests_failed, 0);
    assert_eq!(observation.gateway_metrics.requests_cancelled, 1);
    assert_eq!(observation.gateway_metrics.upstream_retries, 0);
    assert_eq!(observation.gateway_metrics.route_fallbacks, 0);
    assert_eq!(observation.provider_metrics.len(), 1);
    assert_eq!(observation.provider_metrics[0].attempts_started, 1);
    assert_eq!(observation.provider_metrics[0].attempts_cancelled, 1);
    assert_eq!(observation.provider_metrics[0].attempts_stream_failed, 0);
    assert_eq!(observation.provider_metrics[0].attempts_completed, 0);
}

#[tokio::test]
async fn responses_eof_before_terminal_preserves_partial_stream_and_records_failure() {
    let observation = support::process_replay::replay_eof_before_terminal_case(
        "responses_native.eof_before_terminal",
    )
    .await;

    // Preserve the canonical partial stream, then expose terminal-free EOF as a body failure.
    assert_eq!(observation.status, StatusCode::OK);
    assert_eq!(
        observation.content_type.as_deref(),
        Some("text/event-stream")
    );
    assert!(observation.downstream_stream_matches_upstream);
    assert!(observation.downstream_transport_error);
    assert_eq!(observation.upstream_attempts, 1);
    assert_eq!(observation.upstream_request_matches, vec![true]);

    // Submit exactly one failed request and stream-failed Provider attempt without retry or fallback.
    assert_eq!(observation.gateway_metrics.requests_completed, 0);
    assert_eq!(observation.gateway_metrics.requests_failed, 1);
    assert_eq!(observation.gateway_metrics.requests_cancelled, 0);
    assert_eq!(observation.gateway_metrics.upstream_retries, 0);
    assert_eq!(observation.gateway_metrics.route_fallbacks, 0);
    assert_eq!(observation.provider_metrics.len(), 1);
    assert_eq!(observation.provider_metrics[0].attempts_started, 1);
    assert_eq!(observation.provider_metrics[0].attempts_stream_failed, 1);
    assert_eq!(observation.provider_metrics[0].attempts_completed, 0);
}
