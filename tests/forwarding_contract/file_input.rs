//! Proves typed file-input projection, Native fidelity, and fail-closed source handling.

use super::*;
use openbridge::core::{
    ChatFileInputProfile, FileDetail, FileDetailProfile, FileInlineEncoding, FileMediaType,
    InlineFileInputLimits, InlineFileInputProfile, ResponsesFileInputProfile,
};

const FILE_ENCODINGS: &[FileInlineEncoding] =
    &[FileInlineEncoding::RawBase64, FileInlineEncoding::DataUrl];
const PDF_MEDIA_TYPES: &[FileMediaType] = &[FileMediaType::Pdf];
const FILE_DETAILS: &[FileDetail] = &[FileDetail::Auto, FileDetail::Low, FileDetail::High];
const FILE_DETAIL: FileDetailProfile = FileDetailProfile::new(FileDetail::Auto, FILE_DETAILS);
const INLINE_LIMITS: InlineFileInputLimits = InlineFileInputLimits::new(1_024, 768, 2_048, 1_536);
const INLINE_PDF: InlineFileInputProfile =
    InlineFileInputProfile::new(FILE_ENCODINGS, PDF_MEDIA_TYPES, INLINE_LIMITS);
const CHAT_FILE: ChatFileInputProfile = ChatFileInputProfile::new(2, 255, INLINE_PDF);
const RESPONSES_FILE: ResponsesFileInputProfile =
    ResponsesFileInputProfile::new(2, 255, Some(8_192), Some(INLINE_PDF), FILE_DETAIL);

#[tokio::test]
async fn synthetic_native_file_input_projects_and_preserves_exact_wire() {
    let definition = file_input_definition();
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);

    // Project the normalized protocol-specific source sets from the same executable profiles.
    let model = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    assert_eq!(
        model["interfaces"]["chat_completions"]["multimodal_input"]["file"],
        serde_json::json!({
            "sources": ["inline_data"],
            "encodings": ["raw_base64", "data_url"],
            "media_types": ["application/pdf"],
            "detail": null,
            "limits": {
                "max_parts": 2,
                "max_filename_length": 255,
                "max_url_length": null,
                "max_inline_encoded_bytes": 1024,
                "max_inline_decoded_bytes": 768,
                "max_total_inline_encoded_bytes": 2048,
                "max_total_inline_decoded_bytes": 1536
            }
        })
    );
    assert_eq!(
        model["interfaces"]["responses"]["multimodal_input"]["file"]["sources"],
        serde_json::json!(["inline_data", "remote_url"])
    );
    assert_eq!(
        model["interfaces"]["responses"]["multimodal_input"]["file"]["detail"],
        serde_json::json!({"default":"auto","allowed":["auto","low","high"]})
    );

    let chat_body = serde_json::json!({
        "model": "public-model",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Summarize this file."},
                {"type": "file", "file": {"filename": "brief.pdf", "file_data": "JVBERi0xLjQ"}}
            ]
        }]
    });
    let responses_body = serde_json::json!({
        "model": "public-model",
        "input": [{
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Summarize this file."},
                {"type": "input_file", "filename": "brief.pdf", "file_url": "https://files.example.com/brief.pdf"}
            ]
        }]
    });
    let chat_data_url_body = serde_json::json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": [
            {"type": "file", "file": {"filename": "brief.pdf", "file_data": "data:application/pdf;base64,JVBERi0xLjQ="}}
        ]}]
    });
    let responses_inline_body = serde_json::json!({
        "model": "public-model",
        "input": [{"type": "input_file", "filename": "brief.pdf",
            "file_data": "data:application/pdf;base64,JVBERi0xLjQ=", "detail": "high"}]
    });
    for (path, body) in [
        ("/v1/chat/completions", chat_body.clone()),
        ("/v1/chat/completions", chat_data_url_body.clone()),
        ("/v1/responses", responses_body.clone()),
        ("/v1/responses", responses_inline_body.clone()),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }

    // Native preparation changes only trusted model/state fields, not ordered file content.
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].body["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        &chat_body["messages"][0]
    );
    assert_eq!(
        requests[1].body["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        &chat_data_url_body["messages"][0]
    );
    assert_eq!(requests[2].body["input"], responses_body["input"]);
    assert_eq!(requests[3].body["input"], responses_inline_body["input"]);
}

