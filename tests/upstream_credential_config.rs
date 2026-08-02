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
    // 构造仅要求 OpenAI pool 的运行时注册表。
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // 从私有 TOML 加载指定 pool，并验证有序成员和来源类别。
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
    // 覆盖空 pool、空白 key、重复 key 和重复 pool id。
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
    // 构造完整编译期注册表，作为唯一合法 pool 清单。
    let bootstrap =
        openbridge::config::parse_bootstrap_config(include_str!("../config/bootstrap.toml"))
            .unwrap();
    let registry = build_compiled_registry(bootstrap).unwrap();

    // 拒绝 TOML 中未由代码注册的 pool。
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

    // 拒绝调用方要求但 TOML 未配置的编译期 pool。
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
