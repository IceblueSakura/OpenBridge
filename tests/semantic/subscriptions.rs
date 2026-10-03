//! Independent outbound contracts for subscription Responses execution.
use openbridge::{
    adapter::{Adapter, Dialect},
    protocol::openai::Profile,
    topology::{EndpointId, catalog::default_topology},
};
use serde_json::{Value, json};
#[test]
fn subscription_requests_pin_models_preserve_limits_and_separate_delivery() {
    let topology = default_topology().unwrap();
    let client = Adapter::new(Profile::Responses, Dialect::OpenBridge, None);
    assert!(topology.model("grok4.7").is_none());
    for (profile, public, upstream, forced) in [
        ("codex", "gpt-6.1-sol", "gpt-6.1-sol", true),
        ("grok", "grok-4.7", "grok-4.7", false),
    ] {
        let model = topology.model(public).expect("public model registered");
        assert_eq!(model.canonical_model.as_str(), upstream);
        let endpoint = topology
            .endpoint(&EndpointId::new(&format!("{profile}-responses")).unwrap())
            .unwrap();
        for streaming in [false, true] {
            let body =
                json!({"model":public,"input":"hello","stream":streaming,"max_output_tokens":37});
            let request = client.decode_request(body.to_string().as_bytes()).unwrap();
            let wire = endpoint
                .adapter()
                .encode_request(&request, &endpoint.upstream_model, &endpoint.representation)
                .unwrap();
            assert_eq!(wire["model"], upstream);
            assert_eq!(wire["max_output_tokens"], 37);
            assert_eq!(wire["stream"], streaming || forced);
            assert_eq!(wire["store"], false);
            assert_eq!(request.delivery.streaming(), streaming);
            let mut disabled = endpoint.clone();
            disabled.execution.streaming = false;
            if forced || streaming {
                assert_eq!(
                    openbridge::execution::plan::representable(&disabled, &request),
                    Err(openbridge::execution::plan::RejectionReason::Streaming)
                );
            }
            if forced {
                assert_eq!(wire["instructions"], "");
            }
            // Task edits are authoritative; encoding cannot restore the previous budget.
            let mut edited = request.clone();
            let mut settings = edited.task.semantic.settings().clone();
            settings.controls.max_output_tokens = None;
            edited.task.semantic = edited.task.semantic.with_settings(settings).unwrap();
            let wire = endpoint
                .adapter()
                .encode_request(&edited, upstream, &endpoint.representation)
                .unwrap();
            assert!(wire.get("max_output_tokens").is_none());
            let client_wire = client
                .encode_request(&request, public, &endpoint.representation)
                .unwrap();
            assert_eq!(client_wire["stream"], Value::Bool(streaming));
        }
    }
}
