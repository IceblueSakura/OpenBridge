//! Verifies retry, fallback, cooldown, credential rotation, cancellation, and SSE failure boundaries.

use super::*;

#[tokio::test]
async fn stateless_requests_keep_fallback_across_continuation_capable_targets() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    let UpstreamApiCapabilities::Responses(primary_capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("second synthetic API must be Responses");
    };
    primary_capabilities.state = ExecutableResponsesState::new(
        StorageSupport::Unsupported,
        ResponsesAffinity::TargetBoundContinuation,
    );
    let mut fallback = definition.upstream_targets[0].clone();
    fallback.id = "openai-fallback".to_owned();
    fallback.upstream_apis[1].upstream_model = "fallback-model".to_owned();
    definition.upstream_targets.push(fallback);
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "fallback-responses".to_owned(),
        upstream_target: "openai-fallback".to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: openbridge::core::ApiProtocol::Responses.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("fallback-responses".to_owned());
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    // Omit continuation state so ambiguous continuation issuers cannot disable ordinary fallback.
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let attempted_models = transport.attempted_models.lock().unwrap();
    assert_eq!(attempted_models.last(), Some(&"fallback-model".to_owned()));
    assert!(
        attempted_models
            .iter()
            .any(|model| model == "upstream-model")
    );
}

#[tokio::test]
async fn transient_failures_back_off_and_fall_back_to_another_provider_with_final_error() {
    // Build an OpenAI primary target and LongCat fallback, then make both return transient failures.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    add_responses_fallback(&mut definition, "longcat-fallback", ProviderKind::LongCat);
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let started = Instant::now();

    // Execute the request and wait for the bounded retry/fallback lifecycle to converge.
    let response = app.oneshot(request).await.unwrap();

    // Verify exponential backoff, cross-Provider order, and the final safe HTTP error.
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "3");
    assert!(started.elapsed() >= Duration::from_millis(150));
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(body, r#"{"error":{"message":"longcat-fallback failed"}}"#);
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(
        attempts
            .iter()
            .map(|(target, provider, _)| (target.as_str(), *provider))
            .collect::<Vec<_>>(),
        vec![
            ("openai-main", ProviderKind::OpenAi),
            ("openai-main", ProviderKind::OpenAi),
            ("longcat-fallback", ProviderKind::LongCat),
        ]
    );
}

#[tokio::test]
async fn cross_request_credential_cooldown_skips_targets_sharing_the_exhausted_pool() {
    // Build three targets sharing a single-member pool and verify member cooldown across targets.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.upstream_targets[0].quota_scope = Some("shared-quota".to_owned());
    add_responses_fallback(&mut definition, "shared-quota-peer", ProviderKind::OpenAi);
    definition.upstream_targets[1].quota_scope = Some("shared-quota".to_owned());
    add_responses_fallback(&mut definition, "independent-target", ProviderKind::OpenAi);
    definition.upstream_targets[2].quota_scope = Some("independent-quota".to_owned());
    let transport = Arc::new(ScopedHealthTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Preserve the final 429 from the first request; the second returns a controlled 503 without a live attempt.
    for expected in [
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
    }

    // All targets share one pool, allowing only the first live attempt during cooldown.
    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        ["openai-main",]
    );
}

#[tokio::test]
async fn cross_request_health_skips_all_targets_in_the_cooled_fault_domain() {
    // Build two targets sharing a fault domain and one independent failure boundary.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    definition.upstream_targets[0].fault_domain = Some("shared-fault".to_owned());
    add_responses_fallback(&mut definition, "shared-fault-peer", ProviderKind::OpenAi);
    definition.upstream_targets[1].fault_domain = Some("shared-fault".to_owned());
    add_responses_fallback(&mut definition, "independent-target", ProviderKind::OpenAi);
    definition.upstream_targets[2].fault_domain = Some("independent-fault".to_owned());
    let transport = Arc::new(ScopedFaultTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Both consecutive requests should select the independent fault domain after the primary first fails.
    for _ in 0..2 {
        let request = Request::post("/v1/responses")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        [
            "openai-main",
            "openai-main",
            "independent-target",
            "independent-target",
        ]
    );
}

#[tokio::test]
async fn target_bound_continuation_ignores_cooldown_without_cross_target_fallback() {
    // Enable continuation on one uniquely identifiable Responses API and put its target into cooldown first.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.state = ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        );
    }
    let transport = Arc::new(ScopedHealthTransport::default());

    // Confirm one unique issuer keeps continuation in both typed state and parameters.
    let registry =
        build_registry(support::bootstrap(support::BOOTSTRAP), definition.clone()).unwrap();
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["responses"]["state"]["previous_response_id"],
        "supported"
    );
    assert!(
        info["interfaces"]["responses"]["supported_parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter == "previous_response_id")
    );
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // A stateless request cools down the only member and preserves the final live 429.
    let warmup = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(warmup).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    // Continuation must retry the original target and must not silently switch to fallback because of cooldown.
    let continuation = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","previous_response_id":"resp_123"}"#,
        ))
        .unwrap();
    let response = app.oneshot(continuation).await.unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        transport.attempts.lock().unwrap().as_slice(),
        ["openai-main", "openai-main",]
    );
}

