//! Verifies Images Generations analysis, wire conversion, bounded response validation, and zero-egress failures.

mod support;

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::{HeaderMap, HeaderValue, Method, header::CONTENT_TYPE};
use openbridge::{
    config::parse_bootstrap_config,
    core::{ImagesGenerationsCapabilities, ImagesResponseFormat, ImagesSizeDomain, OperationKind},
    ingress::{GatewayState, build_router},
    pipeline::{analyze_images_request, plan_images_request},
    provider::{CredentialKind, PreparedUpstreamRequest, ProviderKind},
    registry::{
        CanonicalModelTask, CanonicalTaskKind, CredentialPoolConfig, ImageGenerationModelProfile,
        ModelConfig, ModelContextLength, ModelLifecycle, ProviderInstanceConfig, PublicModelConfig,
        RegistryConfig, RouteConfig, RouteMode, UpstreamApiCapabilities, UpstreamApiConfig,
        UpstreamApiKey, UpstreamApiModelRules, UpstreamTarget, UpstreamTargetConfig,
        build_registry,
    },
    transport::upstream::{TransportError, UpstreamResponse, UpstreamTransport},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use support::{BOOTSTRAP, metrics::TestMetrics, users_and_credential_pool};

const DOWNSTREAM_KEY: &str = "downstream-token-0000000000000000";

/// Returns a minimal Images Generations registry with one synthetic Native Route.
fn images_definition() -> RegistryConfig {
    RegistryConfig {
        version: "images-test".to_owned(),
        models: vec![ModelConfig {
            id: "synthetic/image-model".to_owned(),
            name: "Synthetic image model".to_owned(),
            description: None,
            tokenizer: None,
            knowledge_cutoff: None,
            task: CanonicalModelTask::ImageGeneration(ImageGenerationModelProfile {
                context_length: ModelContextLength::new(None, None, None),
                supported_parameters: Vec::new(),
            }),
        }],
        provider_instances: vec![ProviderInstanceConfig {
            id: "synthetic-images".to_owned(),
            kind: ProviderKind::Bailian,
            base_url: "https://dashscope.example.com/api/v1".to_owned(),
        }],
        credential_pools: vec![CredentialPoolConfig {
            id: "synthetic-images-primary".to_owned(),
            provider: ProviderKind::Bailian,
            kind: CredentialKind::ApiKey,
        }],
        upstream_targets: vec![UpstreamTargetConfig {
            id: "synthetic-images-main".to_owned(),
            provider_instance: "synthetic-images".to_owned(),
            canonical_model: "synthetic/image-model".to_owned(),
            provider_model: "bailian/image-model".to_owned(),
            credential_pool: "synthetic-images-primary".to_owned(),
            quota_scope: None,
            fault_domain: None,
            timeout_policy: openbridge::registry::UpstreamTimeoutPolicy::new(
                std::time::Duration::from_secs(120),
            ),
            enabled: true,
            upstream_apis: vec![UpstreamApiConfig {
                key: UpstreamApiKey::new(
                    OperationKind::ImagesGenerations,
                    CanonicalTaskKind::ImageGeneration,
                ),
                upstream_model: "synthetic/image-model".to_owned(),
                model_rules: UpstreamApiModelRules::default(),
                capabilities: UpstreamApiCapabilities::ImagesGenerations(images_capabilities()),
                streaming_policy: openbridge::registry::UpstreamStreamingPolicy::Optional,
            }],
        }],
        routes: vec![RouteConfig {
            id: "synthetic-images-route".to_owned(),
            upstream_target: "synthetic-images-main".to_owned(),
            upstream_operation: OperationKind::ImagesGenerations,
            downstream_operation: OperationKind::ImagesGenerations,
            mode: RouteMode::Native,
        }],
        public_models: vec![PublicModelConfig {
            id: "synthetic-image".to_owned(),
            created: 1_785_715_200,
            display_name: "synthetic-image".to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            reasoning_level_policy: openbridge::registry::ReasoningLevelPolicy::Strict,
            routes: vec!["synthetic-images-route".to_owned()],
        }],
    }
}

fn images_capabilities() -> ImagesGenerationsCapabilities {
    ImagesGenerationsCapabilities {
        default_outputs: 1,
        max_outputs: 6,
        allowed_sizes: Some(ImagesSizeDomain {
            minimum_side: 512,
            maximum_side: 2_048,
            minimum_area: 512 * 512,
            maximum_area: 2_048 * 2_048,
        }),
        default_response_format: ImagesResponseFormat::Url,
        allowed_response_formats: Some(&[ImagesResponseFormat::Url]),
        supported_parameters: &["n", "output_format", "response_format", "size", "user"],
        dashscope_extensions: Some(openbridge::core::DashScopeImagesCapabilities {
            default_prompt_extend: true,
            prompt_extend_modes: &[
                openbridge::core::DashScopePromptExtendMode::Direct,
                openbridge::core::DashScopePromptExtendMode::Agent,
            ],
            default_prompt_extend_mode: openbridge::core::DashScopePromptExtendMode::Direct,
            default_enable_thinking: true,
            negative_prompt: true,
            maximum_seed: 2_147_483_647,
            default_watermark: false,
        }),
    }
}

#[derive(Debug)]
struct RecordedImagesRequest {
    method: Method,
    path: String,
    body: Value,
}

struct RecordingImagesTransport {
    requests: Mutex<Vec<RecordedImagesRequest>>,
    responses: Mutex<VecDeque<UpstreamResponse>>,
}

struct FailingImagesTransport {
    attempts: AtomicUsize,
    timeout: bool,
}

struct PendingImagesTransport {
    attempts: AtomicUsize,
    started: tokio::sync::Notify,
}

struct ImagesBodyDropSignal(Arc<AtomicUsize>);

impl Drop for ImagesBodyDropSignal {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl UpstreamTransport for PendingImagesTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.started.notify_one();
        Box::pin(std::future::pending())
    }
}

