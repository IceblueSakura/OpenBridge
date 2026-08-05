//! Startup-loaded OAuth2 credentials owned by OpenBridge.
//!
//! The manager loads one complete provider-bound `auth.json` per configured OAuth2 Provider before
//! the listener starts. It retains an immutable credential snapshot and a redacted file locator for
//! a later lifecycle implementation, but deliberately provides no reload, refresh, or persistence
//! API in the current focus.

use std::{
    fmt, fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::SecretString;
use serde::{Deserialize, de::IgnoredAny};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    credential::{CredentialMetadata, CredentialSource},
    provider::{CredentialKind, ProviderKind},
};

/// Immutable collection of startup-validated OAuth2 credentials.
pub struct OAuth2CredentialManager {
    credentials: Vec<ManagedOAuth2Credential>,
}

impl OAuth2CredentialManager {
    /// Creates an empty manager for runtimes and tests with no configured OAuth2 Provider.
    pub fn empty() -> Self {
        Self {
            credentials: Vec::new(),
        }
    }

    /// Returns the number of configured OAuth2 Providers without exposing their locators or tokens.
    pub fn configured_provider_count(&self) -> usize {
        self.credentials.len()
    }

    /// Returns the immutable credential view for one Provider, if configured.
    pub fn credential_for_provider(&self, provider: ProviderKind) -> Option<OAuth2Credential<'_>> {
        self.credentials
            .iter()
            .find(|credential| credential.provider == provider)
            .map(|credential| OAuth2Credential { credential })
    }
}

impl Default for OAuth2CredentialManager {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for OAuth2CredentialManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialManager")
            .field("configured_providers", &self.credentials.len())
            .finish()
    }
}

/// Short-lived, redacted view of one immutable OAuth2 credential.
pub struct OAuth2Credential<'a> {
    credential: &'a ManagedOAuth2Credential,
}

impl OAuth2Credential<'_> {
    /// Returns the Provider permitted to consume this credential.
    pub fn provider(&self) -> ProviderKind {
        self.credential.provider
    }

    /// Returns the compile-time credential binding ID.
    pub fn pool_id(&self) -> &str {
        &self.credential.pool_id
    }

    /// Returns the sole stable member ID derived from the binding ID.
    pub fn member_id(&self) -> &str {
        &self.credential.member_id
    }

    /// Returns non-sensitive metadata frozen with the startup snapshot.
    pub fn metadata(&self) -> &CredentialMetadata {
        &self.credential.metadata
    }
}

impl fmt::Debug for OAuth2Credential<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2Credential")
            .field("provider", &self.credential.provider)
            .field("pool_id", &self.credential.pool_id)
            .field("member_id", &self.credential.member_id)
            .field("metadata", &self.credential.metadata)
            .field("auth_json_file", &"[REDACTED]")
            .field("tokens", &"[REDACTED]")
            .finish()
    }
}

struct ManagedOAuth2Credential {
    provider: ProviderKind,
    pool_id: String,
    member_id: String,
    metadata: CredentialMetadata,
    _auth_json_file: PathBuf,
    _id_token: SecretString,
    _access_token: SecretString,
    _refresh_token: SecretString,
    _account_id: SecretString,
    _is_fedramp_account: bool,
    _last_refresh: Option<String>,
}

/// Startup-only builder that reads configured OAuth2 auth files before freezing the manager.
#[derive(Default)]
pub(crate) struct OAuth2CredentialManagerBuilder {
    credentials: Vec<ManagedOAuth2Credential>,
}