#[tokio::test]
async fn ambiguous_target_bound_continuation_is_rejected_before_upstream() {
    // Enable continuation on two different targets without an issuer ledger.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.state = ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        );
    }
    add_responses_fallback(&mut definition, "alternate-issuer", ProviderKind::OpenAi);
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[1].upstream_apis[1].capabilities
    {
        capabilities.state = ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        );
    }
    let transport = Arc::new(ScopedHealthTransport::default());

    // Confirm the public Models contract removes continuation from both typed state and parameters.
    let registry =
        build_registry(support::bootstrap(support::BOOTSTRAP), definition.clone()).unwrap();
    let info = serde_json::to_value(registry.public_model("public-model").unwrap().info()).unwrap();
    assert_eq!(
        info["interfaces"]["responses"]["state"]["previous_response_id"],
        "unsupported"
    );
    assert!(
        !info["interfaces"]["responses"]["supported_parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter == "previous_response_id")
    );
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Reject the ambiguous continuation during fixed Public Model preflight without selecting a target.
    let continuation = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","previous_response_id":"resp_123"}"#,
        ))
        .unwrap();
    let response = app.oneshot(continuation).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("unsupported_model_capability"));
    assert!(transport.attempts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn non_streaming_transient_failures_use_the_same_finite_retry_policy() {
    // Build a non-streaming Chat request with one failing target.
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","messages":[{"role":"user","content":"hello"}]}"#,
        ))
        .unwrap();
    let started = Instant::now();

    // Execute the request and wait for bounded backoff retries on the same candidate.
    let response = app.oneshot(request).await.unwrap();

    // Verify that the non-streaming path uses the same policy without exceeding the candidate-local limit.
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(started.elapsed() >= Duration::from_millis(50));
    assert_eq!(transport.attempts.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn request_attempt_budget_is_global_and_reserves_untried_fallbacks() {
    // Build four ordered candidates that all fail and exceed the request budget for two attempts each.
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    add_responses_fallback(&mut definition, "longcat-second", ProviderKind::LongCat);
    add_responses_fallback(&mut definition, "openai-third", ProviderKind::OpenAi);
    add_responses_fallback(&mut definition, "longcat-fourth", ProviderKind::LongCat);
    let transport = Arc::new(BoundedFailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    // Execute the request until the request-wide budget or candidate set converges.
    let response = app.oneshot(request).await.unwrap();

    // Verify that the hard limit of six still covers all four candidates and returns the last candidate error.
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(body, r#"{"error":{"message":"longcat-fourth failed"}}"#);
    let attempts = transport.attempts.lock().unwrap();
    assert_eq!(
        attempts
            .iter()
            .map(|(target, _, _)| target.as_str())
            .collect::<Vec<_>>(),
        vec![
            "openai-main",
            "openai-main",
            "longcat-second",
            "openai-third",
            "openai-third",
            "longcat-fourth",
        ]
    );
}

#[tokio::test]
async fn provider_bound_streams_do_not_try_a_second_route_for_the_same_issuer() {
    let mut definition = support::definition("forward-test", "public-model", "upstream-model");
    if let openbridge::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.state = ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        );
    }
    definition.routes.push(openbridge::registry::RouteConfig {
        id: "fallback-responses".to_owned(),
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: openbridge::core::ApiProtocol::Responses.operation(),
        mode: openbridge::registry::RouteMode::Native,
    });
    definition.public_models[0]
        .routes
        .push("fallback-responses".to_owned());
    let transport = Arc::new(FailoverTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true,"previous_response_id":"resp_123"}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        transport.attempted_models.lock().unwrap().as_slice(),
        ["upstream-model", "upstream-model"]
    );
}

