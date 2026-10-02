//! Explicit composition root. Adding a driver does not change lifecycle or storage.
use super::{AuthDriver, CredentialError, codex::CodexAuthority, grok::GrokAuthority};
use std::sync::Arc;
pub fn builtin_drivers(proxy: Option<&str>) -> Result<Vec<Arc<dyn AuthDriver>>, CredentialError> {
    Ok(vec![
        Arc::new(GrokAuthority::new(proxy)?),
        Arc::new(CodexAuthority::new(proxy)?),
    ])
}
