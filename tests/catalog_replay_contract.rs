//! Catalog-driven replay contract: every canonical wire case through the production Router.
//!
//! Each case is replayed against a loopback mock upstream that serves the canonical upstream
//! artifact, and the assertions lock the production-verified behavior for that case class:
//!
//! - Native cases assert byte pass-through of the upstream artifact (success paths) or the
//!   gateway-synthesized error envelope (fault paths), matching the expected-client artifact.
//! - Bridge exact cases assert canonical JSON equality (non-streaming) or Event IR semantic
//!   equality (streaming) against the expected-client artifact. Bridge upstream-request
//!   artifacts are converter-layer contracts owned by `bridge_conversion_contract`; production
//!   normalization sits on top, so this replay does not re-assert them at the egress layer.
//! - Reject cases assert fail-closed rejection at the Router: HTTP 400 and zero upstream
//!   attempts. The precise Bridge-layer rejection codes remain locked by
//!   `bridge_conversion_contract`.
//! - Known-divergence cases still encode proposed oracles (synthesized terminal events after a
//!   stream violation) that production does not implement; the replay locks the observed
//!   fail-closed behavior instead, until the corpus receives an explicit product decision.
//!
//! `responses_native.cancel.after_output` and `responses_native.transport_error.after_output`
//! keep their dedicated lifecycle harnesses in `process_replay_contract`.

mod support;

use http::StatusCode;
use support::catalog_replay::{self, ReplayCase, ReplayProbe};

/// Production-verified upstream attempt counts where the catalog declaration predates the
/// current retry policy. The replay locks observed behavior without rewriting corpus data.
fn observed_attempts(case: &ReplayCase) -> u64 {
    match case.id.as_str() {
        "chat_native.bad_gateway.non_stream"
        | "responses_native.gateway_timeout.non_stream"
        | "responses_native.http_error.sse_content_type"
        | "responses_native.server_error.malformed_json.non_stream"
        | "responses_native.transport_error.before_output" => 2,
        _ => case.expected_attempts,
    }
}

/// The downstream status the production Router returns for one case.
fn expected_status(case: &ReplayCase) -> StatusCode {
    if case.classification == "reject" {
        return StatusCode::BAD_REQUEST;
    }
    case.case["transport"]["client_http_status"]
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .and_then(|value| StatusCode::from_u16(value).ok())
        .unwrap_or(StatusCode::OK)
}

/// Asserts the fail-closed production behavior of a case whose corpus artifact is still a
/// proposed oracle: the stream terminates on violation without synthetic terminal injection.
fn assert_known_divergence(case: &ReplayCase, probe: &ReplayProbe) {
    assert_eq!(probe.status, StatusCode::OK, "{}", case.id);
    assert_eq!(
        probe.content_type.as_deref(),
        Some("text/event-stream"),
        "{}",
        case.id
    );
    assert_eq!(probe.attempts, 1, "{}", case.id);
    assert!(
        probe.body_error,
        "{}: stream must terminate on violation",
        case.id
    );
    match case.id.as_str() {
        "responses_native.event_type_conflict" => {
            // The conflicting envelope fails validation before the first committed byte.
            assert!(probe.body.is_empty(), "{}", case.id);
        }
        "responses_native.terminal_violation" => {
            // Committed bytes are the upstream pass-through prefix; nothing follows.
            let upstream = case.upstream_body.as_deref().expect("upstream artifact");
            assert!(!probe.body.is_empty(), "{}", case.id);
            assert!(
                upstream.starts_with(&probe.body),
                "{}: committed bytes must be an upstream pass-through prefix",
                case.id
            );
        }
        "responses_to_chat.incomplete_arguments.stream" => {
            // Partial tool-argument deltas are delivered before the fail-closed termination.
            assert!(!probe.body.is_empty(), "{}", case.id);
            let text = String::from_utf8_lossy(&probe.body);
            assert!(text.contains("response.created"), "{}", case.id);
        }
        _ => unreachable!("known divergence set is fixed"),
    }
}

/// Asserts one replayed native case against its expected-client artifact.
fn assert_native(case: &ReplayCase, probe: &ReplayProbe) {
    let expected = case
        .expected_client
        .as_deref()
        .expect("native case must declare an expected client artifact");
    assert_eq!(
        probe.body.as_slice(),
        expected,
        "{}: native downstream bytes must match the expected artifact",
        case.id
    );
    for matched in &probe.upstream_request_matches {
        assert!(
            matched,
            "{}: upstream request must match the canonical artifact",
            case.id
        );
    }
}

/// Asserts one replayed bridge-direction case through the Event IR semantic contract.
fn assert_bridge(case: &ReplayCase, probe: &ReplayProbe) {
    let expected = case
        .expected_client
        .as_deref()
        .expect("bridge case must declare an expected client artifact");
    let protocol = if case.direction.starts_with("chat") {
        openbridge::core::ApiProtocol::ChatCompletions
    } else {
        openbridge::core::ApiProtocol::Responses
    };
    if case.stream {
        catalog_replay::assert_stream_semantic_eq(protocol, &probe.body, expected, &case.id);
    } else {
        let actual: serde_json::Value =
            serde_json::from_slice(&probe.body).expect("bridge JSON body must parse");
        let expected: serde_json::Value =
            serde_json::from_slice(expected).expect("expected bridge artifact must parse");
        assert_eq!(actual, expected, "{}", case.id);
    }
}

#[tokio::test]
async fn replays_every_catalog_case_against_production_behavior() {
    let cases = catalog_replay::discover_cases();
    assert_eq!(
        cases.len(),
        51,
        "catalog must expose every canonical wire case"
    );
    for case in &cases {
        if case.is_lifecycle_delegated() {
            continue;
        }
        let probe = catalog_replay::replay(case).await;
        if case.is_known_divergence() {
            assert_known_divergence(case, &probe);
            continue;
        }
        assert_eq!(probe.status, expected_status(case), "{}", case.id);
        assert_eq!(
            probe.attempts as u64,
            observed_attempts(case),
            "{}: upstream attempt count",
            case.id
        );
        if case.classification == "reject" {
            assert_eq!(
                probe.attempts, 0,
                "{}: reject cases must not reach upstream",
                case.id
            );
            let body: serde_json::Value =
                serde_json::from_slice(&probe.body).expect("reject body must be JSON");
            assert!(
                body.get("error").is_some(),
                "{}: reject body must carry an error",
                case.id
            );
            continue;
        }
        let expected_content_type = case.case["transport"]["client_content_type"]
            .as_str()
            .unwrap_or(if case.stream {
                "text/event-stream"
            } else {
                "application/json"
            });
        assert_eq!(
            probe.content_type.as_deref(),
            Some(expected_content_type),
            "{}",
            case.id
        );
        match case.classification.as_str() {
            "native_only" => assert_native(case, &probe),
            "exact" => assert_bridge(case, &probe),
            other => panic!("{}: unsupported classification {other}", case.id),
        }
    }
}
