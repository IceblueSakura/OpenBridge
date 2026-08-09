//! Contract tests for credential construction and runtime isolation.

use secrecy::SecretString;

use crate::core::{ExecutableResponsesState, ResponsesAffinity, StorageSupport};
use crate::credential::{
    CredentialMetadata, CredentialSource, CredentialStoreBuilder, CredentialStoreError,
};
use crate::provider::{CredentialKind, ProviderKind};

#[test]
fn state_bound_continuation_rejects_a_multi_member_pool() {
    // Enable continuation for the built-in OpenAI Responses API to create a real state-bound constraint.
    let mut definition = crate::providers::compiled_config();
    if let crate::registry::UpstreamApiCapabilities::Responses(capabilities) =
        &mut definition.upstream_targets[0].upstream_apis[1].capabilities
    {
        capabilities.state = ExecutableResponsesState::new(
            StorageSupport::Unsupported,
            ResponsesAffinity::TargetBoundContinuation,
        );
    }
    let bootstrap =
        crate::config::parse_bootstrap_config(include_str!("../../../config/bootstrap.toml"))
            .unwrap();
    let registry = crate::registry::build_registry(bootstrap, definition).unwrap();

    // Inject two members and verify startup fails closed instead of guessing key affinity per request.
    let mut credentials = CredentialStoreBuilder::new();
    for (index, secret) in ["key-a", "key-b"].into_iter().enumerate() {
        credentials
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "openai-primary",
                format!("openai-primary#{}", index + 1),
                SecretString::from(secret),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::Programmatic,
                ),
            )
            .unwrap();
    }
    assert_eq!(
        credentials.build().validate_registry(&registry),
        Err(CredentialStoreError::StatefulPoolHasMultipleMembers)
    );
}

#[test]
fn runtime_store_owns_a_redacted_snapshot_and_rejects_empty_upstream_secrets() {
    // Inject a startup-parsed secret and build the immutable runtime snapshot.
    let mut credentials = CredentialStoreBuilder::new();
    credentials
        .insert_upstream_member(
            ProviderKind::OpenAi,
            "openai-primary",
            "openai-primary#1",
            SecretString::from("startup-secret"),
            CredentialMetadata::upstream(
                CredentialKind::ApiKey,
                CredentialSource::UpstreamConfiguration,
            ),
        )
        .unwrap();

    // Verify that the runtime Store retains the startup snapshot and Debug output contains no plaintext.
    let credentials = credentials.build();
    let credential = credentials
        .upstream_pool(
            ProviderKind::OpenAi,
            "openai-primary",
            CredentialKind::ApiKey,
        )
        .unwrap()
        .remove(0);
    assert_eq!(credential.expose_secret(), "startup-secret");
    assert_eq!(
        credential.metadata().source(),
        CredentialSource::UpstreamConfiguration
    );
    assert_eq!(credential.metadata().generation(), 1);
    assert_eq!(credential.metadata().expires_at(), None);
    assert!(!format!("{credentials:?} {credential:?}").contains("startup-secret"));

    // Reject an empty upstream key so the error occurs outside the request path.
    let mut invalid = CredentialStoreBuilder::new();
    assert_eq!(
        invalid
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "empty",
                "empty#1",
                SecretString::from(String::new()),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::Programmatic,
                ),
            )
            .unwrap_err(),
        CredentialStoreError::Unavailable
    );
    assert_eq!(
        invalid
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "invalid-generation",
                "invalid-generation#1",
                SecretString::from("synthetic"),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::Programmatic,
                )
                .with_generation(0),
            )
            .unwrap_err(),
        CredentialStoreError::InvalidMetadata
    );
    assert_eq!(
        invalid
            .insert_upstream_member(
                ProviderKind::OpenAi,
                " ",
                "member",
                SecretString::from("synthetic"),
                CredentialMetadata::upstream(
                    CredentialKind::ApiKey,
                    CredentialSource::Programmatic,
                ),
            )
            .unwrap_err(),
        CredentialStoreError::InvalidPoolIdentity
    );
}

#[test]
fn builder_rejects_duplicate_bindings_and_invalid_upstream_metadata() {
    // Reject duplicate downstream identities and secrets even when the original user is disabled.
    let mut downstream = CredentialStoreBuilder::new();
    downstream
        .insert_downstream("user-a", SecretString::from("downstream-a"), false)
        .unwrap();
    assert_eq!(
        downstream
            .insert_downstream("user-a", SecretString::from("downstream-b"), true)
            .unwrap_err(),
        CredentialStoreError::DuplicateId
    );
    assert_eq!(
        downstream
            .insert_downstream("user-b", SecretString::from("downstream-a"), true)
            .unwrap_err(),
        CredentialStoreError::DuplicateDownstreamSecret
    );

    // Reject duplicate member identities and secrets within the same Provider-bound pool.
    let metadata =
        CredentialMetadata::upstream(CredentialKind::ApiKey, CredentialSource::Programmatic);
    let mut upstream = CredentialStoreBuilder::new();
    upstream
        .insert_upstream_member(
            ProviderKind::OpenAi,
            "pool-a",
            "pool-a#1",
            SecretString::from("upstream-a"),
            metadata,
        )
        .unwrap();
    assert_eq!(
        upstream
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool-a",
                "pool-a#1",
                SecretString::from("upstream-b"),
                metadata,
            )
            .unwrap_err(),
        CredentialStoreError::DuplicateId
    );
    assert_eq!(
        upstream
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool-a",
                "pool-a#2",
                SecretString::from("upstream-a"),
                metadata,
            )
            .unwrap_err(),
        CredentialStoreError::DuplicateUpstreamSecret
    );

    // Reject blank member IDs and metadata for the downstream credential purpose.
    assert_eq!(
        upstream
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool-b",
                " ",
                SecretString::from("upstream-b"),
                metadata,
            )
            .unwrap_err(),
        CredentialStoreError::InvalidPoolIdentity
    );
    assert_eq!(
        upstream
            .insert_upstream_member(
                ProviderKind::OpenAi,
                "pool-b",
                "pool-b#1",
                SecretString::from("upstream-b"),
                CredentialMetadata::downstream_user(),
            )
            .unwrap_err(),
        CredentialStoreError::InvalidMetadata
    );
}
