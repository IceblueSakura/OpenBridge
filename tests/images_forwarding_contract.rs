//! Verifies Images Generations analysis, wire conversion, bounded response validation, and zero-egress failures.

mod support;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
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

use support::{BOOTSTRAP, users_and_credentials};

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
            request_timeout: std::time::Duration::from_secs(120),
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
        supported_parameters: &["n", "response_format", "size", "user"],
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
        "usage": { "output_image_count": count },
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
    let bootstrap = parse_bootstrap_config(BOOTSTRAP).expect("bootstrap parses");
    let registry =
        Arc::new(build_registry(bootstrap, images_definition()).expect("images registry compiles"));
    let (users, credentials) = users_and_credentials(DOWNSTREAM_KEY, &registry, "synthetic-secret");
    let state = GatewayState::new(registry.clone(), transport, users, credentials);
    (build_router(state), registry)
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
fn strict_analysis_rejects_unknown_fields_and_b64_json() {
    // Unknown top-level fields are rejected before egress with a stable classification.
    let unknown = json!({ "model": "synthetic-image", "prompt": "a cat", "quality": "hd" });
    let error = analyze_images_request(&Bytes::from(serde_json::to_vec(&unknown).unwrap()))
        .expect_err("unknown field must be rejected");
    assert!(matches!(
        error,
        openbridge::pipeline::ImagesRequestError::InvalidRequest { .. }
    ));

    // b64_json is a closed-format rejection in the first Images contract.
    let b64 =
        json!({ "model": "synthetic-image", "prompt": "a cat", "response_format": "b64_json" });
    let error = analyze_images_request(&Bytes::from(serde_json::to_vec(&b64).unwrap()))
        .expect_err("b64_json must be rejected");
    assert!(matches!(
        error,
        openbridge::pipeline::ImagesRequestError::InvalidRequest {
            param: Some("response_format")
        }
    ));

    // A valid minimal request parses without complaint.
    let valid = json!({ "model": "synthetic-image", "prompt": "a cat" });
    let requirements = analyze_images_request(&Bytes::from(serde_json::to_vec(&valid).unwrap()))
        .expect("minimal request parses");
    assert_eq!(requirements.public_model(), "synthetic-image");
    assert_eq!(requirements.prompt_length(), 5);
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
    let (router, _) = images_router(transport.clone());

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
    assert_eq!(
        downstream["data"][0]["url"],
        "https://dashscope-result.example.com/image.png"
    );
    assert_eq!(
        downstream["data"][1]["url"],
        "https://dashscope-result.example.com/image.png"
    );
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
    let (router, _) = images_router(transport.clone());

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
