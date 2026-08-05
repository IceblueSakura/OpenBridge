//! Verifies private upstream credential TOML parsing, pool binding, and environment-variable isolation.

use openbridge::{
    credential::CredentialSource,
    providers::build_compiled_registry,
    upstream_credentials::{UpstreamCredentialConfigError, UpstreamCredentialConfiguration},
};

const UPSTREAM_CREDENTIALS: &str = r#"
schema_version = 1

[[credential_pools]]
id = "openai-primary"
api_keys = ["synthetic-openai-key-a", "synthetic-openai-key-b"]
"#;

#[test]
fn upstream_toml_loads_a_required_pool_without_environment_locators() {
    // Build a runtime registry that requires only the OpenAI pool.
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // Load the selected pool from private TOML and verify ordered members and source category.
    let configuration = UpstreamCredentialConfiguration::from_toml(UPSTREAM_CREDENTIALS).unwrap();
    let credentials = configuration
        .into_builder_for(&registry, ["openai-primary"])
        .unwrap()
        .build();
    let members = credentials
        .upstream_pool(
            openbridge::provider::ProviderKind::OpenAi,
            "openai-primary",
            openbridge::provider::CredentialKind::ApiKey,
        )
        .unwrap();
    assert_eq!(members[0].member_id(), "openai-primary#1");
    assert_eq!(members[1].member_id(), "openai-primary#2");
    assert_eq!(
        members[0].metadata().source(),
        CredentialSource::UpstreamConfiguration
    );
}

#[test]
fn upstream_toml_rejects_invalid_pool_documents_without_exposing_secrets() {
    // Cover empty pools, blank keys, duplicate keys, and duplicate pool IDs.
    let cases = [
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = []\n",
            UpstreamCredentialConfigError::EmptyPool {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"  \"]\n",
            UpstreamCredentialConfigError::BlankApiKey {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-a\", \"secret-a\"]\n",
            UpstreamCredentialConfigError::DuplicateApiKey {
                id: "openai-primary".to_owned(),
            },
        ),
        (
            "schema_version = 1\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-a\"]\n[[credential_pools]]\nid = \"openai-primary\"\napi_keys = [\"secret-b\"]\n",
            UpstreamCredentialConfigError::DuplicatePoolId {
                id: "openai-primary".to_owned(),
            },
        ),
    ];
    for (document, expected) in cases {
        let error = UpstreamCredentialConfiguration::from_toml(document).unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("secret-a"));
        assert!(!format!("{error:?} {error}").contains("secret-b"));
    }
}

#[test]
fn upstream_toml_rejects_unknown_and_missing_required_pools_before_insertion() {
    // Build the complete compile-time registry as the only valid pool list.
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // Reject a pool in TOML that is not registered in code.
    let unknown = UpstreamCredentialConfiguration::from_toml(
        "schema_version = 1\n[[credential_pools]]\nid = \"unknown\"\napi_keys = [\"secret\"]\n",
    )
        .unwrap();
    assert_eq!(
        unknown
            .into_builder_for(&registry, ["openai-primary"])
            .unwrap_err(),
        UpstreamCredentialConfigError::UnknownPool {
            id: "unknown".to_owned()
        }
    );

    // Reject a compile-time pool required by the caller but absent from TOML.
    let missing = UpstreamCredentialConfiguration::from_toml(UPSTREAM_CREDENTIALS).unwrap();
    assert_eq!(
        missing
            .into_builder_for(&registry, ["openai-primary", "mimo-primary"])
            .unwrap_err(),
        UpstreamCredentialConfigError::MissingPool {
            id: "mimo-primary".to_owned()
        }
    );
}
