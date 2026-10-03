//! Profile-neutral lifecycle coordination; network waits never hold store transactions.
use super::{
    AuthDriver, CredentialError as Error, LoginOptions, LoginPrompt,
    model::{
        AccessState, Account, AccountState, AccountStatus, Grant, LogoutOutcome, random_id,
        valid_alias, valid_client, valid_profile,
    },
    store::{Loaded, Store, Transaction},
};
use std::{collections::BTreeMap, path::Path, sync::Arc};

#[derive(Clone)]
pub struct CredentialManager {
    pub(super) store: Arc<Store>,
    pub(super) keys: Arc<Store>,
    drivers: BTreeMap<String, Arc<dyn AuthDriver>>,
}
impl CredentialManager {
    #[cfg(test)]
    pub(crate) fn write_gateway_config_for_test(&self, value: &serde_json::Value) {
        self.store.test_config(value);
    }
    pub fn new(root: impl AsRef<Path>, drivers: Vec<Arc<dyn AuthDriver>>) -> Result<Self, Error> {
        if drivers.len() > 16 {
            return Err(Error::InvalidInput);
        }
        let mut registry = BTreeMap::new();
        for driver in drivers {
            valid_profile(driver.profile())?;
            if driver.profile() == "api-keys" {
                return Err(Error::InvalidInput);
            }
            if registry
                .insert(driver.profile().to_owned(), driver)
                .is_some()
            {
                return Err(Error::InvalidInput);
            }
        }
        let store = Store::open(root.as_ref())?;
        let keys = store.key_partition();
        Ok(Self {
            store,
            keys,
            drivers: registry,
        })
    }
    fn driver(&self, profile: &str) -> Result<&dyn AuthDriver, Error> {
        self.drivers
            .get(profile)
            .map(AsRef::as_ref)
            .ok_or(Error::UnknownProfile)
    }
    pub(super) fn load(
        &self,
        tx: &Transaction<'_>,
        profile: &str,
        alias: &str,
    ) -> Result<Loaded, Error> {
        let mut loaded = tx.load(profile, alias)?;
        if let Some(account) = &mut loaded.account {
            if loaded.recovery {
                account.credential = None;
                account.state = AccountState::NeedsReauthorization;
                account.login_attempt = None;
            }
            self.driver(profile)?.validate(
                &account.client_id,
                account.identity.as_ref(),
                account.credential.as_ref(),
            )?;
        }
        Ok(loaded)
    }
    /// Redacted inventory; each partition is read under its own short transaction.
    pub fn inventory(&self) -> Result<Vec<super::CredentialStatus>, Error> {
        let mut statuses: Vec<_> = self
            .list(None)?
            .into_iter()
            .map(super::CredentialStatus::OAuth)
            .collect();
        statuses.extend(
            self.list_api_keys(None)?
                .into_iter()
                .map(super::CredentialStatus::ApiKey),
        );
        Ok(statuses)
    }
    pub fn list(&self, selected: Option<&str>) -> Result<Vec<AccountStatus>, Error> {
        if let Some(profile) = selected {
            self.driver(profile)?;
        }
        let tx = self.store.transaction()?;
        let mut statuses = Vec::new();
        for profile in self
            .drivers
            .keys()
            .filter(|id| selected.is_none_or(|s| s == *id))
        {
            for alias in tx.aliases(profile)? {
                let loaded = self.load(&tx, profile, &alias)?;
                if let Some(account) = loaded.account {
                    statuses.push(account.status(loaded.recovery)?);
                } else if loaded.recovery {
                    statuses.push(AccountStatus {
                        profile: profile.clone(),
                        account: alias,
                        state: AccountState::NeedsReauthorization,
                        access: AccessState::Unavailable,
                        revision: 0,
                        generation: 0,
                        expires_at: None,
                        login_pending: false,
                        recovery_required: true,
                    });
                }
            }
        }
        Ok(statuses)
    }
    pub async fn login(
        &self,
        profile: &str,
        alias: &str,
        options: LoginOptions,
        notify: impl Fn(&LoginPrompt) + Send + Sync,
    ) -> Result<AccountStatus, Error> {
        valid_alias(alias)?;
        let driver = self.driver(profile)?;
        let client = driver.login_client(&options)?;
        valid_client(&client)?;
        let mut ticket = {
            let _account_lock = self.store.account_lock(profile, alias)?;
            let tx = self.store.transaction()?;
            let mut account = self
                .load(&tx, profile, alias)?
                .account
                .unwrap_or_else(|| Account::new(profile, alias, &client));
            if account.client_id != client {
                return Err(Error::IdentityMismatch);
            }
            let id = random_id()?;
            account.login_attempt = Some(id.clone());
            account.advance()?;
            tx.publish(&account)?;
            LoginTicket {
                store: self.store.clone(),
                profile: profile.into(),
                alias: alias.into(),
                client,
                id,
                complete: false,
            }
        };
        let grant = driver.login(&ticket.client, &options, &notify).await?;
        let _account_lock = self.store.account_lock(profile, alias)?;
        let tx = self.store.transaction()?;
        let loaded = self.load(&tx, profile, alias)?;
        let mut account = loaded.account.ok_or(Error::Superseded)?;
        if loaded.recovery || account.login_attempt.as_deref() != Some(&ticket.id) {
            return Err(Error::Superseded);
        }
        if account.client_id != ticket.client
            || account
                .identity
                .as_ref()
                .is_some_and(|i| i != &grant.identity)
        {
            return Err(Error::IdentityMismatch);
        }
        self.validate_grant(driver, &account.client_id, &grant)?;
        for other in tx.aliases(profile)? {
            if other != alias
                && let Some(other) = self.load(&tx, profile, &other)?.account
                && other.client_id == account.client_id
                && other.identity.as_ref() == Some(&grant.identity)
            {
                return Err(Error::IdentityMismatch);
            }
        }
        account.identity = Some(grant.identity);
        account.replace_credential(Some(grant.credential))?;
        account.state = AccountState::Active;
        account.login_attempt = None;
        account.advance()?;
        tx.publish(&account)?;
        ticket.complete = true;
        account.status(false)
    }
    fn validate_grant(
        &self,
        driver: &dyn AuthDriver,
        client: &str,
        grant: &Grant,
    ) -> Result<(), Error> {
        if !grant.identity.valid() || !grant.credential.valid() {
            return Err(Error::Protocol);
        }
        driver.validate(client, Some(&grant.identity), Some(&grant.credential))
    }
    pub async fn refresh(&self, profile: &str, alias: &str) -> Result<AccountStatus, Error> {
        let driver = self.driver(profile)?;
        if !driver.can_refresh() {
            return Err(Error::Unsupported);
        }
        let _account_lock = self.store.account_lock(profile, alias)?;
        let (mut account, previous) = {
            let tx = self.store.transaction()?;
            let mut account = self
                .load(&tx, profile, alias)?
                .account
                .ok_or(Error::LoginRequired)?;
            let previous = account.credential.take().ok_or(Error::LoginRequired)?;
            if previous.refresh.is_none() {
                return Err(Error::LoginRequired);
            }
            account.replace_credential(None)?;
            account.state = AccountState::NeedsReauthorization;
            account.login_attempt = None;
            account.advance()?;
            tx.publish(&account)?;
            (account, previous)
        };
        let expected = account.identity.as_ref().ok_or(Error::Storage)?;
        let grant = driver
            .refresh(&account.client_id, expected, &previous)
            .await?;
        self.validate_grant(driver, &account.client_id, &grant)?;
        if &grant.identity != expected {
            return Err(Error::IdentityMismatch);
        }
        let tx = self.store.transaction()?;
        let loaded = self.load(&tx, profile, alias)?;
        let current = loaded.account.ok_or(Error::Superseded)?;
        if loaded.recovery
            || current.revision != account.revision
            || current.client_id != account.client_id
            || current.identity != account.identity
            || current.login_attempt.is_some()
        {
            return Err(Error::Superseded);
        }
        account.replace_credential(Some(grant.credential))?;
        account.state = AccountState::Active;
        account.advance()?;
        tx.publish(&account)?;
        account.status(false)
    }
    pub async fn logout(
        &self,
        profile: &str,
        alias: &str,
        revoke: bool,
    ) -> Result<LogoutOutcome, Error> {
        let driver = self.driver(profile)?;
        let _account_lock = self.store.account_lock(profile, alias)?;
        let (client, previous) = {
            let tx = self.store.transaction()?;
            let loaded = self.load(&tx, profile, alias)?;
            let Some(mut account) = loaded.account else {
                if loaded.recovery {
                    tx.clear_unpublished(profile, alias)?;
                    return Ok(if revoke {
                        LogoutOutcome::RevocationUnconfirmed
                    } else {
                        LogoutOutcome::LocalOnly
                    });
                }
                return Ok(LogoutOutcome::AlreadySignedOut);
            };
            let previous = account.credential.take();
            account.replace_credential(None)?;
            account.state = AccountState::SignedOut;
            account.login_attempt = None;
            account.advance()?;
            tx.publish(&account)?;
            (account.client_id, previous)
        };
        if !revoke {
            return Ok(LogoutOutcome::LocalOnly);
        }
        let Some(previous) = previous else {
            return Ok(LogoutOutcome::RevocationUnconfirmed);
        };
        Ok(if driver.revoke(&client, &previous).await.is_ok() {
            LogoutOutcome::Revoked
        } else {
            LogoutOutcome::RevocationUnconfirmed
        })
    }
}
struct LoginTicket {
    store: Arc<Store>,
    profile: String,
    alias: String,
    client: String,
    id: String,
    complete: bool,
}
impl Drop for LoginTicket {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        // Best effort, only this ticket; never clear a crash/publication quarantine.
        let Ok(_account_lock) = self.store.account_lock(&self.profile, &self.alias) else {
            return;
        };
        let Ok(tx) = self.store.transaction() else {
            return;
        };
        let Ok(loaded) = tx.load(&self.profile, &self.alias) else {
            return;
        };
        if !loaded.recovery
            && let Some(mut account) = loaded.account
            && account.login_attempt.as_deref() == Some(&self.id)
        {
            account.login_attempt = None;
            if account.advance().is_ok() {
                let _ = tx.publish(&account);
            }
        }
    }
}
