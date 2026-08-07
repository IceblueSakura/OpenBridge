//! Guarded OAuth2 reload, refresh, and publish transactions.

use std::{sync::Arc, time::SystemTime};

use secrecy::ExposeSecret;

use super::super::{
    document::{parse_auth_document, serialize_auth_document, validate_refreshed_tokens},
    storage::{OAuth2StorageError, version_for_document},
    transport::refresh::ChatGptRefreshTransport,
};
use super::credential::{ManagedOAuth2Credential, OAuth2RefreshOutcome, refresh_due_at};

/// Runs one in-process and cross-process guarded refresh using a replaceable transport.
pub(super) async fn refresh_provider_with<T>(
    credential: &Arc<ManagedOAuth2Credential>,
    transport: &T,
    now: SystemTime,
) -> OAuth2RefreshOutcome
where
    T: ChatGptRefreshTransport,
{
    refresh_provider_with_trigger(credential, transport, now, RefreshTrigger::Scheduled).await
}

/// Runs guarded 401 recovery with a replaceable transport for deterministic contract tests.
pub(super) async fn recover_after_unauthorized_with<T>(
    credential: &Arc<ManagedOAuth2Credential>,
    transport: &T,
    now: SystemTime,
    rejected_generation: u64,
) -> OAuth2RefreshOutcome
where
    T: ChatGptRefreshTransport,
{
    refresh_provider_with_trigger(
        credential,
        transport,
        now,
        RefreshTrigger::Unauthorized {
            rejected_generation,
        },
    )
    .await
}

/// Runs one guarded reload/refresh transaction for the selected lifecycle trigger.
async fn refresh_provider_with_trigger<T>(
    credential: &Arc<ManagedOAuth2Credential>,
    transport: &T,
    now: SystemTime,
    trigger: RefreshTrigger,
) -> OAuth2RefreshOutcome
where
    T: ChatGptRefreshTransport,
{
    // Merge concurrent callers for the same credential before acquiring the file lock.
    let _refresh_gate = credential.refresh_gate.lock().await;
    if let Some(outcome) = credential.current_terminal_or_backoff(now) {
        return outcome;
    }
    if let RefreshTrigger::Unauthorized {
        rejected_generation,
    } = trigger
        && credential.current_generation() != rejected_generation
    {
        return OAuth2RefreshOutcome::Current {
            generation: credential.current_generation(),
        };
    }

    // Acquire the cross-process lock and reload the complete persisted source under it.
    let target = credential.target.clone();
    let locked_source = tokio::task::spawn_blocking(move || {
        let locked = target.lock()?;
        let document = locked.read_document()?;
        let version = version_for_document(&document);
        Ok::<_, OAuth2StorageError>((locked, document, version))
    })
    .await;
    let (locked, document, version) = match locked_source {
        Ok(Ok(source)) => source,
        Ok(Err(_)) | Err(_) => return credential.record_storage_failure(now),
    };
    let persisted = match parse_auth_document(&document, false) {
        Ok(bundle) => bundle,
        Err(_) => return credential.record_reauth_required(),
    };

    // Reuse an externally or concurrently published generation before considering a grant.
    if let RefreshTrigger::Unauthorized {
        rejected_generation,
    } = trigger
    {
        let (current_generation, current_version) = credential.current_generation_and_version();
        if current_generation != rejected_generation {
            return OAuth2RefreshOutcome::Current {
                generation: current_generation,
            };
        }
        if current_version != version {
            return credential.publish_current_if_changed(persisted, version);
        }
    } else if refresh_due_at(&credential.pool_id, persisted.expires_at) > now {
        // Skip the scheduled network refresh while the persisted token remains outside the safety window.
        return credential.publish_current_if_changed(persisted, version);
    }

    // Send exactly one refresh grant while retaining the cross-process rotation lease.
    let response = match transport.refresh(&persisted.refresh_token).await {
        Ok(response) => response,
        Err(error) => return credential.record_transport_failure(error, now),
    };
    if response.validate_token_type().is_err() {
        return credential.record_ambiguous();
    }
    let refreshed = match validate_refreshed_tokens(
        &persisted,
        response
            .id_token
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret),
        response.access_token.expose_secret(),
        response
            .refresh_token
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret),
    ) {
        Ok(bundle) => bundle,
        Err(_) => return credential.record_ambiguous(),
    };
    let serialized = match serialize_auth_document(&refreshed) {
        Ok(document) => document,
        Err(_) => return credential.record_ambiguous(),
    };
    let next_version = version_for_document(&serialized);

    // Persist the complete rotation in the held transaction before publishing memory state.
    let write = tokio::task::spawn_blocking(move || locked.replace(&serialized)).await;
    match write {
        Ok(Ok(())) => credential.publish_refreshed(refreshed, next_version),
        Ok(Err(_)) | Err(_) => credential.record_ambiguous(),
    }
}

#[derive(Clone, Copy)]
enum RefreshTrigger {
    Scheduled,
    Unauthorized { rejected_generation: u64 },
}
