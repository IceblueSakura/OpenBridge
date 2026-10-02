//! Account-bound OAuth ownership; never imported by semantic codecs or lowering.
mod browser;
mod callback;
mod codex;
mod codex_metadata;
mod driver;
mod grok;
mod grok_metadata;
mod http;
mod jwt;
mod manager;
mod model;
mod oauth;
mod profiles;
mod storage_fs;
mod store;

pub use driver::{AuthDriver, DriverFuture, LoginObserver};
pub use manager::CredentialManager;
pub use model::{
    AccessState, AccountState, AccountStatus, BrowserPrompt, Credential, DevicePrompt, Grant,
    LoginMethod, LoginOptions, LoginPrompt, LogoutOutcome, Secret, VerifiedIdentity,
};
pub use profiles::builtin_drivers;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CredentialError {
    #[error("unknown authorization profile")]
    UnknownProfile,
    #[error("operation or login method is unsupported by this profile")]
    Unsupported,
    #[error("legacy credential snapshot is unsupported; select a new explicit store directory")]
    LegacyStore,
    #[error("credential publication is uncertain; explicit login or local logout required")]
    PersistenceUncertain,
    #[error("invalid account or client identifier")]
    InvalidInput,
    #[error("credential store is unavailable or unsafe")]
    Storage,
    #[error("credential store is busy")]
    Busy,
    #[error("credential pool limit reached")]
    PoolFull,
    #[error("account is not signed in; login required")]
    LoginRequired,
    #[error("authorization transaction was superseded")]
    Superseded,
    #[error("authorized identity does not match the account binding")]
    IdentityMismatch,
    #[error("required OAuth permissions were not granted")]
    Permission,
    #[error("authority returned an invalid or unsupported response")]
    Protocol,
    #[error("authority request failed; no automatic retry")]
    Network,
    #[error("authority request timed out")]
    Timeout,
    #[error("authorization was denied")]
    Denied,
    #[error("authorization transaction expired")]
    Expired,
    #[error("loopback callback listener is unavailable")]
    Callback,
    #[error("device login is not enabled for this deployment; verify authority/account policy")]
    DeviceUnavailable,
    #[error("refresh token expired; login required")]
    GrantExpired,
    #[error("refresh token already used; login required")]
    GrantReused,
    #[error("refresh token revoked; login required")]
    GrantRevoked,
    #[error("OAuth grant was rejected; login required")]
    InvalidGrant,
}

#[cfg(test)]
mod browser_tests;
#[cfg(test)]
mod codex_browser_tests;
#[cfg(test)]
mod codex_tests;
#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod tests;