impl UpstreamTransport for FailingImagesTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        _request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if self.timeout {
                Err(TransportError::Timeout)
            } else {
                Err(TransportError::InvalidTarget)
            }
        })
    }
}

impl UpstreamTransport for RecordingImagesTransport {
    fn send<'a>(
        &'a self,
        _target: &'a UpstreamTarget,
        request: PreparedUpstreamRequest,
        _headers: HeaderMap,
    ) -> BoxFuture<'a, Result<UpstreamResponse, TransportError>> {
        self.requests.lock().unwrap().push(RecordedImagesRequest {
            method: request.method().clone(),
            path: request.relative_uri().to_string(),
            body: serde_json::from_slice(request.body()).expect("images request body is JSON"),
        });
        Box::pin(async move {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(TransportError::InvalidTarget)
        })
    }
}

fn dashscope_success(url: &str, count: usize) -> UpstreamResponse {
    let mut content = Vec::new();
    for _ in 0..count {
        content.push(json!({ "image": url }));
    }
    let body = serde_json::to_vec(&json!({
        "output": {
            "choices": [{
                "finish_reason": "stop",
                "message": { "role": "assistant", "content": content },
            }],
        },
        "usage": {
            "output_image_count": count,
            "output_width": 1024,
            "output_height": 1024
        },
        "request_id": "synthetic-request-id",
    }))
    .expect("dashscope response is serializable");
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    UpstreamResponse::new(StatusCode::OK, headers, Body::from(body))
}

fn images_router(
    transport: Arc<RecordingImagesTransport>,
) -> (axum::Router, Arc<openbridge::registry::RuntimeRegistry>) {
    let (router, registry, _) = images_router_with_metrics(transport);
    (router, registry)
}

fn images_router_with_metrics(
    transport: Arc<dyn UpstreamTransport>,
) -> (
    axum::Router,
    Arc<openbridge::registry::RuntimeRegistry>,
    TestMetrics,
) {
    images_router_with_bootstrap_and_metrics(transport, BOOTSTRAP)
}

fn images_router_with_bootstrap_and_metrics(
    transport: Arc<dyn UpstreamTransport>,
    bootstrap_document: &str,
) -> (
    axum::Router,
    Arc<openbridge::registry::RuntimeRegistry>,
    TestMetrics,
) {
    let bootstrap = parse_bootstrap_config(bootstrap_document).expect("bootstrap parses");
    let registry =
        Arc::new(build_registry(bootstrap, images_definition()).expect("images registry compiles"));
    let (users, credentials) = users_and_credential_pool(
        DOWNSTREAM_KEY,
        &registry,
        &["synthetic-secret-a", "synthetic-secret-b"],
    );
    let metrics = TestMetrics::new();
    let state = GatewayState::new(registry.clone(), transport, users, credentials)
        .with_metrics(metrics.instruments());
    (build_router(state), registry, metrics)
}

