//! Closed execution metadata from trusted authorization, never inbound headers.
use crate::{
    credential::{AccessGrant, codex_metadata},
    provider::AuthScheme,
};
pub(crate) fn headers(auth: AuthScheme, grant: &AccessGrant) -> Result<Vec<(String, String)>, ()> {
    match auth {
        AuthScheme::OAuthBearer("codex") => {
            let account = grant.identity.scope.as_ref().ok_or(())?;
            if account.is_empty() || !account.bytes().all(|c| c.is_ascii_graphic()) {
                return Err(());
            }
            Ok(vec![
                ("chatgpt-account-id".into(), account.clone()),
                ("originator".into(), codex_metadata::ORIGINATOR.into()),
                ("user-agent".into(), codex_metadata::user_agent().into()),
            ])
        }
        AuthScheme::OAuthBearer("grok") if grant.identity.scope.is_none() => Ok(vec![]),
        _ => Err(()),
    }
}