#[tokio::test]
async fn dropping_the_downstream_stream_cancels_the_pending_upstream_stream() {
    let dropped = Arc::new(AtomicBool::new(false));
    let app = app_with_transport(Arc::new(PendingSseTransport {
        dropped: dropped.clone(),
    }));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn aborting_downstream_before_response_cancels_the_pending_upstream_request() {
    // Build an upstream request that remains pending before response headers.
    let dropped = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(PendingRequestTransport {
        attempts: AtomicUsize::new(0),
        started: tokio::sync::Notify::new(),
        dropped: dropped.clone(),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let task = tokio::spawn(app.oneshot(request));
    transport.started.notified().await;

    // Simulate downstream disconnection and wait for the handler future to finish cancellation.
    task.abort();
    let error = task.await.unwrap_err();

    // Verify that the pending send was dropped and no second attempt started.
    assert!(error.is_cancelled());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn aborting_downstream_during_backoff_prevents_the_next_attempt() {
    // Build an upstream request that fails immediately and enters backoff.
    let transport = Arc::new(BackoffCancellationTransport {
        attempts: AtomicUsize::new(0),
        first_attempt: tokio::sync::Notify::new(),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();
    let task = tokio::spawn(app.oneshot(request));
    transport.first_attempt.notified().await;

    // Simulate downstream disconnection before the backoff timer completes.
    task.abort();
    let error = task.await.unwrap_err();
    tokio::time::sleep(Duration::from_millis(75)).await;

    // Wait beyond the first backoff interval and verify that the timer did not start a background request.
    assert!(error.is_cancelled());
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn eof_before_terminal_does_not_fabricate_a_terminal_event() {
    let app = app_with_transport(Arc::new(EofWithoutTerminalTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();

    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("response.output_text.delta")
    );
    assert!(
        !std::str::from_utf8(&body)
            .unwrap()
            .contains("response.completed")
    );
    assert!(!std::str::from_utf8(&body).unwrap().contains("[DONE]"));
}

#[tokio::test]
async fn buffered_non_streaming_responses_reject_eof_before_terminal() {
    let app = app_with_streaming_only_responses_transport(
        Arc::new(EofWithoutTerminalTransport),
        16_777_216,
    );
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    // Refuse to synthesize a non-streaming response from a stream without an explicit terminal.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
}

#[tokio::test]
async fn buffered_non_streaming_responses_reject_invalid_sse() {
    let app =
        app_with_streaming_only_responses_transport(Arc::new(InvalidSseTransport), 16_777_216);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    // Convert framing or UTF-8 failures into one safe JSON gateway error before response takeover.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
}

#[tokio::test]
async fn buffered_non_streaming_responses_require_sse_content_type() {
    let app =
        app_with_streaming_only_responses_transport(Arc::new(SuccessfulJsonTransport), 16_777_216);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    // Do not reinterpret a success body that violates the configured streaming-only API contract.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
}

#[tokio::test]
async fn native_streaming_responses_require_sse_content_type() {
    let app = app_with_transport(Arc::new(SuccessfulJsonTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    // Reject a successful non-SSE body before committing a native streaming response.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
}

#[tokio::test]
async fn buffered_non_streaming_responses_enforce_the_json_takeover_budget() {
    let app =
        app_with_streaming_only_responses_transport(Arc::new(OversizedResponsesSseTransport), 128);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    // Reject the raw SSE body before buffering can exceed the configured non-streaming budget.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
}

#[tokio::test]
async fn partial_upstream_stream_failures_close_without_a_retry() {
    let transport = Arc::new(PartialStreamFailureTransport {
        attempts: AtomicUsize::new(0),
    });
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await.is_err());
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_upstream_sse_closes_the_stream_after_output_starts() {
    let app = app_with_transport(Arc::new(InvalidSseTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await.is_err());
}

#[tokio::test]
async fn streaming_requests_preserve_non_sse_error_bodies() {
    let app = app_with_transport(Arc::new(NonSseErrorTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        b"\xff".as_slice()
    );
}

#[tokio::test]
async fn rate_limit_rotates_to_the_next_credential_member_before_output() {
    // Inject two synthetic members into one Provider pool; the second should complete after the first returns 429.
    let transport = Arc::new(CredentialRotationTransport::default());
    let (app, metrics) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    // Verify that rotation shares the existing retry budget and does not replay the rejected member.
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        transport.authorizations.lock().unwrap().as_slice(),
        ["Bearer key-a", "Bearer key-b"]
    );
    assert_eq!(metrics.snapshot().credential_rotations, 1);
    assert_eq!(metrics.snapshot().upstream_retries, 1);
}

#[tokio::test]
async fn healthy_requests_share_the_pool_round_robin_cursor() {
    // Two independent requests share the GatewayState cursor and should use different members in sequence.
    let transport = Arc::new(FixedStatusCredentialTransport {
        status: StatusCode::OK,
        authorizations: Mutex::new(Vec::new()),
    });
    let (app, _) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
    for _ in 0..2 {
        let request = Request::post("/v1/chat/completions")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        transport.authorizations.lock().unwrap().as_slice(),
        ["Bearer key-a", "Bearer key-b"]
    );
}

#[tokio::test]
async fn rate_limited_member_stays_cooled_while_a_successful_peer_remains_available() {
    // The first request cools down key-a and succeeds with key-b; the second must not hit key-a again.
    let transport = Arc::new(CredentialRotationTransport::default());
    let (app, _) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
    for _ in 0..2 {
        let request = Request::post("/v1/chat/completions")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        transport.authorizations.lock().unwrap().as_slice(),
        ["Bearer key-a", "Bearer key-b", "Bearer key-b"]
    );
}

#[tokio::test]
async fn server_errors_retry_the_same_member_without_rotating() {
    // A 503 from a two-member pool still uses the existing candidate retry policy, but the credential must remain fixed.
    let transport = Arc::new(FixedStatusCredentialTransport {
        status: StatusCode::SERVICE_UNAVAILABLE,
        authorizations: Mutex::new(Vec::new()),
    });
    let (app, metrics) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
        .unwrap();

    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        transport.authorizations.lock().unwrap().as_slice(),
        ["Bearer key-a", "Bearer key-a"]
    );
    assert_eq!(metrics.snapshot().credential_rotations, 0);
}

#[tokio::test]
async fn two_rate_limited_members_exhaust_the_candidate_without_wrapping() {
    // When both members return 429, each is attempted at most once and the final safe HTTP error is preserved.
    let transport = Arc::new(FixedStatusCredentialTransport {
        status: StatusCode::TOO_MANY_REQUESTS,
        authorizations: Mutex::new(Vec::new()),
    });
    let (app, metrics) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
    let request = Request::post("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
        .unwrap();

    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        transport.authorizations.lock().unwrap().as_slice(),
        ["Bearer key-a", "Bearer key-b"]
    );
    assert_eq!(metrics.snapshot().credential_rotations, 1);
}

#[tokio::test]
async fn non_429_client_errors_do_not_retry_or_rotate_credentials() {
    // Non-429 4xx responses are terminal for the request and must not expand authentication or quota probing through another key.
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::PAYMENT_REQUIRED,
        StatusCode::FORBIDDEN,
        StatusCode::REQUEST_TIMEOUT,
    ] {
        let transport = Arc::new(FixedStatusCredentialTransport {
            status,
            authorizations: Mutex::new(Vec::new()),
        });
        let (app, metrics) = app_with_transport_and_pool(transport.clone(), &["key-a", "key-b"]);
        let request = Request::post("/v1/chat/completions")
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
            .body(Body::from(r#"{"model":"public-model","messages":[]}"#))
            .unwrap();

        assert_eq!(app.oneshot(request).await.unwrap().status(), status);
        assert_eq!(
            transport.authorizations.lock().unwrap().as_slice(),
            ["Bearer key-a"]
        );
        assert_eq!(metrics.snapshot().credential_rotations, 0);
    }
}

#[tokio::test]
async fn streaming_rate_limits_retry_before_output_and_preserve_retry_headers() {
    let transport = Arc::new(RateLimitedTransport::default());
    let app = app_with_transport(transport.clone());
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(
            r#"{"model":"public-model","input":"hello","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(transport.attempts.lock().unwrap().to_owned(), 1);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()["retry-after"], "2");
    assert_eq!(response.headers()["x-should-retry"], "true");
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        r#"{"error":{"message":"rate limited"}}"#
    );
}

#[tokio::test]
async fn upstream_timeouts_return_a_safe_gateway_timeout() {
    let app = app_with_transport(Arc::new(TimeoutTransport));
    let request = Request::post("/v1/responses")
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
        .body(Body::from(r#"{"model":"public-model","input":"hello"}"#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "upstream_timeout");
    assert!(!std::str::from_utf8(&body).unwrap().contains("reqwest"));
}
