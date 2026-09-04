//! Explicit Provider login entry points for OpenBridge-owned OAuth2 credentials.
//!
//! Each Provider owns its own state-machine module and wire transport so upstream protocol drift
//! stays contained per Provider. This facade exposes only the public login functions, prompts,
//! outcome, and error shapes; the protocol-agnostic atoms live in `common` and are not part of
//! any Provider's flow logic.

mod chatgpt;
pub(crate) mod common;
mod grok;

pub use chatgpt::{ChatGptDevicePrompt, login_chatgpt};
pub use common::{OAuth2LoginError, OAuth2LoginOutcome};
pub use grok::{GrokDevicePrompt, login_grok};