#[tokio::test]
async fn file_id_and_production_models_remain_zero_egress() {
    let transport = Arc::new(MimoImageTransport::default());
    let synthetic = app_with_transport_and_definition(transport.clone(), file_input_definition());
    let response = synthetic
        .oneshot(
            Request::post("/v1/responses")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","input":[{"role":"user","content":[{"type":"input_file","file_id":"file_synthetic"}]}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(transport.requests.lock().unwrap().is_empty());

    let production = app_with_compiled_registry(transport.clone());
    let model = compiled_authenticated_get(&production, "/openbridge/v1/models/gpt-5.6-sol").await;
    assert!(model["interfaces"]["responses"]["multimodal_input"]["file"].is_null());
}

#[tokio::test]
async fn malformed_sources_media_and_limits_are_zero_egress() {
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), file_input_definition());
    let long_filename = format!("{}.pdf", "a".repeat(252));
    let oversized_data = "A".repeat(1_028);
    let cases = vec![
        serde_json::json!({"model":"public-model","messages":[{"role":"user","content":[
            {"type":"file","file":{"filename":"brief.pdf","file_data":"not-base64"}}
        ]}]}),
        serde_json::json!({"model":"public-model","messages":[{"role":"user","content":[
            {"type":"file","file":{"filename":"brief.pdf","file_data":"A"}}
        ]}]}),
        serde_json::json!({"model":"public-model","messages":[{"role":"user","content":[
            {"type":"file","detail":"high","file":{"filename":"brief.pdf","file_data":"JVBERi0xLjQ="}}
        ]}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.pdf",
            "file_data":"JVBERi0xLjQ=","file_url":"https://files.example.com/brief.pdf"}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.txt",
            "file_data":"JVBERi0xLjQ="}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.pdf",
            "file_data":"JVBERi0xLjQ=","detail":"original"}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.pdf",
            "file_url":"http://files.example.com/brief.pdf"}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.pdf",
            "file_url":"https://127.0.0.1/brief.pdf"}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":long_filename,
            "file_data":"JVBERi0xLjQ="}]}),
        serde_json::json!({"model":"public-model","input":[{"type":"input_file","filename":"brief.pdf",
            "file_data":oversized_data}]}),
        serde_json::json!({"model":"public-model","input":[
            {"type":"input_file","filename":"one.pdf","file_data":"JVBERi0xLjQ="},
            {"type":"input_file","filename":"two.pdf","file_data":"JVBERi0xLjQ="},
            {"type":"input_file","filename":"three.pdf","file_data":"JVBERi0xLjQ="}
        ]}),
    ];
    for body in cases {
        let protocol = if body.get("messages").is_some() {
            "/v1/chat/completions"
        } else {
            "/v1/responses"
        };
        let response = app
            .clone()
            .oneshot(
                Request::post(protocol)
                    .header(CONTENT_TYPE, "application/json")
                    .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(transport.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn generation_bridge_never_contributes_file_capability() {
    let mut definition = file_input_definition();
    definition.public_models[0].routes = vec![RouteConfig {
        upstream_target: "openai-main".to_owned(),
        upstream_operation: OperationKind::Responses,
        downstream_operation: OperationKind::ChatCompletions,
    }];
    let transport = Arc::new(MimoImageTransport::default());
    let app = app_with_transport_and_definition(transport.clone(), definition);
    let model = authenticated_get(&app, "/openbridge/v1/models/public-model").await;
    assert!(model["interfaces"]["chat_completions"]["multimodal_input"]["file"].is_null());

    let response = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, "Bearer downstream-token-0000000000000000")
                .body(Body::from(
                    r#"{"model":"public-model","messages":[{"role":"user","content":[{"type":"file","file":{"filename":"brief.pdf","file_data":"JVBERi0xLjQ="}}]}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(transport.requests.lock().unwrap().is_empty());
}

fn file_input_definition() -> RegistryConfig {
    let mut definition = support::definition("file-input-test", "public-model", "upstream-model");
    let chat = match &mut definition.upstream_targets[0].upstream_apis[0].capabilities {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => capabilities,
        _ => panic!("first synthetic API must remain Chat Completions"),
    };
    chat.media.file = Some(CHAT_FILE);
    let responses = match &mut definition.upstream_targets[0].upstream_apis[1].capabilities {
        UpstreamApiCapabilities::Responses(capabilities) => capabilities,
        _ => panic!("second synthetic API must remain Responses"),
    };
    responses.media.file = Some(RESPONSES_FILE);
    definition
}