impl OAuth2CredentialManagerBuilder {
    /// Creates an empty startup builder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Loads one Provider-bound auth file and retains its complete immutable token bundle.
    pub(crate) fn load_auth_json_file(
        &mut self,
        provider: ProviderKind,
        pool_id: &str,
        path: PathBuf,
    ) -> Result<(), OAuth2CredentialManagerError> {
        // Reject unsupported or duplicate Provider ownership before reading any locator.
        if provider != ProviderKind::ChatGpt {
            return Err(OAuth2CredentialManagerError::UnsupportedProvider);
        }
        if self
            .credentials
            .iter()
            .any(|credential| credential.provider == provider)
        {
            return Err(OAuth2CredentialManagerError::DuplicateProvider);
        }

        // Read the complete OpenBridge-owned document into zeroizing memory.
        let document = Zeroizing::new(
            fs::read_to_string(&path).map_err(|_| OAuth2CredentialManagerError::Read)?,
        );
        let auth: RawAuthDotJson<'_> = serde_json::from_str(&document)
            .map_err(|_| OAuth2CredentialManagerError::InvalidDocument)?;
        let tokens = validate_auth_document(&auth)?;

        // Validate the account-bound token context required by the ChatGPT Provider.
        let expires_at = parse_access_token_expiry(tokens.access_token)?;
        if expires_at <= SystemTime::now() {
            return Err(OAuth2CredentialManagerError::ExpiredAccessToken);
        }
        let (account_id, is_fedramp_account) =
            parse_id_token_context(tokens.id_token, tokens.account_id)?;

        // Move every lifecycle token into a single immutable Provider-bound manager entry.
        self.credentials.push(ManagedOAuth2Credential {
            provider,
            pool_id: pool_id.to_owned(),
            member_id: format!("{pool_id}#1"),
            metadata: CredentialMetadata::upstream(
                CredentialKind::OAuth2BearerAccessToken,
                CredentialSource::OAuth2AuthJsonFile,
            )
            .with_expires_at(expires_at),
            _auth_json_file: path,
            _id_token: SecretString::from(tokens.id_token.to_owned()),
            _access_token: SecretString::from(tokens.access_token.to_owned()),
            _refresh_token: SecretString::from(tokens.refresh_token.to_owned()),
            _account_id: SecretString::from(account_id.to_string()),
            _is_fedramp_account: is_fedramp_account,
            _last_refresh: auth.last_refresh.map(str::to_owned),
        });
        Ok(())
    }

    /// Freezes all loaded entries into an immutable manager.
    pub(crate) fn build(self) -> OAuth2CredentialManager {
        OAuth2CredentialManager {
            credentials: self.credentials,
        }
    }
}

impl fmt::Debug for OAuth2CredentialManagerBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuth2CredentialManagerBuilder")
            .field("configured_providers", &self.credentials.len())
            .finish()
    }
}

/// Validates the managed ChatGPT auth envelope and returns its complete token bundle.
fn validate_auth_document<'a>(
    auth: &'a RawAuthDotJson<'a>,
) -> Result<&'a RawOAuth2Tokens<'a>, OAuth2CredentialManagerError> {
    // Require the explicit managed ChatGPT mode and reject conflicting credential material.
    if !auth
        .auth_mode
        .is_some_and(|mode| mode.eq_ignore_ascii_case("chatgpt"))
    {
        return Err(OAuth2CredentialManagerError::UnsupportedAuthMode);
    }
    if auth.openai_api_key.is_some()
        || auth.personal_access_token.is_some()
        || auth.bedrock_api_key.is_some()
    {
        return Err(OAuth2CredentialManagerError::ConflictingAuthMaterial);
    }

    // Require every lifecycle token and any present refresh timestamp to be non-blank.
    let tokens = auth
        .tokens
        .as_ref()
        .ok_or(OAuth2CredentialManagerError::MissingTokens)?;
    if tokens.id_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidIdToken);
    }
    if tokens.access_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidAccessToken);
    }
    if tokens.refresh_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidRefreshToken);
    }
    if auth
        .last_refresh
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(OAuth2CredentialManagerError::InvalidLastRefresh);
    }
    Ok(tokens)
}

