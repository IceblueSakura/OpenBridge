//! Trusted credential sources; no account selection from business data.
use super::StartupError;
use crate::{
    credential::{AccessBinding, AccessGrant},
    provider::{CredentialBindingId, CredentialKind, SecretMaterial},
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub(super) enum Source {
    ApiKey(Arc<SecretMaterial>),
    Account(AccessBinding),
}
#[derive(Default)]
pub struct Credentials(BTreeMap<CredentialBindingId, Source>);
impl From<BTreeMap<CredentialBindingId, Arc<SecretMaterial>>> for Credentials {
    fn from(keys: BTreeMap<CredentialBindingId, Arc<SecretMaterial>>) -> Self {
        Self(
            keys.into_iter()
                .map(|(id, key)| (id, Source::ApiKey(key)))
                .collect(),
        )
    }
}
impl Credentials {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, id: CredentialBindingId, secret: Arc<SecretMaterial>) {
        self.0.insert(id, Source::ApiKey(secret));
    }
    pub fn insert_account(&mut self, id: CredentialBindingId, binding: AccessBinding) {
        self.0.insert(id, Source::Account(binding));
    }
    pub(super) fn get(&self, id: &CredentialBindingId) -> Option<&Source> {
        self.0.get(id)
    }
}
fn check_profile(kind: CredentialKind, profile: Option<&str>) -> Result<(), StartupError> {
    match (kind, profile) {
        (CredentialKind::ApiKey, None) => Ok(()),
        (CredentialKind::OAuth(expected), Some(actual)) if actual == expected => Ok(()),
        _ => Err(StartupError::Credentials),
    }
}

#[cfg(test)]
#[test]
fn credential_kind_and_authorization_profile_must_both_match() {
    for (kind, profile, accepted) in [
        (CredentialKind::ApiKey, None, true),
        (CredentialKind::ApiKey, Some("first"), false),
        (CredentialKind::OAuth("first"), None, false),
        (CredentialKind::OAuth("first"), Some("first"), true),
        (CredentialKind::OAuth("first"), Some("second"), false),
    ] {
        assert_eq!(check_profile(kind, profile).is_ok(), accepted);
    }
}

impl Source {
    pub(super) fn check(&self, kind: CredentialKind) -> Result<(), StartupError> {
        let profile = match self {
            Self::ApiKey(_) => None,
            Self::Account(binding) => Some(binding.profile()),
        };
        check_profile(kind, profile)
    }
    pub(super) fn provenance(&self) -> String {
        match self {
            Self::ApiKey(secret) => secret.expose().into(),
            Self::Account(binding) => binding.provenance(),
        }
    }
    pub(super) fn resolve(
        &self,
    ) -> Result<(Arc<SecretMaterial>, Option<AccessGrant>), super::ApiError> {
        match self {
            Self::ApiKey(secret) => Ok((secret.clone(), None)),
            Self::Account(binding) => {
                let grant = binding.borrow().map_err(|_| {
                    super::ApiError::new(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "credential_unavailable",
                    )
                })?;
                let secret = SecretMaterial::new(grant.access.expose())
                    .map_err(|_| super::ApiError::upstream())?;
                Ok((Arc::new(secret), Some(grant)))
            }
        }
    }
    #[cfg(test)]
    pub(super) fn static_key(&self) -> &SecretMaterial {
        match self {
            Self::ApiKey(secret) => secret,
            _ => panic!("expected static test key"),
        }
    }
}
