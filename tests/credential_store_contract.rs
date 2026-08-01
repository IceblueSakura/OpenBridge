use openbridge::{
    credential::{CredentialId, CredentialStoreError},
    identity::UserConfiguration,
    provider::{ProviderAdapter, ProviderKind},
};
use secrecy::SecretString;

const USERS: &str = r#"
schema_version = 1

[[users]]
id = "shared-id"
name = "Alice"
api_key = "alice-downstream-api-key-00000001"
enabled = true

[[users]]
id = "disabled-user"
name = "Disabled User"
api_key = "disabled-user-api-key-0000000000"
enabled = false
"#;

#[test]
fn one_store_keeps_downstream_and_upstream_credentials_purpose_bound_and_redacted() {
    let configuration = UserConfiguration::from_toml(USERS).unwrap();
    let (users, mut credentials) = configuration.into_parts();
    credentials
        .insert_upstream(
            ProviderKind::OpenAi,
            "shared-id",
            SecretString::from("synthetic-upstream-secret"),
        )
        .unwrap();
    let credentials = credentials.build();

    let alice = users
        .authenticate(&credentials, "alice-downstream-api-key-00000001")
        .expect("enabled downstream credential must authenticate");
    assert_eq!(alice.id(), "shared-id");
    assert!(
        users
            .authenticate(&credentials, "disabled-user-api-key-0000000000")
            .is_none()
    );
    assert_eq!(
        credentials
            .upstream(ProviderKind::LongCat, "shared-id")
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );

    let upstream = credentials
        .upstream(ProviderKind::OpenAi, "shared-id")
        .expect("matching upstream binding must resolve");
    let headers = ProviderAdapter::for_kind(ProviderKind::OpenAi)
        .prepare_auth_headers(&upstream)
        .unwrap();
    assert!(headers.contains(http::header::AUTHORIZATION));
    assert_eq!(upstream.binding_id(), "shared-id");
    assert_eq!(
        credentials.credential_ids().collect::<Vec<_>>(),
        vec![
            &CredentialId::DownstreamUser {
                user_id: "shared-id".to_owned(),
            },
            &CredentialId::UpstreamBinding {
                binding_id: "shared-id".to_owned(),
                provider: ProviderKind::OpenAi,
            },
        ]
    );
    assert!(
        !format!("{credentials:?} {upstream:?} {headers:?}").contains("synthetic-upstream-secret")
    );
}
