use openbridge::{
    config::load_bootstrap,
    core::Protocol,
    models::longcat,
    pipeline::{analyze_request, plan_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_definition},
    registry::{ReasoningSupport, build_registry},
};

#[test]
fn checked_in_bootstrap_and_compiled_registry_are_loadable() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap = load_bootstrap(bootstrap).expect("checked-in bootstrap must remain valid");
    let snapshot =
        build_compiled_registry(bootstrap).expect("compiled registry must remain internally valid");

    assert_eq!(snapshot.version().as_str(), "dev-1");
    assert!(snapshot.listen().ip().is_loopback());
    assert_eq!(
        snapshot
            .public_model("code-primary")
            .expect("public model is compiled")
            .serving_routes(),
        ["code-primary-openai-chat", "code-primary-openai-responses"]
    );

    let longcat = snapshot
        .public_model("LongCat-2.0")
        .expect("LongCat public model is compiled");
    assert_eq!(longcat.serving_routes().len(), 2);
    let target = snapshot
        .upstream_target("meituan-longcat-2")
        .expect("LongCat target is compiled");
    let chat = target.offering("chat").unwrap();
    assert_eq!(target.kind(), ProviderKind::Meituan);
    assert_eq!(chat.upstream_model(), "LongCat-2.0");
    assert_eq!(target.endpoint_base().as_str(), "https://api.longcat.chat/");
    assert_eq!(
        chat.model().context_length().input_tokens(),
        Some(1_048_756)
    );
    assert_eq!(chat.model().context_length().output_tokens(), Some(262_144));
    assert_eq!(chat.model().reasoning(), ReasoningSupport::Supported);
    assert!(
        chat.model()
            .supported_parameters()
            .iter()
            .any(|parameter| parameter == "tools")
    );
    assert!(
        chat.model()
            .supported_parameters()
            .iter()
            .any(|parameter| parameter == "reasoning")
    );

    for (protocol, body) in [
        (
            Protocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[]}"#,
        ),
        (
            Protocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello"}"#,
        ),
        (
            Protocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
        ),
        (
            Protocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello","tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}],"reasoning":{}}"#,
        ),
    ] {
        let body = bytes::Bytes::copy_from_slice(body.as_bytes());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&snapshot, &profile, body)
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(plan.upstream_target_id(), "meituan-longcat-2");
    }
}

#[test]
fn real_model_can_be_shared_by_targets_from_different_providers() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap = load_bootstrap(bootstrap).expect("checked-in bootstrap must remain valid");
    let mut definition = compiled_definition();
    let mut alternate = definition
        .upstream_targets
        .iter()
        .find(|target| target.id == "meituan-longcat-2")
        .expect("LongCat target is compiled")
        .clone();
    alternate.id = "openai-longcat-test".to_owned();
    alternate.provider = ProviderKind::OpenAi;
    alternate.credential.id = "openai-longcat-test".to_owned();
    alternate.credential.environment_variable = "OPENAI_API_KEY".to_owned();
    for offering in &mut alternate.offerings {
        offering.upstream_model = "meituan/longcat-2.0".to_owned();
        offering.endpoint_profile = "public-api".to_owned();
    }
    alternate.base_url = "https://api.openai.com".to_owned();
    definition.upstream_targets.push(alternate);

    let snapshot = build_registry(bootstrap, definition)
        .expect("different providers may reference one canonical model");
    let direct = snapshot
        .upstream_target("meituan-longcat-2")
        .expect("direct LongCat target exists")
        .offering("chat")
        .unwrap();
    let alternate = snapshot
        .upstream_target("openai-longcat-test")
        .expect("alternate provider target exists")
        .offering("chat")
        .unwrap();

    assert_eq!(direct.model().id(), longcat::MODEL_ID);
    assert_eq!(alternate.model().id(), longcat::MODEL_ID);
    assert_eq!(direct.model(), alternate.model());
}
