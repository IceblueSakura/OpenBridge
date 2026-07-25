use openbridge::{config::load_bootstrap, providers::build_compiled_registry};

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
}
