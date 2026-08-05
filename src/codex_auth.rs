//! Probe-only loader for Codex ChatGPT file credentials.
//!
//! The loader reads one administrator-selected `auth.json` exactly once, borrows only the current
//! ChatGPT access-token fields during parsing, and moves an account-bound OAuth snapshot into the
//! purpose-restricted credential store. It never reads or retains the refresh token, writes the
//! source file, or includes its path or credential values in errors and debug output.

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::SecretString;
use serde::Deserialize;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    credential::{CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder},
    provider::{CredentialKind, ProviderKind},
    registry::RuntimeRegistry,
};

/// Reads a Codex ChatGPT auth file into the credential pool bound to one compiled probe target.
///
/// The target must be the independent ChatGPT Provider using an OAuth bearer pool. All token,
/// account, JWT, JSON, and filesystem details are normalized to value-free error variants.
pub fn load_codex_auth_file_for_target(
    path: &Path,
    registry: &RuntimeRegistry,
    upstream_target_id: &str,
) -> Result<CredentialStore, CodexAuthFileError> {
    // Resolve the compiled target and require its exact ChatGPT OAuth pool boundary.
    let target = registry
        .upstream_target(upstream_target_id)
        .ok_or(CodexAuthFileError::TargetMismatch)?;
    let pool = registry
        .credential_pool(target.credential_pool_id())
        .ok_or(CodexAuthFileError::TargetMismatch)?;
    if target.kind() != ProviderKind::ChatGpt
        || pool.provider() != ProviderKind::ChatGpt
        || pool.kind() != CredentialKind::OAuth2BearerAccessToken
    {
        return Err(CodexAuthFileError::TargetMismatch);
    }

    // Read the document once into zeroizing memory and parse only borrowed fields used by the probe.
    let document = Zeroizing::new(fs::read_to_string(path).map_err(|_| CodexAuthFileError::Read)?);
    let auth: RawCodexAuth<'_> =
        serde_json::from_str(&document).map_err(|_| CodexAuthFileError::InvalidDocument)?;
    validate_auth_mode(&auth)?;
    let tokens = auth.tokens.ok_or(CodexAuthFileError::MissingTokens)?;

    // Validate token availability, account binding, expiry, and conditional routing context.
    if tokens.access_token.trim().is_empty() {
        return Err(CodexAuthFileError::InvalidAccessToken);
    }
    let expires_at = parse_access_token_expiry(tokens.access_token)?;
    if expires_at <= SystemTime::now() {
        return Err(CodexAuthFileError::ExpiredAccessToken);
    }
    let (account_id, is_fedramp_account) =
        parse_id_token_context(tokens.id_token, tokens.account_id)?;

    // Move only the access token and account context into a single-member purpose-bound snapshot.
    let mut builder = CredentialStoreBuilder::new();
    builder
        .insert_chatgpt_oauth_member(
            pool.id(),
            format!("{}#1", pool.id()),
            SecretString::from(tokens.access_token.to_owned()),
            SecretString::from(account_id.to_string()),
            is_fedramp_account,
            CredentialMetadata::upstream(pool.kind(), CredentialSource::CodexAuthFile)
                .with_expires_at(expires_at),
        )
        .map_err(|_| CodexAuthFileError::CredentialConstruction)?;
    Ok(builder.build())
}

/// Validates the Codex authentication mode without accepting API-key or mixed credential records.
fn validate_auth_mode(auth: &RawCodexAuth<'_>) -> Result<(), CodexAuthFileError> {
    // Preserve Codex's legacy missing-mode ChatGPT fallback while rejecting explicit non-ChatGPT modes.
    if auth.openai_api_key.is_some()
        || auth
            .auth_mode
            .is_some_and(|mode| !mode.eq_ignore_ascii_case("chatgpt"))
    {
        return Err(CodexAuthFileError::UnsupportedAuthMode);
    }
    Ok(())
}

