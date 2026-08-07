//! Validation and serialization for OpenBridge-owned ChatGPT OAuth documents.
//!
//! This module parses only the credential fields required by the trusted ChatGPT registration.
//! Token payloads are decoded into zeroizing memory, and errors never retain or report input data.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::Zeroizing;

use super::error::OAuth2CredentialManagerError;

/// Complete validated token and account context owned by one OAuth2 credential.
pub(crate) struct ValidatedOAuth2Bundle {
    pub(crate) id_token: SecretString,
    pub(crate) access_token: SecretString,
    pub(crate) refresh_token: SecretString,
    pub(crate) account_id: SecretString,
    pub(crate) is_fedramp_account: bool,
    pub(crate) expires_at: SystemTime,
    pub(crate) last_refresh: Option<String>,
}

/// Parses and validates one managed ChatGPT auth document.
pub(crate) fn parse_auth_document(
    document: &[u8],
    reject_expired: bool,
) -> Result<ValidatedOAuth2Bundle, OAuth2CredentialManagerError> {
    // Parse the envelope and validate the explicit ChatGPT credential shape.
    let auth: RawAuthDotJson<'_> = serde_json::from_slice(document)
        .map_err(|_| OAuth2CredentialManagerError::InvalidDocument)?;
    let tokens = validate_auth_document(&auth)?;

    // Validate the access-token lifetime and account-bound ID-token context.
    let expires_at = parse_access_token_expiry(tokens.access_token)?;
    if reject_expired && expires_at <= SystemTime::now() {
        return Err(OAuth2CredentialManagerError::ExpiredAccessToken);
    }
    let (account_id, is_fedramp_account) =
        parse_id_token_context(tokens.id_token, tokens.account_id)?;

    // Move the complete bundle into purpose-bound secret storage.
    Ok(ValidatedOAuth2Bundle {
        id_token: SecretString::from(tokens.id_token.to_owned()),
        access_token: SecretString::from(tokens.access_token.to_owned()),
        refresh_token: SecretString::from(tokens.refresh_token.to_owned()),
        account_id: SecretString::from(account_id.to_string()),
        is_fedramp_account,
        expires_at,
        last_refresh: auth.last_refresh.map(str::to_owned),
    })
}

/// Validates token-exchange values before they may become a persisted auth document.
pub(crate) fn validate_exchanged_tokens(
    id_token: &str,
    access_token: &str,
    refresh_token: &str,
) -> Result<ValidatedOAuth2Bundle, OAuth2CredentialManagerError> {
    // Reject incomplete exchange responses before decoding any token payload.
    if id_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidIdToken);
    }
    if access_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidAccessToken);
    }
    if refresh_token.trim().is_empty() {
        return Err(OAuth2CredentialManagerError::InvalidRefreshToken);
    }

    // Validate lifetime and account context against the same rules as startup loading.
    let expires_at = parse_access_token_expiry(access_token)?;
    if expires_at <= SystemTime::now() {
        return Err(OAuth2CredentialManagerError::ExpiredAccessToken);
    }
    let (account_id, is_fedramp_account) = parse_id_token_context(id_token, None)?;
    Ok(ValidatedOAuth2Bundle {
        id_token: SecretString::from(id_token.to_owned()),
        access_token: SecretString::from(access_token.to_owned()),
        refresh_token: SecretString::from(refresh_token.to_owned()),
        account_id: SecretString::from(account_id.to_string()),
        is_fedramp_account,
        expires_at,
        last_refresh: Some(current_timestamp()?),
    })
}

/// Validates a refresh response while preserving omitted optional tokens and account identity.
pub(crate) fn validate_refreshed_tokens(
    previous: &ValidatedOAuth2Bundle,
    id_token: Option<&str>,
    access_token: &str,
    refresh_token: Option<&str>,
) -> Result<ValidatedOAuth2Bundle, OAuth2CredentialManagerError> {
    // Preserve optional tokens only when the authority omitted them entirely.
    let id_token = id_token.unwrap_or_else(|| previous.id_token.expose_secret());
    let refresh_token = refresh_token.unwrap_or_else(|| previous.refresh_token.expose_secret());
    let refreshed = validate_exchanged_tokens(id_token, access_token, refresh_token)?;

    // Reject account or FedRAMP identity changes before the rotated bundle may be persisted.
    let previous_account = previous.account_id.expose_secret().as_bytes();
    let refreshed_account = refreshed.account_id.expose_secret().as_bytes();
    if previous_account.len() != refreshed_account.len()
        || previous_account.ct_eq(refreshed_account).unwrap_u8() != 1
        || previous.is_fedramp_account != refreshed.is_fedramp_account
    {
        return Err(OAuth2CredentialManagerError::AccountBindingMismatch);
    }
    Ok(refreshed)
}

/// Serializes one validated bundle using the existing managed auth-file envelope.
pub(crate) fn serialize_auth_document(
    bundle: &ValidatedOAuth2Bundle,
) -> Result<Zeroizing<Vec<u8>>, OAuth2CredentialManagerError> {
    // Require a valid timestamp before constructing a complete persistence record.
    let last_refresh = bundle
        .last_refresh
        .as_deref()
        .ok_or(OAuth2CredentialManagerError::InvalidLastRefresh)?;
    let document = SerializableAuthDotJson {
        auth_mode: "chatgpt",
        openai_api_key: None,
        tokens: SerializableOAuth2Tokens {
            id_token: bundle.id_token.expose_secret(),
            access_token: bundle.access_token.expose_secret(),
            refresh_token: bundle.refresh_token.expose_secret(),
            account_id: bundle.account_id.expose_secret(),
        },
        last_refresh,
    };

    // Serialize directly into zeroizing bytes and append one conventional trailing newline.
    let mut bytes = Zeroizing::new(
        serde_json::to_vec_pretty(&document)
            .map_err(|_| OAuth2CredentialManagerError::InvalidDocument)?,
    );
    bytes.push(b'\n');
    Ok(bytes)
}

/// Returns the current UTC timestamp in the auth envelope's RFC 3339 form.
pub(crate) fn current_timestamp() -> Result<String, OAuth2CredentialManagerError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| OAuth2CredentialManagerError::InvalidLastRefresh)
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

#[derive(Serialize)]
struct SerializableAuthDotJson<'a> {
    auth_mode: &'static str,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<&'a str>,
    tokens: SerializableOAuth2Tokens<'a>,
    last_refresh: &'a str,
}

#[derive(Serialize)]
struct SerializableOAuth2Tokens<'a> {
    id_token: &'a str,
    access_token: &'a str,
    refresh_token: &'a str,
    account_id: &'a str,
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