/// Parses the access-token JWT expiry without retaining unrelated claims.
fn parse_access_token_expiry(token: &str) -> Result<SystemTime, OAuth2CredentialManagerError> {
    // Decode the payload into zeroizing memory and parse only the standard numeric expiry claim.
    let payload =
        decode_jwt_payload(token).map_err(|_| OAuth2CredentialManagerError::InvalidAccessToken)?;
    let claims: AccessTokenClaims = serde_json::from_slice(&payload)
        .map_err(|_| OAuth2CredentialManagerError::InvalidAccessToken)?;
    let expiry = claims
        .exp
        .ok_or(OAuth2CredentialManagerError::MissingAccessTokenExpiry)?;
    let expiry =
        u64::try_from(expiry).map_err(|_| OAuth2CredentialManagerError::InvalidAccessToken)?;
    UNIX_EPOCH
        .checked_add(Duration::from_secs(expiry))
        .ok_or(OAuth2CredentialManagerError::InvalidAccessToken)
}

/// Validates ID-token account binding and the conditional FedRAMP routing claim.
fn parse_id_token_context(
    token: &str,
    selected_account_id: Option<&str>,
) -> Result<(Zeroizing<String>, bool), OAuth2CredentialManagerError> {
    // Decode the payload into zeroizing memory and borrow only the nested ChatGPT auth claims.
    let payload =
        decode_jwt_payload(token).map_err(|_| OAuth2CredentialManagerError::InvalidIdToken)?;
    let claims: IdTokenClaims<'_> = serde_json::from_slice(&payload)
        .map_err(|_| OAuth2CredentialManagerError::InvalidIdToken)?;
    let embedded_account_id = claims
        .auth
        .as_ref()
        .and_then(|auth| auth.chatgpt_account_id)
        .filter(|value| !value.trim().is_empty());
    let selected_account_id = selected_account_id.filter(|value| !value.trim().is_empty());

    // Reject conflicting sources, then follow the top-level-to-ID-token account fallback.
    if selected_account_id
        .zip(embedded_account_id)
        .is_some_and(|(selected, embedded)| selected != embedded)
    {
        return Err(OAuth2CredentialManagerError::AccountBindingMismatch);
    }
    let account_id = selected_account_id
        .or(embedded_account_id)
        .ok_or(OAuth2CredentialManagerError::MissingAccountBinding)?;
    let is_fedramp_account = claims
        .auth
        .as_ref()
        .is_some_and(|auth| auth.chatgpt_account_is_fedramp);
    Ok((Zeroizing::new(account_id.to_owned()), is_fedramp_account))
}

/// Decodes the middle segment of an exactly three-part JWT into zeroizing bytes.
fn decode_jwt_payload(token: &str) -> Result<Zeroizing<Vec<u8>>, ()> {
    // Require non-empty header, payload, and signature segments with no trailing components.
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(());
    };
    if header.is_empty() || payload.is_empty() || signature.is_empty() {
        return Err(());
    }

    // Decode only the payload using JWT base64url rules and zero it after claim validation.
    URL_SAFE_NO_PAD
        .decode(payload)
        .map(Zeroizing::new)
        .map_err(|_| ())
}

#[derive(Deserialize)]
struct RawAuthDotJson<'a> {
    #[serde(default)]
    auth_mode: Option<&'a str>,
    #[serde(rename = "OPENAI_API_KEY", default)]
    openai_api_key: Option<&'a str>,
    #[serde(default, borrow)]
    tokens: Option<RawOAuth2Tokens<'a>>,
    #[serde(default)]
    last_refresh: Option<&'a str>,
    #[serde(default)]
    personal_access_token: Option<IgnoredAny>,
    #[serde(default)]
    bedrock_api_key: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct RawOAuth2Tokens<'a> {
    id_token: &'a str,
    access_token: &'a str,
    refresh_token: &'a str,
    #[serde(default)]
    account_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct AccessTokenClaims {
    #[serde(default)]
    exp: Option<i64>,
}

#[derive(Deserialize)]
struct IdTokenClaims<'a> {
    #[serde(rename = "https://api.openai.com/auth", default, borrow)]
    auth: Option<IdTokenAuthClaims<'a>>,
}

#[derive(Deserialize)]
struct IdTokenAuthClaims<'a> {
    #[serde(default)]
    chatgpt_account_id: Option<&'a str>,
    #[serde(default)]
    chatgpt_account_is_fedramp: bool,
}

