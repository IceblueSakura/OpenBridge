//! File activation and replay epoch rebinding, without inference traffic.
use super::*;
use crate::credential::{CredentialPool, CredentialRef};
fn setup() -> (tempfile::TempDir, CredentialManager) {
    let dir = crate::credential::test_support::private_directory();
    let manager = CredentialManager::new(dir.path(), vec![]).unwrap();
    manager.write_gateway_config_for_test(
        &serde_json::json!({"client_key":"synthetic-gateway-key-at-least-32-bytes"}),
    );
    manager
        .add_api_key(
            "deepseek",
            "one",
            Secret::new("synthetic-managed-key".into()).unwrap(),
        )
        .unwrap();
    manager
        .set_pool(
            "deepseek",
            "deepseek-api-key",
            0,
            CredentialPool {
                members: vec![CredentialRef::ApiKey {
                    alias: "one".into(),
                }],
                fallback: false,
                max_attempts: 1,
            },
        )
        .unwrap();
    (dir, manager)
}
#[test]
fn bootstrap_loads_selected_key_and_requires_reactivation_after_rotation() {
    let (dir, manager) = setup();
    let first = Bootstrap::from_directory(dir.path()).unwrap();
    let entry = &first.gateway.state.entries[&(
        super::super::family(Profile::Responses),
        "deepseek-flash".into(),
    )];
    let candidate = &entry.candidates[0];
    let (key, grant) = candidate.secret.resolve().unwrap();
    assert!(grant.is_none());
    let (_, request) = super::super::admission::prepare(
        &first.gateway.state,
        Profile::Responses,
        br#"{"model":"deepseek-flash","input":"synthetic request"}"#,
    )
    .unwrap();
    let prepared =
        crate::execution::prepare(&candidate.endpoint, &candidate.provider, &key, &request)
            .unwrap();
    assert_eq!(prepared.auth_header.1, "Bearer synthetic-managed-key");
    manager
        .replace_api_key(
            "deepseek",
            "one",
            1,
            Secret::new("synthetic-replacement".into()).unwrap(),
        )
        .unwrap();
    assert!(candidate.secret.resolve().is_err());
    let next = Bootstrap::from_directory(dir.path()).unwrap();
    let next = &next.gateway.state.entries[&(
        super::super::family(Profile::Responses),
        "deepseek-flash".into(),
    )];
    assert_ne!(
        entry.downstream.replay_origin,
        next.downstream.replay_origin
    );
}
#[test]
fn file_pools_reject_unknown_bindings_repeated_members_and_invalid_attempt_limits() {
    for (path, value) in [
        (
            "/pools/deepseek-api-key/config/max_attempts",
            serde_json::json!(0),
        ),
        (
            "/pools/deepseek-api-key/config/members",
            serde_json::json!([{"kind":"api_key","alias":"one"},{"kind":"api_key","alias":"one"}]),
        ),
        ("/provider", serde_json::json!("another-provider")),
    ] {
        let (dir, _) = setup();
        let file = dir.path().join("deepseek.json");
        let mut value_file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        *value_file.pointer_mut(path).unwrap() = value;
        std::fs::write(file, serde_json::to_vec(&value_file).unwrap()).unwrap();
        assert!(Bootstrap::from_directory(dir.path()).is_err());
    }
    let (dir, manager) = setup();
    manager
        .set_pool(
            "deepseek",
            "unknown",
            0,
            CredentialPool {
                members: vec![CredentialRef::ApiKey {
                    alias: "one".into(),
                }],
                fallback: true,
                max_attempts: 2,
            },
        )
        .unwrap();
    assert!(Bootstrap::from_directory(dir.path()).is_err());
}
#[test]
fn pool_candidates_have_distinct_ordinals_and_reject_opaque_replay_without_affinity() {
    let (dir, manager) = setup();
    manager
        .add_api_key(
            "deepseek",
            "two",
            Secret::new("synthetic-second".into()).unwrap(),
        )
        .unwrap();
    manager
        .set_pool(
            "deepseek",
            "deepseek-api-key",
            1,
            CredentialPool {
                members: vec![
                    CredentialRef::ApiKey {
                        alias: "two".into(),
                    },
                    CredentialRef::ApiKey {
                        alias: "one".into(),
                    },
                ],
                fallback: true,
                max_attempts: 2,
            },
        )
        .unwrap();
    let boot = Bootstrap::from_directory(dir.path()).unwrap();
    let entry = &boot.gateway.state.entries[&(
        super::super::family(Profile::Responses),
        "deepseek-flash".into(),
    )];
    let request = entry
        .client
        .decode_request(br#"{"model":"deepseek-flash","input":"test"}"#)
        .unwrap();
    let candidates = entry.eligible(&request).unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].endpoint.id, candidates[1].endpoint.id);
    assert_eq!(
        candidates[0].secret.resolve().unwrap().0.expose(),
        "synthetic-second"
    );
    assert_eq!(
        candidates[1].secret.resolve().unwrap().0.expose(),
        "synthetic-managed-key"
    );
    assert!(entry.client.adaptation.scope.is_none());
    let request = entry.client.decode_request(br#"{"model":"deepseek-flash","input":"test","include":["reasoning.encrypted_content"]}"#).unwrap();
    assert!(entry.eligible(&request).is_err());
    manager.write_gateway_config_for_test(&serde_json::json!({"client_key":"synthetic-gateway-key-at-least-32-bytes","max_attempts":1}));
    let capped = Bootstrap::from_directory(dir.path()).unwrap();
    assert_eq!(
        capped.gateway.state.entries[&(
            super::super::family(Profile::Responses),
            "deepseek-flash".into()
        )]
            .policy
            .max_attempts,
        1
    );
    assert_eq!(
        capped.gateway.state.entries[&(
            super::super::family(Profile::Responses),
            "deepseek-flash".into()
        )]
            .candidates
            .len(),
        1
    );
}
