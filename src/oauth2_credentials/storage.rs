//! Transactional file storage for one OpenBridge-owned OAuth2 credential.
//!
//! A purpose-bound target hides its locator from ordinary callers. Writers serialize through an
//! advisory lock, compare the source version, and atomically replace a complete same-directory
//! document so concurrent login or refresh operations cannot silently overwrite each other.

use std::{
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::provider::ProviderKind;

const MAX_AUTH_DOCUMENT_BYTES: u64 = 64 * 1024;
const TEMP_FILE_ATTEMPTS: usize = 32;
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
pub(crate) fn next_test_id() -> u64 {
    NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
}

/// Purpose-bound destination for one Provider's managed OAuth2 auth document.
#[derive(Clone)]
pub struct OAuth2LoginTarget {
    provider: ProviderKind,
    pool_id: String,
    auth_json_file: PathBuf,
}

impl OAuth2LoginTarget {
    /// Creates a target after private configuration and registry ownership are validated.
    pub(crate) fn new(
        provider: ProviderKind,
        pool_id: impl Into<String>,
        auth_json_file: PathBuf,
    ) -> Self {
        Self {
            provider,
            pool_id: pool_id.into(),
            auth_json_file,
        }
    }

    /// Returns the sole Provider permitted to populate this target.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// Returns the compile-time credential pool binding ID.
    pub fn pool_id(&self) -> &str {
        &self.pool_id
    }

    /// Captures a non-reversible source version without requiring the file to exist.
    pub(crate) fn capture_version(&self) -> Result<OAuth2AuthFileVersion, OAuth2StorageError> {
        read_version(&self.auth_json_file)
    }

    /// Replaces the complete document only if the source version remains unchanged.
    pub(crate) fn compare_and_replace(
        &self,
        expected: &OAuth2AuthFileVersion,
        document: &[u8],
    ) -> Result<(), OAuth2StorageError> {
        // Acquire the destination lock and reload its source version inside the transaction.
        let locked = self.lock()?;
        let current = locked.current_version()?;
        if &current != expected {
            return Err(OAuth2StorageError::ConcurrentModification);
        }

        // Publish through the same secure atomic-replacement boundary used by refresh.
        locked.replace(document)
    }

    /// Acquires the persistent advisory lock used by guarded refresh and transactional writes.
    pub(crate) fn lock(&self) -> Result<OAuth2LockedAuthFile, OAuth2StorageError> {
        // Prepare the parent before creating the persistent same-directory lock file.
        let parent = self
            .auth_json_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|_| OAuth2StorageError::PrepareDirectory)?;

        // Serialize every writer for this destination through a persistent advisory lock file.
        let lock_path = lock_path(&self.auth_json_file);
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| OAuth2StorageError::Lock)?;
        lock_file.lock().map_err(|_| OAuth2StorageError::Lock)?;
        Ok(OAuth2LockedAuthFile {
            auth_json_file: self.auth_json_file.clone(),
            _lock_file: lock_file,
        })
    }
}

impl fmt::Debug for OAuth2LoginTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2LoginTarget")
            .field("provider", &self.provider)
            .field("pool_id", &self.pool_id)
            .field("auth_json_file", &"[REDACTED]")
            .finish()
    }
}

/// Held cross-process transaction guard for one managed OAuth2 auth file.
pub(crate) struct OAuth2LockedAuthFile {
    auth_json_file: PathBuf,
    _lock_file: File,
}

impl OAuth2LockedAuthFile {
    /// Reads the complete current auth document while the advisory lock is held.
    pub(crate) fn read_document(&self) -> Result<Zeroizing<Vec<u8>>, OAuth2StorageError> {
        read_auth_document(&self.auth_json_file)
    }

