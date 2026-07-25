use openbridge::{
    config::load_bootstrap,
    core::Protocol,
    models::longcat,
    pipeline::prepare_native_request,
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
            .alias("code-primary")
            .expect("public alias is compiled")
            .candidates(),
        ["openai-main"]
    );

    let longcat = snapshot
        .alias("LongCat-2.0")
        .expect("LongCat public alias is compiled");
    assert_eq!(longcat.candidates(), ["meituan-longcat-2"]);
    let deployment = snapshot
        .deployment("meituan-longcat-2")
        .expect("LongCat deployment is compiled");
    assert_eq!(deployment.provider_id(), "meituan");
    assert_eq!(deployment.upstream_model(), "LongCat-2.0");
    assert_eq!(
        deployment.endpoint_base().as_str(),
        "https://api.longcat.chat/"
    );
    assert_eq!(
        deployment.model().context_length().input_tokens(),
        Some(1_048_756)
    );
    assert_eq!(
        deployment.model().context_length().output_tokens(),
        Some(262_144)
    );
    assert_eq!(deployment.model().reasoning(), ReasoningSupport::Supported);
    assert!(
        deployment
            .model()
            .supported_parameters()
            .iter()
            .any(|parameter| parameter == "tools")
    );
    assert!(
        deployment
            .model()
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
        let prepared = prepare_native_request(&snapshot, protocol, body.as_bytes().to_vec().into())
            .expect("LongCat should remain on the native path for both protocols");
        assert_eq!(prepared.deployment_id(), "meituan-longcat-2");
    }
}

#[test]
fn canonical_model_can_be_shared_by_deployments_from_different_providers() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let bootstrap = load_bootstrap(bootstrap).expect("checked-in bootstrap must remain valid");
    let mut definition = compiled_definition();
    let mut alternate = definition
        .deployments
        .iter()
        .find(|deployment| deployment.id == "meituan-longcat-2")
        .expect("LongCat deployment is compiled")
        .clone();
    alternate.id = "openai-longcat-test".to_owned();
    alternate.provider = "openai".to_owned();
    alternate.upstream_model = "meituan/longcat-2.0".to_owned();
    alternate.endpoint_profile = "public-api".to_owned();
    alternate.base_url = "https://api.openai.com".to_owned();
    definition.deployments.push(alternate);

    let snapshot = build_registry(bootstrap, definition)
        .expect("different providers may reference one canonical model");
    let direct = snapshot
        .deployment("meituan-longcat-2")
        .expect("direct LongCat deployment exists");
    let alternate = snapshot
        .deployment("openai-longcat-test")
        .expect("alternate provider deployment exists");

    assert_eq!(direct.model().id(), longcat::MODEL_ID);
    assert_eq!(alternate.model().id(), longcat::MODEL_ID);
    assert_eq!(direct.model(), alternate.model());
    assert_eq!(direct.provider_id(), "meituan");
    assert_eq!(alternate.provider_id(), "openai");
}
