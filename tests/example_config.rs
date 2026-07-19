use openbridge::config::load_registry;

#[test]
fn checked_in_example_configuration_is_loadable() {
    let bootstrap = include_str!("../config/bootstrap.toml");
    let routes = include_str!("../config/routes.toml");

    let snapshot = load_registry(bootstrap, routes).expect("checked-in config must remain valid");

    assert_eq!(snapshot.version().as_str(), "dev-1");
    assert!(snapshot.listen().ip().is_loopback());
}