    /// Atomically replaces the complete auth document while retaining the advisory lock.
    pub(crate) fn replace(&self, document: &[u8]) -> Result<(), OAuth2StorageError> {
        // Validate the bounded complete document before creating a temporary artifact.
        if document.is_empty() || document.len() as u64 > MAX_AUTH_DOCUMENT_BYTES {
            return Err(OAuth2StorageError::InvalidDocumentSize);
        }
        let parent = self
            .auth_json_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        // Write and sync a secure same-directory temporary file before atomic replacement.
        let (temporary_path, mut temporary) = create_temporary_file(parent)?;
        let write_result = (|| {
            temporary
                .write_all(document)
                .map_err(|_| OAuth2StorageError::Write)?;
            temporary.sync_all().map_err(|_| OAuth2StorageError::Sync)?;
            drop(temporary);
            fs::rename(&temporary_path, &self.auth_json_file)
                .map_err(|_| OAuth2StorageError::Replace)?;
            sync_parent_directory(parent)?;
            Ok(())
        })();

        // Remove only the exact unpublished temporary file after a failed transaction.
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        write_result
    }

    /// Captures the locked source version for compare-and-replace login transactions.
    pub(crate) fn current_version(&self) -> Result<OAuth2AuthFileVersion, OAuth2StorageError> {
        read_version(&self.auth_json_file)
    }
}

/// Reads one regular managed auth document into bounded zeroizing memory.
pub(crate) fn read_auth_document(path: &Path) -> Result<Zeroizing<Vec<u8>>, OAuth2StorageError> {
    // Reject absent, linked, non-file, or oversized sources before reading credential bytes.
    let metadata = fs::symlink_metadata(path).map_err(|_| OAuth2StorageError::Read)?;
    if !metadata.is_file() || metadata.len() > MAX_AUTH_DOCUMENT_BYTES {
        return Err(OAuth2StorageError::InvalidDocumentSize);
    }

    // Recheck the actual bytes after reading to close metadata-to-read growth races.
    let document = Zeroizing::new(fs::read(path).map_err(|_| OAuth2StorageError::Read)?);
    if document.len() as u64 > MAX_AUTH_DOCUMENT_BYTES {
        return Err(OAuth2StorageError::InvalidDocumentSize);
    }
    Ok(document)
}

impl fmt::Debug for OAuth2LockedAuthFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2LockedAuthFile")
            .field("auth_json_file", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Digest of an existing auth document, or the explicit absence of one.
#[derive(Clone, Eq, PartialEq)]
pub(crate) enum OAuth2AuthFileVersion {
    Missing,
    Present([u8; 32]),
}

impl fmt::Debug for OAuth2AuthFileVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "OAuth2AuthFileVersion::Missing",
            Self::Present(_) => "OAuth2AuthFileVersion::Present([REDACTED])",
        })
    }
}

/// Reads one bounded document and converts it into a non-reversible version digest.
fn read_version(path: &Path) -> Result<OAuth2AuthFileVersion, OAuth2StorageError> {
    // Distinguish an absent first-login target from every other read failure.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(OAuth2AuthFileVersion::Missing);
        }
        Err(_) => return Err(OAuth2StorageError::Read),
    };
    if !metadata.is_file() || metadata.len() > MAX_AUTH_DOCUMENT_BYTES {
        return Err(OAuth2StorageError::InvalidDocumentSize);
    }

    // Hash bounded zeroizing file bytes without retaining a reversible source version.
    let bytes = read_auth_document(path)?;
    Ok(version_for_document(&bytes))
}

/// Computes the non-reversible version used by in-memory guarded-refresh snapshots.
pub(crate) fn version_for_document(document: &[u8]) -> OAuth2AuthFileVersion {
    OAuth2AuthFileVersion::Present(Sha256::digest(document).into())
}

/// Derives the persistent advisory-lock path beside the managed auth document.
fn lock_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("auth.json"))
        .to_os_string();
    name.push(".lock");
    path.with_file_name(name)
}

