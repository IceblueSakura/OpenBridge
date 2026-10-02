//! Account documents and short store transactions; no authorization protocol policy.
use super::{
    CredentialError as Error,
    model::{Account, random_id, valid_alias, valid_profile},
    storage_fs::Directory,
};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};
const ACCOUNT_BYTES: u64 = 128 * 1024;
const ACCOUNTS_PER_PROFILE: usize = 64;

pub(super) struct Store {
    root: Directory,
    #[cfg(test)]
    fault: std::sync::atomic::AtomicU8,
}
pub(super) struct Transaction<'a> {
    store: &'a Store,
    _lock: File,
}
pub(super) struct Loaded {
    pub account: Option<Account>,
    pub recovery: bool,
}
#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum PublishStep {
    Write = 1,
    FileSync,
    Rename,
    DirectorySync,
    Cleanup,
    MarkerSync,
    MarkerDirectorySync,
    CleanupDirectorySync,
}
impl Store {
    pub fn open(root: &Path) -> Result<Arc<Self>, Error> {
        let store = Arc::new(Self {
            root: Directory::open(root)?,
            #[cfg(test)]
            fault: std::sync::atomic::AtomicU8::new(0),
        });
        let _ = store.transaction()?;
        Ok(store)
    }
    pub fn transaction(&self) -> Result<Transaction<'_>, Error> {
        // Detect obsolete snapshots without reading or converting their secrets.
        if self.root.exists("accounts.json")? {
            return Err(Error::LegacyStore);
        }
        Ok(Transaction {
            store: self,
            _lock: self.root.lock("store.lock")?,
        })
    }
    pub fn account_lock(&self, profile: &str, alias: &str) -> Result<File, Error> {
        valid_profile(profile)?;
        valid_alias(alias)?;
        let tx = self.transaction()?;
        let directory = tx.directory(profile, true)?.ok_or(Error::Storage)?;
        let name = format!("{alias}.lock");
        if !directory.exists(&name)? && directory.names()?.len() >= 256 {
            return Err(Error::PoolFull);
        }
        let lock = directory.lock(&name)?;
        drop(tx);
        Ok(lock)
    }
    #[cfg(test)]
    pub fn fail_at(&self, fault: PublishStep) {
        self.fault
            .store(fault as u8, std::sync::atomic::Ordering::SeqCst);
    }
    fn checkpoint(&self, stage: PublishStep) -> Result<(), Error> {
        #[cfg(test)]
        if self
            .fault
            .compare_exchange(
                stage as u8,
                0,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            return Err(Error::Storage);
        }
        let _ = stage;
        Ok(())
    }
}
impl Transaction<'_> {
    fn directory(&self, profile: &str, create: bool) -> Result<Option<Directory>, Error> {
        valid_profile(profile)?;
        self.store.root.child(profile, create)
    }
    pub fn aliases(&self, profile: &str) -> Result<Vec<String>, Error> {
        let Some(dir) = self.directory(profile, false)? else {
            return Ok(Vec::new());
        };
        let mut aliases = BTreeSet::new();
        for name in dir.names()? {
            if let Some(alias) = name
                .strip_suffix(".json")
                .or_else(|| name.strip_suffix(".pending"))
            {
                valid_alias(alias).map_err(|_| Error::Storage)?;
                aliases.insert(alias.to_owned());
            }
        }
        if aliases.len() > ACCOUNTS_PER_PROFILE {
            return Err(Error::Storage);
        }
        Ok(aliases.into_iter().collect())
    }
    pub fn load(&self, profile: &str, alias: &str) -> Result<Loaded, Error> {
        valid_alias(alias)?;
        let Some(dir) = self.directory(profile, false)? else {
            return Ok(Loaded {
                account: None,
                recovery: false,
            });
        };
        let recovery = match dir.file(&format!("{alias}.pending"), libc::O_RDONLY) {
            Ok(file) if file.metadata().map_err(|_| Error::Storage)?.len() == 0 => true,
            Ok(_) => return Err(Error::Storage),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => return Err(Error::Storage),
        };
        let account = match dir.file(&format!("{alias}.json"), libc::O_RDONLY) {
            Ok(file) => {
                if file.metadata().map_err(|_| Error::Storage)?.len() > ACCOUNT_BYTES {
                    return Err(Error::Storage);
                }
                let mut bytes = Vec::new();
                file.take(ACCOUNT_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Error::Storage)?;
                if bytes.len() as u64 > ACCOUNT_BYTES {
                    return Err(Error::Storage);
                }
                let account: Account =
                    serde_json::from_slice(&bytes).map_err(|_| Error::Storage)?;
                account.validate(profile, alias)?;
                Some(account)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(Error::Storage),
        };
        Ok(Loaded { account, recovery })
    }
    pub fn clear_unpublished(&self, profile: &str, alias: &str) -> Result<(), Error> {
        let loaded = self.load(profile, alias)?;
        if loaded.account.is_some() || !loaded.recovery {
            return Err(Error::Storage);
        }
        let dir = self.directory(profile, false)?.ok_or(Error::Storage)?;
        dir.remove(&format!("{alias}.pending"))
            .map_err(|_| Error::PersistenceUncertain)?;
        dir.sync().map_err(|_| Error::PersistenceUncertain)
    }
    pub fn publish(&self, account: &Account) -> Result<(), Error> {
        account.validate(&account.profile, &account.alias)?;
        let dir = self
            .directory(&account.profile, true)?
            .ok_or(Error::Storage)?;
        let name = format!("{}.json", account.alias);
        let marker = format!("{}.pending", account.alias);
        let before = self.load(&account.profile, &account.alias)?;
        if before.account.is_none()
            && !before.recovery
            && self.aliases(&account.profile)?.len() >= ACCOUNTS_PER_PROFILE
        {
            return Err(Error::PoolFull);
        }
        let mut bytes = serde_json::to_vec_pretty(account).map_err(|_| Error::Storage)?;
        bytes.push(b'\n');
        if bytes.len() as u64 > ACCOUNT_BYTES {
            return Err(Error::Storage);
        }

        let temporary = format!(".account-{}.tmp", random_id()?);
        // The marker must be durable before any replacement can become visible.
        let guard = dir
            .file(&marker, libc::O_WRONLY | libc::O_CREAT)
            .map_err(|_| Error::Storage)?;
        if guard.metadata().map_err(|_| Error::Storage)?.len() != 0 {
            return Err(Error::Storage);
        }
        self.store
            .checkpoint(PublishStep::MarkerSync)
            .map_err(|_| Error::PersistenceUncertain)?;
        guard.sync_all().map_err(|_| Error::PersistenceUncertain)?;
        self.store
            .checkpoint(PublishStep::MarkerDirectorySync)
            .map_err(|_| Error::PersistenceUncertain)?;
        dir.sync().map_err(|_| Error::PersistenceUncertain)?;
        let mut renamed = false;
        let result: Result<(), Error> = (|| {
            let mut file = dir
                .file(&temporary, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)
                .map_err(|_| Error::Storage)?;
            self.store.checkpoint(PublishStep::Write)?;
            file.write_all(&bytes).map_err(|_| Error::Storage)?;
            self.store.checkpoint(PublishStep::FileSync)?;
            file.sync_all().map_err(|_| Error::Storage)?;
            self.store.checkpoint(PublishStep::Rename)?;
            dir.rename(&temporary, &name)?;
            renamed = true;
            self.store.checkpoint(PublishStep::DirectorySync)?;
            dir.sync()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = dir.remove(&temporary);
            if !renamed && !before.recovery && dir.remove(&marker).is_ok() && dir.sync().is_ok() {
                return Err(Error::Storage);
            }
            return Err(Error::PersistenceUncertain);
        }
        // Snapshot durability is already confirmed. A leftover marker is conservative;
        // failed cleanup must never roll back or resurrect the previous token.
        if self.store.checkpoint(PublishStep::Cleanup).is_err() || dir.remove(&marker).is_err() {
            return Err(Error::PersistenceUncertain);
        }
        // If cleanup durability fails, a crash may restore the marker, not old data.
        let _ = self
            .store
            .checkpoint(PublishStep::CleanupDirectorySync)
            .and_then(|_| dir.sync());
        Ok(())
    }
}
