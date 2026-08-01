use openbridge::{
    config::parse_bootstrap_config,
    core::ApiProtocol,
    identity::UserConfigPath,
    models::longcat,
    pipeline::{analyze_request, plan_request},
    provider::ProviderKind,
    providers::{build_compiled_registry, compiled_config},
    registry::{ReasoningSupport, build_registry},
};

#[test]
fn checked_in_bootstrap_and_compiled_registry_are_loadable() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap =
        parse_bootstrap_config(bootstrap).expect("checked-in bootstrap must remain valid");
    let registry =
        build_compiled_registry(bootstrap).expect("compiled registry must remain internally valid");

    assert_eq!(registry.version().as_str(), "dev-1");
    assert!(registry.listen().ip().is_loopback());
    let users = UserConfigPath::new("config/users.example.toml")
        .load()
        .expect("checked-in user example must remain valid");
    assert_eq!(users.users().next().unwrap().id(), "local-user");
    assert_eq!(
        registry
            .public_model("code-primary")
            .expect("public model is compiled")
            .routes(),
        ["code-primary-openai-chat", "code-primary-openai-responses"]
    );

    let longcat = registry
        .public_model("LongCat-2.0")
        .expect("LongCat public model is compiled");
    assert_eq!(longcat.routes().len(), 2);
    let target = registry
        .upstream_target("longcat-2")
        .expect("LongCat target is compiled");
    let chat = target.upstream_api("chat").unwrap();
    assert_eq!(target.kind(), ProviderKind::LongCat);
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
            ApiProtocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[]}"#,
        ),
        (
            ApiProtocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello"}"#,
        ),
        (
            ApiProtocol::ChatCompletions,
            r#"{"model":"LongCat-2.0","messages":[],"tools":[{"type":"function","function":{"name":"probe"}}]}"#,
        ),
        (
            ApiProtocol::Responses,
            r#"{"model":"LongCat-2.0","input":"hello","tools":[{"type":"function","name":"probe","parameters":{"type":"object"}}],"reasoning":{}}"#,
        ),
    ] {
        let body = bytes::Bytes::copy_from_slice(body.as_bytes());
        let profile = analyze_request(protocol, &body).unwrap();
        let plan = plan_request(&registry, &profile, body)
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(plan.upstream_target_id(), "longcat-2");
    }
}

#[test]
fn real_model_can_be_shared_by_targets_from_different_providers() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap =
        parse_bootstrap_config(bootstrap).expect("checked-in bootstrap must remain valid");
    let mut definition = compiled_config();
    let mut alternate = definition
        .upstream_targets
        .iter()
        .find(|target| target.id == "longcat-2")
        .expect("LongCat target is compiled")
        .clone();
    alternate.id = "openai-longcat-test".to_owned();
    alternate.provider = ProviderKind::OpenAi;
    alternate.credential.id = "openai-longcat-test".to_owned();
    alternate.credential.environment_variable = "OPENAI_API_KEY".to_owned();
    for upstream_api in &mut alternate.upstream_apis {
        upstream_api.upstream_model = "longcat/longcat-2.0".to_owned();
        upstream_api.endpoint_profile = "public-api".to_owned();
    }
    alternate.base_url = "https://api.openai.com".to_owned();
    definition.upstream_targets.push(alternate);

    let registry = build_registry(bootstrap, definition)
        .expect("different providers may reference one canonical model");
    let direct = registry
        .upstream_target("longcat-2")
        .expect("direct LongCat target exists")
        .upstream_api("chat")
        .unwrap();
    let alternate = registry
        .upstream_target("openai-longcat-test")
        .expect("alternate provider target exists")
        .upstream_api("chat")
        .unwrap();

    assert_eq!(direct.model().id(), longcat::MODEL_ID);
    assert_eq!(alternate.model().id(), longcat::MODEL_ID);
    assert_eq!(direct.model(), alternate.model());
}
