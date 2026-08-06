//! OpenBridge-owned OAuth2 credential lifecycle facade.
//!
//! Document validation, transactional file storage, interactive Provider login, and runtime
//! credential ownership live in separate child modules. Callers never receive a generic secret or
//! locator accessor through this facade.

mod document;
pub mod login;
mod manager;
mod refresh;
mod storage;

pub use login::{ChatGptDevicePrompt, OAuth2LoginError, OAuth2LoginOutcome, login_chatgpt};
pub use manager::{
    OAuth2Credential, OAuth2CredentialManager, OAuth2CredentialManagerError,
    OAuth2CredentialStatus, OAuth2RefreshOutcome,
};
pub use storage::OAuth2LoginTarget;

pub(crate) use manager::OAuth2CredentialLease;
pub(crate) use manager::OAuth2CredentialManagerBuilder;