/// Value-free failure returned while building the immutable OAuth2 manager.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OAuth2CredentialManagerError {
    /// The configured Provider has no managed auth-file adapter.
    #[error("OAuth2 credential Provider is unsupported")]
    UnsupportedProvider,
    /// The same Provider was added more than once.
    #[error("OAuth2 credential Provider is configured more than once")]
    DuplicateProvider,
    /// The configured auth file could not be read.
    #[error("OAuth2 auth file could not be read")]
    Read,
    /// The file is not a valid ChatGPT OAuth JSON document.
    #[error("OAuth2 auth file is invalid")]
    InvalidDocument,
    /// The file does not explicitly select managed ChatGPT authentication.
    #[error("OAuth2 auth mode is unsupported")]
    UnsupportedAuthMode,
    /// Another credential type appears beside the ChatGPT OAuth bundle.
    #[error("OAuth2 auth file contains conflicting credential material")]
    ConflictingAuthMaterial,
    /// The document contains no OAuth2 token bundle.
    #[error("OAuth2 token bundle is missing")]
    MissingTokens,
    /// The ID token is blank or cannot be decoded.
    #[error("OAuth2 identity token is invalid")]
    InvalidIdToken,
    /// The access token is blank or cannot be decoded.
    #[error("OAuth2 access token is invalid")]
    InvalidAccessToken,
    /// The access-token JWT has no absolute expiry.
    #[error("OAuth2 access token expiry is missing")]
    MissingAccessTokenExpiry,
    /// The access token is already expired when the startup snapshot is built.
    #[error("OAuth2 access token is expired")]
    ExpiredAccessToken,
    /// The refresh token required by the later lifecycle is blank.
    #[error("OAuth2 refresh token is invalid")]
    InvalidRefreshToken,
    /// The token bundle has no selected account/workspace binding.
    #[error("OAuth2 account binding is missing")]
    MissingAccountBinding,
    /// The selected account conflicts with the ID-token account claim.
    #[error("OAuth2 account binding is inconsistent")]
    AccountBindingMismatch,
    /// The optional refresh timestamp is present but blank.
    #[error("OAuth2 last refresh timestamp is invalid")]
    InvalidLastRefresh,
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn managed_entry_retains_complete_tokens_without_exposing_them_through_debug() {
        let entry = ManagedOAuth2Credential {
            provider: ProviderKind::ChatGpt,
            pool_id: "chatgpt-codex".to_owned(),
            member_id: "chatgpt-codex#1".to_owned(),
            metadata: CredentialMetadata::upstream(
                CredentialKind::OAuth2BearerAccessToken,
                CredentialSource::OAuth2AuthJsonFile,
            ),
            _auth_json_file: std::path::Path::new("sensitive-auth-file.json").to_owned(),
            _id_token: SecretString::from("synthetic-id".to_owned()),
            _access_token: SecretString::from("synthetic-access".to_owned()),
            _refresh_token: SecretString::from("synthetic-refresh".to_owned()),
            _account_id: SecretString::from("synthetic-account".to_owned()),
            _is_fedramp_account: false,
            _last_refresh: Some("2026-08-05T00:00:00Z".to_owned()),
        };

        // Confirm the manager-owned record retains the full future lifecycle bundle.
        assert_eq!(entry._id_token.expose_secret(), "synthetic-id");
        assert_eq!(entry._access_token.expose_secret(), "synthetic-access");
        assert_eq!(entry._refresh_token.expose_secret(), "synthetic-refresh");
        assert_eq!(entry._account_id.expose_secret(), "synthetic-account");

        // Keep every secret and locator out of the public credential view.
        let view = OAuth2Credential { credential: &entry };
        let debug = format!("{view:?}");
        for forbidden in [
            "synthetic-id",
            "synthetic-access",
            "synthetic-refresh",
            "synthetic-account",
            "sensitive-auth-file",
        ] {
            assert!(!debug.contains(forbidden));
        }
    }
}
