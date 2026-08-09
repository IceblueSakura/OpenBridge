//! Verifies credential merging, purpose isolation, uniqueness, and continuation affinity.

mod support;

use openbridge::{
    core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport},
    credential::{
        CredentialId, CredentialMetadata, CredentialSource, CredentialStoreError, CredentialType,
    },
    identity::UserConfiguration,
    provider::{CredentialKind, ProviderAdapter, ProviderKind},
    registry::{UpstreamApiCapabilities, build_registry},
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
fn continuation_pool_rejects_multiple_members_without_a_public_model() {
    // Enable continuation on the executable API while removing every Public Model and Route.
    let mut definition =
        support::definition("credential-continuation", "unused-public-model", "upstream");
    let UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    else {
        panic!("second synthetic API must be Responses");
    };
    capabilities.state = ExecutableResponsesState::new(
        StorageSupport::Unsupported,
        ResponsesAffinity::TargetBoundContinuation,
    );
    definition.public_models.clear();
    definition.routes.clear();
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();

    // Validate the executable Target constraint directly, without relying on a public projection.
    let (_, credentials) = support::users_and_credential_pool(
        "downstream-token-0000000000000000",
        &registry,
        &["key-a", "key-b"],
    );
    assert_eq!(
        credentials.validate_registry(&registry),
        Err(CredentialStoreError::StatefulPoolHasMultipleMembers)
    );
}

#[test]
fn ordinary_target_bound_pool_accepts_multiple_members() {
    // Keep the fixture's ordinary Target-bound Responses state without enabling continuation.
    let mut definition =
        support::definition("credential-target-bound", "unused-public-model", "upstream");
    definition.public_models.clear();
    definition.routes.clear();
    let registry = build_registry(support::bootstrap(support::BOOTSTRAP), definition).unwrap();

    // Storage-independent Target affinity alone must not disable credential rotation.
    let (_, credentials) = support::users_and_credential_pool(
        "downstream-token-0000000000000000",
        &registry,
        &["key-a", "key-b"],
    );
    credentials
        .validate_registry(&registry)
        .expect("ordinary Target-bound state must permit multiple credential members");
}

#[test]
fn one_store_keeps_downstream_and_upstream_credentials_purpose_bound_and_redacted() {
    let configuration = UserConfiguration::from_toml(USERS).unwrap();
    let (users, mut credentials) = configuration.into_parts();
    let expires_at = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
    credentials
        .insert_upstream_member(
            ProviderKind::OpenAi,
            "shared-id",
            "shared-id#1",
            SecretString::from("synthetic-upstream-secret"),
            CredentialMetadata::upstream(
                CredentialKind::ApiKey,
                CredentialSource::UpstreamConfiguration,
            )
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
            .upstream_pool(ProviderKind::LongCat, "shared-id", CredentialKind::ApiKey,)
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );
    assert_eq!(
        credentials
            .upstream_pool(
                ProviderKind::OpenAi,
                "shared-id",
                CredentialKind::OAuth2BearerAccessToken,
            )
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );

    let upstream = credentials
        .upstream_pool(ProviderKind::OpenAi, "shared-id", CredentialKind::ApiKey)
        .expect("matching upstream pool must resolve")
        .remove(0);
    assert_eq!(
        upstream.metadata().credential_type(),
        CredentialType::Upstream(CredentialKind::ApiKey)
    );
    assert_eq!(
        upstream.metadata().source(),
        CredentialSource::UpstreamConfiguration
    );
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
    assert_eq!(upstream.pool_id(), "shared-id");
    assert_eq!(upstream.member_id(), "shared-id#1");
    assert_eq!(
        credentials.credential_ids().collect::<Vec<_>>(),
        vec![
            &CredentialId::DownstreamUser {
                user_id: "shared-id".to_owned(),
            },
            &CredentialId::UpstreamPoolMember {
                pool_id: "shared-id".to_owned(),
                member_id: "shared-id#1".to_owned(),
                provider: ProviderKind::OpenAi,
            },
        ]
    );
    assert!(
        !format!("{credentials:?} {upstream:?} {headers:?}").contains("synthetic-upstream-secret")
    );
    assert!(!format!("{credentials:?} {upstream:?}").contains("OPENAI_API_KEY"));
}
