//! Verifies checked-in bootstrap examples, credential pools, and compiled registry consistency.

use super::*;

#[test]
fn checked_in_examples_compile_into_a_closed_runtime_registry() {
    // Parse the active and example bootstrap documents as one maintained process policy.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml"))
        .expect("checked-in bootstrap must remain valid");
    let template = parse_bootstrap_config(include_str!("../../config/bootstrap.example.toml"))
        .expect("checked-in bootstrap template must remain valid");
    assert_eq!(template, bootstrap);
    assert!(bootstrap.listen().ip().is_loopback());
    let registry =
        build_compiled_registry(bootstrap).expect("compiled registry must remain internally valid");
    let users = UserConfigPath::new("config/users.example.toml")
        .load()
        .expect("checked-in user example must remain valid");
    assert!(users.users().users().next().is_some());

    // Resolve every published Route through a trusted Target and one declared Upstream API.
    let mut public_model_count = 0;
    for public_model in registry.public_models() {
        public_model_count += 1;
        assert!(
            !public_model.routes().is_empty(),
            "{} has no executable Route",
            public_model.standard().id()
        );
        for route_id in public_model.routes() {
            let route = registry
                .route(route_id)
                .expect("published Route must resolve");
            let target = registry
                .upstream_target(route.upstream_target())
                .expect("published Route Target must resolve");
            assert!(target.enabled(), "{} is not selectable", target.id());
            assert!(
                target.upstream_api(route.upstream_operation()).is_some(),
                "{route_id} references an unavailable Upstream API"
            );
        }
    }
    assert!(public_model_count > 0);

    // Keep every compiled Target on HTTPS and bound to a declared credential pool.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        assert_eq!(target.endpoint_base().scheme(), "https", "{target_id}");
        assert!(
            registry
                .credential_pool(target.credential_pool_id())
                .is_some(),
            "{target_id} has no credential pool"
        );
    }
}

#[test]
fn compiled_provider_credential_pools_are_shared_and_match_the_private_toml_example() {
    // Build the registry and load only API-key pools from a TOML template with no real values.
    let bootstrap = parse_bootstrap_config(include_str!("../../config/bootstrap.toml")).unwrap();
    let registry = build_compiled_registry(bootstrap).expect("compiled registry should be valid");
    let pool_ids = registry
        .credential_pool_ids()
        .filter(|pool_id| {
            registry
                .credential_pool(pool_id)
                .is_some_and(|pool| pool.kind() == CredentialKind::ApiKey)
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let credentials = UpstreamCredentialConfiguration::from_toml(include_str!(
        "../../config/upstream-credentials.example.toml"
    ))
    .unwrap()
    .into_builder_for(&registry, pool_ids.iter().map(String::as_str))
    .unwrap()
    .build();

    // Verify that each API-key target retrieves the template credential by Provider and pool.
    for target_id in registry.upstream_target_ids() {
        let target = registry.upstream_target(target_id).unwrap();
        let pool = registry
            .credential_pool(target.credential_pool_id())
            .unwrap();
        if pool.kind() != CredentialKind::ApiKey {
            continue;
        }
        assert!(
            credentials
                .upstream_pool(target.kind(), target.credential_pool_id(), pool.kind(),)
                .is_ok()
        );
    }

    // Keep the ChatGPT OAuth pool outside the immutable API-key credential snapshot.
    assert!(
        credentials
            .upstream_pool(
                ProviderKind::ChatGpt,
                "chatgpt-codex",
                CredentialKind::OAuth2BearerAccessToken,
            )
            .is_err()
    );
}