fn downstream_request(body: Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/v1/images/generations")
        .header(CONTENT_TYPE, "application/json")
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {DOWNSTREAM_KEY}"),
        )
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[test]
fn strict_analysis_distinguishes_unknown_fields_from_known_standard_fields() {
    // Unknown top-level fields are rejected before egress with a stable classification.
    let unknown = json!({ "model": "synthetic-image", "prompt": "a cat", "provider_magic": true });
    let error = analyze_images_request(&Bytes::from(serde_json::to_vec(&unknown).unwrap()))
        .expect_err("unknown field must be rejected");
    assert!(matches!(
        error,
        openbridge::pipeline::ImagesRequestError::InvalidRequest { .. }
    ));

    // Known OpenAI fields parse structurally; model-bound preflight owns support rejection.
    for known in [
        json!({ "model": "synthetic-image", "prompt": "a cat", "quality": "hd" }),
        json!({ "model": "synthetic-image", "prompt": "a cat", "response_format": "b64_json" }),
    ] {
        analyze_images_request(&Bytes::from(serde_json::to_vec(&known).unwrap()))
            .expect("known standard field must parse before preflight");
    }

    // A valid minimal request parses without complaint.
    let valid = json!({ "model": "synthetic-image", "prompt": "a cat" });
    let requirements = analyze_images_request(&Bytes::from(serde_json::to_vec(&valid).unwrap()))
        .expect("minimal request parses");
    assert_eq!(requirements.public_model(), "synthetic-image");
    assert_eq!(requirements.prompt_length(), 5);
}

#[tokio::test]
async fn known_but_unsupported_standard_fields_fail_with_field_level_zero_egress() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::new()),
    });
    let (router, _) = images_router(transport.clone());

    for (field, value) in [
        ("background", json!("transparent")),
        ("moderation", json!("auto")),
        ("output_compression", json!(80)),
        ("partial_images", json!(1)),
        ("quality", json!("high")),
        ("style", json!("vivid")),
        ("response_format", json!("b64_json")),
        ("output_format", json!("webp")),
        ("stream", json!(true)),
    ] {
        let response = router
            .clone()
            .oneshot(downstream_request(json!({
                "model": "synthetic-image",
                "prompt": "a cat",
                field: value,
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{field}");
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["error"]["code"], "unsupported_model_capability");
        assert_eq!(error["error"]["param"], field);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[test]
fn preflight_rejects_sizes_outside_the_fixed_domain() {
    let registry = build_registry(
        parse_bootstrap_config(BOOTSTRAP).expect("bootstrap parses"),
        images_definition(),
    )
    .expect("images registry compiles");
    let body = Bytes::from(
        serde_json::to_vec(
            &json!({ "model": "synthetic-image", "prompt": "a cat", "size": "64x64" }),
        )
        .unwrap(),
    );
    let requirements = analyze_images_request(&body).expect("size parses");
    let error = plan_images_request(&registry, &requirements, body)
        .expect_err("undersized image must fail preflight");
    assert!(matches!(
        error,
        openbridge::pipeline::ImagesRequestError::UnsupportedModelCapability { param: "size" }
    ));
}

#[tokio::test]
async fn native_images_route_converts_wire_and_validates_the_downstream_contract() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/image.png",
            2,
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport.clone());

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat",
            "n": 2,
            "size": "1024x1024",
            "response_format": "url",
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // The upstream wire must be the DashScope native shape with `*` size and no user/format fields.
    let recorded = transport.requests.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let request = &recorded[0];
    assert_eq!(request.method, Method::POST);
    assert_eq!(
        request.path,
        "/services/aigc/multimodal-generation/generation"
    );
    assert_eq!(request.body["model"], "synthetic/image-model");
    assert_eq!(
        request.body["input"]["messages"][0]["content"][0]["text"],
        "a cat"
    );
    assert_eq!(request.body["parameters"]["n"], 2);
    assert_eq!(request.body["parameters"]["size"], "1024*1024");
    assert!(request.body.get("response_format").is_none());
    assert!(request.body.get("user").is_none());
    drop(recorded);

    // The downstream body must be the OpenAI Images shape with both URLs in order.
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let downstream: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(downstream["data"].as_array().unwrap().len(), 2);
    assert_eq!(downstream["output_format"], "png");
    assert_eq!(downstream["size"], "1024x1024");
    assert_eq!(
        downstream["data"][0]["url"],
        "https://dashscope-result.example.com/image.png"
    );
    assert_eq!(
        downstream["data"][1]["url"],
        "https://dashscope-result.example.com/image.png"
    );
    let providers = metrics.provider_snapshots();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].attempts_started, 1);
    assert_eq!(providers[0].response_ready_ms.count, 1);
    assert_eq!(providers[0].upstream_first_byte_ms.count, 1);
    assert_eq!(providers[0].attempts_completed, 1);
    let gateway = metrics.snapshot();
    assert_eq!(gateway.images_output_count_observations, 1);
    assert_eq!(gateway.images_output_count, 2);
    assert_eq!(gateway.images_output_width, 1024);
    assert_eq!(gateway.images_output_height, 1024);
    let telemetry = serde_json::to_string(&(gateway, providers)).unwrap();
    assert!(!telemetry.contains("https://dashscope-result.example.com/image.png"));
    assert!(!telemetry.contains("a cat"));
}

