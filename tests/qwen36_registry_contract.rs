//! Verifies the checked-in Qwen3.6 27B Public Model, Routes, and Bailian Chat wire contract.

use bytes::Bytes;
use openbridge::{
    config::parse_bootstrap_config,
    core::{ApiProtocol, ApiRequest, OperationKind, ReasoningOutput},
    pipeline::{analyze_request, plan_request},
    provider::{ProviderAdapter, ProviderKind},
    providers::build_compiled_registry,
    registry::RouteMode,
};
use serde_json::json;

#[test]
fn qwen36_27b_is_publicly_routable_through_bailian_chat() {
    // Compile the production registry and resolve the fixed Bailian deployment.
    let bootstrap = parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
        .expect("the checked-in bootstrap must remain valid");
    let registry = build_compiled_registry(bootstrap)
        .expect("the checked-in Qwen3.6 registration must compile");
    let target = registry
        .upstream_target("bailian-qwen3-6-27b")
        .expect("the fixed Qwen3.6 Bailian Target must exist");
    let chat = target
        .upstream_api(OperationKind::ChatCompletions)
        .expect("Qwen3.6 must use the confirmed Bailian Chat API");
    assert_eq!(chat.upstream_model(), "qwen3.6-27b");
    assert_eq!(chat.reasoning_output(), ReasoningOutput::PlainText);
    assert!(target.upstream_api(OperationKind::Responses).is_none());

    // Publish one Chat Native Route and one Responses-via-Chat Bridge Route.
    let public_model = registry
        .public_model("qwen3.6-27b")
        .expect("Qwen3.6 must be discoverable as a Public Model");
    assert_eq!(
        public_model.routes(),
        [
            "qwen3-6-27b-bailian-chat",
            "qwen3-6-27b-bailian-responses-via-chat"
        ]
    );
    for (route_id, expected_mode, expected_downstream) in [
        (
            "qwen3-6-27b-bailian-chat",
            RouteMode::Native,
            OperationKind::ChatCompletions,
        ),
        (
            "qwen3-6-27b-bailian-responses-via-chat",
            RouteMode::Bridged,
            OperationKind::Responses,
        ),
    ] {
        let route = registry
            .route(route_id)
            .expect("the published Route must resolve");
        assert_eq!(route.mode(), expected_mode, "{route_id}");
        assert_eq!(
            route.upstream_operation(),
            OperationKind::ChatCompletions,
            "{route_id}"
        );
        assert_eq!(
            route.downstream_operation(),
            expected_downstream,
            "{route_id}"
        );
    }

    // Expose only the confirmed binary reasoning contract on both downstream interfaces.
    let info = serde_json::to_value(public_model.info()).unwrap();
    for interface in ["chat_completions", "responses"] {
        assert_eq!(
            info["interfaces"][interface]["reasoning"]["levels"],
            json!(["none", "high"]),
            "{interface}"
        );
        assert_eq!(
            info["interfaces"][interface]["reasoning"]["output"], "plain_text",
            "{interface}"
        );
    }

    // Plan each downstream protocol through the same trusted Target without inventing Native Responses.
    for (protocol, body, expected_bridge) in [
        (
            ApiProtocol::ChatCompletions,
            Bytes::from_static(
                br#"{"model":"qwen3.6-27b","messages":[{"role":"user","content":"hello"}],"reasoning_effort":"high"}"#,
            ),
            false,
        ),
        (
            ApiProtocol::Responses,
            Bytes::from_static(
                br#"{"model":"qwen3.6-27b","input":"hello","reasoning":{"effort":"high"}}"#,
            ),
            true,
        ),
    ] {
        let requirements = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &requirements, body).unwrap();
        let [candidate] = plan.candidates() else {
            panic!("Qwen3.6 {protocol:?} must select exactly one candidate");
        };
        assert_eq!(candidate.upstream_target_id(), "bailian-qwen3-6-27b");
        assert_eq!(
            candidate.upstream_operation(),
            OperationKind::ChatCompletions
        );
        assert_eq!(candidate.bridge().is_some(), expected_bridge);
    }

    // Encode the admitted none/high levels with the boolean shape confirmed by Bailian Chat.
    let adapter = ProviderAdapter::for_kind(ProviderKind::Bailian);
    for (level, expected_enabled) in [("none", false), ("high", true)] {
        let request = ApiRequest::new(
            ApiProtocol::ChatCompletions,
            Bytes::from(format!(
                r#"{{"model":"qwen3.6-27b","messages":[{{"role":"user","content":"hello"}}],"reasoning_effort":"{level}"}}"#
            )),
        );
        let upstream = adapter.prepare_request(&request, "qwen3.6-27b").unwrap();
        let body: serde_json::Value = serde_json::from_slice(upstream.body()).unwrap();
        assert!(body.get("reasoning_effort").is_none(), "{level}");
        assert_eq!(body["enable_thinking"], expected_enabled, "{level}");
    }
}
