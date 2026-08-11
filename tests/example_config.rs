//! Verifies that both checked-in Bootstrap profiles compile into a runnable registry.

use openbridge::{config::parse_bootstrap_config, providers::build_compiled_registry};

#[test]
fn checked_in_bootstrap_profiles_compile_into_the_runtime_registry() {
    for document in [
        include_str!("../config/bootstrap.toml"),
        include_str!("../config/bootstrap.example.toml"),
    ] {
        let bootstrap = parse_bootstrap_config(document)
            .expect("the checked-in Bootstrap profile must remain parseable");
        build_compiled_registry(bootstrap)
            .expect("the checked-in Bootstrap profile must compile into a runtime registry");
    }
}