#[tokio::test]
async fn dropping_validated_downstream_body_does_not_reclassify_provider_success() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/drop.png",
            1,
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport);

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "sensitive-prompt-marker"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    tokio::task::yield_now().await;

    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_completed, 1);
    assert_eq!(providers[0].attempts_stream_failed, 0);
    assert_eq!(providers[0].attempts_cancelled, 0);
}

#[tokio::test]
async fn standard_omitted_equivalents_and_fixed_png_contract_reach_the_native_route() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/image.png",
            1,
        )])),
    });
    let (router, _) = images_router(transport.clone());

    // OpenAI optional nulls are omission-equivalent; auto size and non-streaming PNG have exact
    // qwen semantics and must not leak as DashScope-native parameters.
    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat",
            "n": null,
            "size": "auto",
            "response_format": null,
            "user": null,
            "output_format": "png",
            "stream": false,
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let recorded = transport.requests.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let request = &recorded[0].body;
    assert!(request["parameters"].get("n").is_none());
    assert!(request["parameters"].get("size").is_none());
    for downstream_only in ["response_format", "user", "output_format", "stream"] {
        assert!(request.get(downstream_only).is_none());
    }
}

#[tokio::test]
async fn dashscope_extensions_are_validated_and_mapped_through_extra_body_shape() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/image.png",
            1,
        )])),
    });
    let (router, _) = images_router(transport.clone());

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat",
            "prompt_extend": true,
            "prompt_extend_mode": "agent",
            "enable_thinking": false,
            "negative_prompt": "text, watermark",
            "seed": 42,
            "watermark": true,
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let recorded = transport.requests.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].body["parameters"]["prompt_extend"], true);
    assert_eq!(
        recorded[0].body["parameters"]["prompt_extend_mode"],
        "agent"
    );
    assert_eq!(recorded[0].body["parameters"]["enable_thinking"], false);
    assert_eq!(
        recorded[0].body["parameters"]["negative_prompt"],
        "text, watermark"
    );
    assert_eq!(recorded[0].body["parameters"]["seed"], 42);
    assert_eq!(recorded[0].body["parameters"]["watermark"], true);
}