/// Parses the access-token JWT expiry without verifying or retaining unrelated claims.
fn parse_access_token_expiry(token: &str) -> Result<SystemTime, CodexAuthFileError> {
    // Decode the payload into zeroizing memory and parse only the standard numeric expiry claim.
    let payload = decode_jwt_payload(token).map_err(|_| CodexAuthFileError::InvalidAccessToken)?;
    let claims: AccessTokenClaims =
        serde_json::from_slice(&payload).map_err(|_| CodexAuthFileError::InvalidAccessToken)?;
    let expiry = claims
        .exp
        .ok_or(CodexAuthFileError::MissingAccessTokenExpiry)?;
    let expiry = u64::try_from(expiry).map_err(|_| CodexAuthFileError::InvalidAccessToken)?;
    UNIX_EPOCH
        .checked_add(Duration::from_secs(expiry))
        .ok_or(CodexAuthFileError::InvalidAccessToken)
}

/// Validates the ID-token shape, account binding, and conditional FedRAMP routing claim.
fn parse_id_token_context(
    token: &str,
    selected_account_id: Option<&str>,
) -> Result<(Zeroizing<String>, bool), CodexAuthFileError> {
    // Decode the payload into zeroizing memory and borrow only the nested ChatGPT auth claims.
    let payload = decode_jwt_payload(token).map_err(|_| CodexAuthFileError::InvalidIdToken)?;
    let claims: IdTokenClaims<'_> =
        serde_json::from_slice(&payload).map_err(|_| CodexAuthFileError::InvalidIdToken)?;
    let embedded_account_id = claims
        .auth
        .as_ref()
        .and_then(|auth| auth.chatgpt_account_id)
        .filter(|value| !value.trim().is_empty());
    let selected_account_id = selected_account_id.filter(|value| !value.trim().is_empty());

    // Reject conflicting sources, then follow Codex's top-level-to-ID-token account fallback.
    if selected_account_id
        .zip(embedded_account_id)
        .is_some_and(|(selected, embedded)| selected != embedded)
    {
        return Err(CodexAuthFileError::AccountBindingMismatch);
    }
    let account_id = selected_account_id
        .or(embedded_account_id)
        .ok_or(CodexAuthFileError::MissingAccountBinding)?;
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
struct RawCodexAuth<'a> {
    #[serde(default)]
    auth_mode: Option<&'a str>,
    #[serde(rename = "OPENAI_API_KEY", default)]
    openai_api_key: Option<&'a str>,
    #[serde(default, borrow)]
    tokens: Option<RawCodexTokens<'a>>,
}

#[derive(Deserialize)]
struct RawCodexTokens<'a> {
    id_token: &'a str,
    access_token: &'a str,
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

/// Value-free failure returned while binding one Codex file credential to the ChatGPT probe.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CodexAuthFileError {
    /// The selected target is not the compiled ChatGPT OAuth probe target.
    #[error("selected target does not accept Codex ChatGPT file credentials")]
    TargetMismatch,
    /// The file could not be read.
    #[error("Codex authentication file could not be read")]
    Read,
    /// The file was not a valid JSON authentication record.
    #[error("Codex authentication file is invalid")]
    InvalidDocument,
    /// The record represents API-key or another unsupported authentication mode.
    #[error("Codex authentication mode is not ChatGPT")]
    UnsupportedAuthMode,
    /// The record contains no token bundle.
    #[error("Codex ChatGPT token bundle is missing")]
    MissingTokens,
    /// The access token is empty or not a valid JWT.
    #[error("Codex ChatGPT access token is invalid")]
    InvalidAccessToken,
    /// The access-token JWT has no absolute expiry.
    #[error("Codex ChatGPT access token expiry is missing")]
    MissingAccessTokenExpiry,
    /// The access token has already expired.
    #[error("Codex ChatGPT access token has expired")]
    ExpiredAccessToken,
    /// The ID token is not a valid JWT claim envelope.
    #[error("Codex ChatGPT identity token is invalid")]
    InvalidIdToken,
    /// The token bundle has no selected ChatGPT account/workspace binding.
    #[error("Codex ChatGPT account binding is missing")]
    MissingAccountBinding,
    /// The ID-token workspace conflicts with the selected account binding.
    #[error("Codex ChatGPT account binding is inconsistent")]
    AccountBindingMismatch,
    /// The validated bundle could not enter the purpose-restricted credential store.
    #[error("Codex ChatGPT credential could not be constructed")]
    CredentialConstruction,
}
