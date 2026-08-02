use openbridge::{
    credential::{
        CredentialId, CredentialMetadata, CredentialSource, CredentialStoreError, CredentialType,
    },
    identity::UserConfiguration,
    provider::{CredentialKind, ProviderAdapter, ProviderKind},
};
use secrecy::SecretString;
use std::time::{Duration, SystemTime};

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
    let expires_at = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    credentials
        .insert_upstream(
            ProviderKind::OpenAi,
            "shared-id",
            SecretString::from("synthetic-upstream-secret"),
            CredentialMetadata::upstream(CredentialKind::ApiKey, CredentialSource::Environment)
                .with_generation(7)
                .with_expires_at(expires_at),
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
            .upstream(ProviderKind::LongCat, "shared-id", CredentialKind::ApiKey,)
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );
    assert_eq!(
        credentials
            .upstream(
                ProviderKind::OpenAi,
                "shared-id",
                CredentialKind::OAuth2BearerAccessToken,
            )
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );

    let upstream = credentials
        .upstream(ProviderKind::OpenAi, "shared-id", CredentialKind::ApiKey)
        .expect("matching upstream binding must resolve");
    assert_eq!(
        upstream.metadata().credential_type(),
        CredentialType::Upstream(CredentialKind::ApiKey)
    );
    assert_eq!(upstream.metadata().source(), CredentialSource::Environment);
    assert_eq!(upstream.metadata().generation(), 7);
    assert_eq!(upstream.metadata().expires_at(), Some(expires_at));
    let metadata = credentials.credential_metadata().collect::<Vec<_>>();
    assert_eq!(metadata.len(), 2);
    assert_eq!(
        metadata[0].1.credential_type(),
        CredentialType::DownstreamApiKey
    );
    assert_eq!(metadata[0].1.source(), CredentialSource::UserConfiguration);
    assert_eq!(metadata[1].1, upstream.metadata());
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
    assert!(!format!("{credentials:?} {upstream:?}").contains("OPENAI_API_KEY"));
}