#[tokio::test]
async fn dashscope_defaults_are_frozen_and_conflicting_extension_fields_fail_before_egress() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/image.png",
            1,
        )])),
    });
    let (router, _) = images_router(transport.clone());

    // Omitted extension fields use explicit, reviewed qwen defaults rather than upstream drift.
    let response = router
        .clone()
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    {
        let recorded = transport.requests.lock().unwrap();
        let parameters = &recorded[0].body["parameters"];
        assert_eq!(parameters["prompt_extend"], true);
        assert_eq!(parameters["prompt_extend_mode"], "direct");
        assert_eq!(parameters["enable_thinking"], true);
        assert_eq!(parameters["watermark"], false);
    }

    // Explicitly disabling extension conflicts with mode/thinking children and never reaches egress.
    for (field, value) in [
        ("prompt_extend_mode", json!("agent")),
        ("enable_thinking", json!(false)),
    ] {
        let response = router
            .clone()
            .oneshot(downstream_request(json!({
                "model": "synthetic-image",
                "prompt": "a cat",
                "prompt_extend": false,
                field: value,
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["error"]["code"], "invalid_request_error");
        assert_eq!(error["error"]["param"], field);
    }
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[test]
fn dashscope_extensions_require_a_model_bound_extension_profile() {
    let mut definition = images_definition();
    let UpstreamApiCapabilities::ImagesGenerations(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[0].capabilities
    else {
        panic!("synthetic Images target must own Images capabilities");
    };
    capabilities.dashscope_extensions = None;
    let registry = build_registry(
        parse_bootstrap_config(BOOTSTRAP).expect("bootstrap parses"),
        definition,
    )
    .expect("registry without extensions remains valid");
    let body = Bytes::from(
        serde_json::to_vec(&json!({
            "model": "synthetic-image",
            "prompt": "a cat",
            "seed": 42
        }))
        .unwrap(),
    );
    let requirements = analyze_images_request(&body).expect("extension field parses structurally");
    let error = plan_images_request(&registry, &requirements, body)
        .expect_err("missing extension profile must fail preflight");
    assert!(matches!(
        error,
        openbridge::pipeline::ImagesRequestError::UnsupportedModelCapability { param: "seed" }
    ));
}

#[tokio::test]
async fn cancellation_before_headers_finishes_the_only_images_attempt_once() {
    let transport = Arc::new(PendingImagesTransport {
        attempts: AtomicUsize::new(0),
        started: tokio::sync::Notify::new(),
    });
    let (router, _, metrics) = images_router_with_metrics(transport.clone());
    let task = tokio::spawn(router.oneshot(downstream_request(json!({
        "model": "synthetic-image",
        "prompt": "a cat"
    }))));
    transport.started.notified().await;

    task.abort();
    let _ = task.await;
    tokio::task::yield_now().await;

    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);
    let gateway = metrics.snapshot();
    assert_eq!(gateway.upstream_attempts, 1);
    assert_eq!(gateway.requests_cancelled, 1);
    assert_eq!(gateway.upstream_retries, 0);
    assert_eq!(gateway.credential_rotations, 0);
    assert_eq!(gateway.route_fallbacks, 0);
    let providers = metrics.provider_snapshots();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].attempts_started, 1);
    assert_eq!(providers[0].attempts_cancelled, 1);
}

#[tokio::test]
async fn non_success_headers_record_one_http_failed_attempt_without_recovery() {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([UpstreamResponse::new(
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            Body::from(r#"{"code":"RateLimit"}"#),
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport.clone());

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
    let gateway = metrics.snapshot();
    assert_eq!(gateway.upstream_attempts, 1);
    assert_eq!(gateway.upstream_http_failures, 1);
    assert_eq!(gateway.upstream_retries, 0);
    assert_eq!(gateway.credential_rotations, 0);
    assert_eq!(gateway.route_fallbacks, 0);
    let providers = metrics.provider_snapshots();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].attempts_started, 1);
    assert_eq!(providers[0].attempts_http_failed, 1);
}

#[tokio::test]
async fn timeout_returns_504_and_records_one_non_replayed_provider_attempt() {
    let transport = Arc::new(FailingImagesTransport {
        attempts: AtomicUsize::new(0),
        timeout: true,
    });
    let (router, _, metrics) = images_router_with_metrics(transport.clone());

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "a cat"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "upstream_timeout");
    assert_eq!(transport.attempts.load(Ordering::SeqCst), 1);

    let gateway = metrics.snapshot();
    assert_eq!(gateway.upstream_attempts, 1);
    assert_eq!(gateway.upstream_transport_failures, 1);
    assert_eq!(gateway.upstream_retries, 0);
    assert_eq!(gateway.credential_rotations, 0);
    assert_eq!(gateway.route_fallbacks, 0);
    let providers = metrics.provider_snapshots();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].key.operation, "images_generations");
    assert_eq!(providers[0].attempts_started, 1);
    assert_eq!(providers[0].attempts_transport_failed, 1);
}