/// Creates one collision-resistant temporary file with platform-appropriate private access.
fn create_temporary_file(parent: &Path) -> Result<(PathBuf, File), OAuth2StorageError> {
    // Try bounded process-unique names and rely on create_new for the TOCTOU boundary.
    for _ in 0..TEMP_FILE_ATTEMPTS {
        let suffix = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".openbridge-oauth.{}.{}.tmp",
            std::process::id(),
            suffix
        ));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        configure_owner_only(&mut options);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(OAuth2StorageError::CreateTemporary),
        }
    }
    Err(OAuth2StorageError::CreateTemporary)
}

/// Applies owner-only creation mode where POSIX permissions are available.
#[cfg(unix)]
fn configure_owner_only(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

/// Relies on the private directory ACL on Windows while still using create_new atomically.
#[cfg(not(unix))]
fn configure_owner_only(_options: &mut OpenOptions) {}

/// Syncs the containing directory after replacement on platforms that support directory handles.
#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<(), OAuth2StorageError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| OAuth2StorageError::Sync)
}

/// Windows `rename` uses an atomic replace path; directory handle syncing is not portable there.
#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<(), OAuth2StorageError> {
    Ok(())
}

/// Value-free transactional storage failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OAuth2StorageError {
    /// The document is empty, too large, or the existing target is not a regular file.
    #[error("OAuth2 auth document has an invalid size or file type")]
    InvalidDocumentSize,
    /// The parent directory could not be prepared.
    #[error("OAuth2 auth directory could not be prepared")]
    PrepareDirectory,
    /// The advisory lock could not be opened or acquired.
    #[error("OAuth2 auth transaction lock failed")]
    Lock,
    /// The current auth document could not be read.
    #[error("OAuth2 auth document could not be read")]
    Read,
    /// Another writer changed the auth document after this operation started.
    #[error("OAuth2 auth document changed during the transaction")]
    ConcurrentModification,
    /// A secure same-directory temporary file could not be created.
    #[error("OAuth2 auth temporary file could not be created")]
    CreateTemporary,
    /// The complete document could not be written.
    #[error("OAuth2 auth document could not be written")]
    Write,
    /// File or directory state could not be synchronized.
    #[error("OAuth2 auth document could not be synchronized")]
    Sync,
    /// The complete temporary document could not atomically replace the target.
    #[error("OAuth2 auth document could not be replaced")]
    Replace,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    #[test]
    fn compare_and_replace_serializes_writers_and_never_exposes_secrets() {
        let fixture = TestDirectory::new();
        let target = OAuth2LoginTarget::new(
            ProviderKind::ChatGpt,
            "chatgpt-codex",
            fixture.path.join("sensitive-auth.json"),
        );

        // Create the first document from an explicit missing-file version.
        let missing = target.capture_version().unwrap();
        target
            .compare_and_replace(&missing, br#"{"token":"synthetic-first"}"#)
            .unwrap();
        let first = target.capture_version().unwrap();

        // Replace from the current version and reject a stale competing writer.
        target
            .compare_and_replace(&first, br#"{"token":"synthetic-second"}"#)
            .unwrap();
        let error = target
            .compare_and_replace(&first, br#"{"token":"synthetic-stale"}"#)
            .unwrap_err();
        assert_eq!(error, OAuth2StorageError::ConcurrentModification);
        assert_eq!(
            fs::read(target.auth_json_file.as_path()).unwrap(),
            br#"{"token":"synthetic-second"}"#
        );

        // Keep the locator, digest, and document values out of every diagnostic view.
        let diagnostic = format!("{target:?} {first:?} {error:?} {error}");
        for forbidden in [
            "sensitive-auth",
            "synthetic-first",
            "synthetic-second",
            "synthetic-stale",
        ] {
            assert!(!diagnostic.contains(forbidden));
        }
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let suffix = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "openbridge-oauth-storage-test-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Remove only files created by this process-unique test directory.
            if let Ok(entries) = fs::read_dir(&self.path) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.path);
        }
    }
}