#[tokio::test]
async fn cancellation_while_reading_images_body_drops_source_and_marks_attempt_cancelled() {
    let body_started = Arc::new(tokio::sync::Notify::new());
    let body_dropped = Arc::new(AtomicUsize::new(0));
    let started = body_started.clone();
    let dropped = body_dropped.clone();
    let body = Body::from_stream(futures_util::stream::once(async move {
        started.notify_one();
        let _guard = ImagesBodyDropSignal(dropped);
        std::future::pending::<Result<Bytes, std::io::Error>>().await
    }));
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([UpstreamResponse::new(
            StatusCode::OK,
            headers,
            body,
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport);
    let task = tokio::spawn(router.oneshot(downstream_request(json!({
        "model": "synthetic-image",
        "prompt": "a cat"
    }))));
    body_started.notified().await;

    task.abort();
    let _ = task.await;
    tokio::task::yield_now().await;

    assert_eq!(body_dropped.load(Ordering::SeqCst), 1);
    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_cancelled, 1);
    assert_eq!(providers[0].attempts_completed, 0);
    assert_eq!(metrics.snapshot().images_output_count_observations, 0);
}

#[tokio::test]
async fn oversized_success_body_fails_before_commit_without_image_usage() {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([UpstreamResponse::new(
            StatusCode::OK,
            headers,
            Body::from("x".repeat(256)),
        )])),
    });
    let bootstrap = BOOTSTRAP.replace(
        "max_json_response_body_bytes = 16777216",
        "max_json_response_body_bytes = 128",
    );
    let (router, _, metrics) = images_router_with_bootstrap_and_metrics(transport, &bootstrap);

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "sensitive-prompt-marker"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_stream_failed, 1);
    assert_eq!(providers[0].attempts_completed, 0);
    assert_eq!(metrics.snapshot().images_output_count_observations, 0);
}

#[tokio::test]
async fn body_transport_failure_fails_before_commit_without_image_usage() {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let body = Body::from_stream(futures_util::stream::once(async {
        Err::<Bytes, _>(std::io::Error::other("synthetic image body failure"))
    }));
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([UpstreamResponse::new(
            StatusCode::OK,
            headers,
            body,
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport);

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "sensitive-prompt-marker"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_stream_failed, 1);
    assert_eq!(providers[0].attempts_completed, 0);
    assert_eq!(metrics.snapshot().images_output_count_observations, 0);
}

#[tokio::test]
async fn malformed_or_early_eof_json_fails_before_commit_without_image_usage() {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([UpstreamResponse::new(
            StatusCode::OK,
            headers,
            Body::from(r#"{"output":"#),
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport);

    let response = router
        .oneshot(downstream_request(json!({
            "model": "synthetic-image",
            "prompt": "sensitive-prompt-marker"
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "invalid_upstream_response");
    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_stream_failed, 1);
    assert_eq!(providers[0].attempts_completed, 0);
    assert_eq!(metrics.snapshot().images_output_count_observations, 0);
}

#[tokio::test]
async fn image_count_mismatch_fails_closed_before_downstream_commit() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::from([dashscope_success(
            "https://dashscope-result.example.com/image.png",
            1,
        )])),
    });
    let (router, _, metrics) = images_router_with_metrics(transport.clone());

    // The request resolves to two outputs but the upstream returns one; the gateway must fail closed.
    let response = router
        .oneshot(downstream_request(
            json!({ "model": "synthetic-image", "prompt": "a cat", "n": 2 }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let downstream: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(downstream["error"]["code"], "invalid_upstream_response");
    let providers = metrics.provider_snapshots();
    assert_eq!(providers[0].attempts_stream_failed, 1);
    assert_eq!(providers[0].attempts_completed, 0);
    assert_eq!(metrics.snapshot().images_output_count_observations, 0);
}

#[tokio::test]
async fn invalid_requests_are_rejected_before_any_upstream_attempt() {
    let transport = Arc::new(RecordingImagesTransport {
        requests: Mutex::new(Vec::new()),
        responses: Mutex::new(VecDeque::new()),
    });
    let (router, _) = images_router(transport.clone());

    // Unknown fields, missing prompts, and out-of-domain sizes must never reach the upstream.
    for body in [
        json!({ "model": "synthetic-image", "quality": "hd" }),
        json!({ "prompt": "a cat" }),
        json!({ "model": "synthetic-image", "prompt": "a cat", "size": "99x99" }),
    ] {
        let response = router
            .clone()
            .oneshot(downstream_request(body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}
