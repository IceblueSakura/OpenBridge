//! Validation and serialization for OpenBridge-owned OAuth2 auth documents.
//!
//! The auth envelope dispatches on `auth_mode`, so each managed Provider owns its token-bundle
//! validation and account context. Token payloads are decoded into zeroizing memory, and errors
//! never retain or report input data.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::Zeroizing;

use crate::provider::ProviderKind;

use super::error::OAuth2CredentialManagerError;

/// Provider-specific account routing context bound to an OAuth2 token bundle.
///
/// Each OAuth2 Provider defines its own context variant so account identity stays inseparable
/// from its access token. The generic lifecycle dispatches on this enum instead of hardcoding
/// one Provider's claims.
pub(crate) enum OAuth2AccountContext {
    /// ChatGPT account binding and conditional FedRAMP routing flag.
    ChatGpt {
        account_id: SecretString,
        is_fedramp_account: bool,
    },
    /// Grok subscription identity and normalized tier.
    Grok {
        subject: SecretString,
        subscription_tier: String,
    },
}

impl OAuth2AccountContext {
    /// Returns the Provider that owns this account routing context.
    pub(crate) fn provider(&self) -> ProviderKind {
        match self {
            Self::ChatGpt { .. } => ProviderKind::ChatGpt,
            Self::Grok { .. } => ProviderKind::Grok,
        }
    }

    /// Returns whether the account routing identity is unchanged across a token rotation.
    pub(crate) fn same_account(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::ChatGpt {
                    account_id: previous,
                    is_fedramp_account: previous_fedramp,
                },
                Self::ChatGpt {
                    account_id: refreshed,
                    is_fedramp_account: refreshed_fedramp,
                },
            ) => {
                // Compare the account binding in constant time before the routing flag.
                let previous = previous.expose_secret().as_bytes();
                let refreshed = refreshed.expose_secret().as_bytes();
                previous.len() == refreshed.len()
                    && previous.ct_eq(refreshed).unwrap_u8() == 1
                    && previous_fedramp == refreshed_fedramp
            }
            (
                Self::Grok {
                    subject: previous, ..
                },
                Self::Grok {
                    subject: refreshed, ..
                },
            ) => {
                // The subject is the stable identity; the tier may legitimately change.
                let previous = previous.expose_secret().as_bytes();
                let refreshed = refreshed.expose_secret().as_bytes();
                previous.len() == refreshed.len() && previous.ct_eq(refreshed).unwrap_u8() == 1
            }
            // Cross-provider context changes are always a binding mismatch.
            _ => false,
        }
    }
}

/// Complete validated token and account context owned by one OAuth2 credential.
pub(crate) struct ValidatedOAuth2Bundle {
    pub(crate) id_token: SecretString,
    pub(crate) access_token: SecretString,
    pub(crate) refresh_token: SecretString,
    pub(crate) context: OAuth2AccountContext,
    pub(crate) expires_at: SystemTime,
    pub(crate) last_refresh: Option<String>,
}

/// Parses and validates one managed OAuth2 auth document for its owning Provider.
pub(crate) fn parse_auth_document(
    provider: ProviderKind,
    document: &[u8],
    reject_expired: bool,
) -> Result<ValidatedOAuth2Bundle, OAuth2CredentialManagerError> {
    // Parse the envelope and validate the explicit Provider credential shape.
    let auth: RawAuthDotJson<'_> = serde_json::from_slice(document)
        .map_err(|_| OAuth2CredentialManagerError::InvalidDocument)?;
    let tokens = validate_auth_document(provider, &auth)?;

    // Validate the access-token lifetime and account-bound ID-token context.
    let expires_at = parse_access_token_expiry(tokens.access_token)?;
    if reject_expired && expires_at <= SystemTime::now() {
        return Err(OAuth2CredentialManagerError::ExpiredAccessToken);
    }
    let context = parse_account_context(
        provider,
        tokens.id_token,
        tokens.access_token,
        tokens.account_id,
        auth.subscription_tier,
    )?;

    // Move the complete bundle into purpose-bound secret storage.
    Ok(ValidatedOAuth2Bundle {
        id_token: SecretString::from(tokens.id_token.to_owned()),
        access_token: SecretString::from(tokens.access_token.to_owned()),
        refresh_token: SecretString::from(tokens.refresh_token.to_owned()),
        context,
        expires_at,
        last_refresh: auth.last_refresh.map(str::to_owned),
    })
}

/// Validates token-exchange values before they may become a persisted auth document.
pub(crate) fn validate_exchanged_tokens(
    provider: ProviderKind,
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
    let context = parse_account_context(provider, id_token, access_token, None, None)?;
    Ok(ValidatedOAuth2Bundle {
        id_token: SecretString::from(id_token.to_owned()),
        access_token: SecretString::from(access_token.to_owned()),
        refresh_token: SecretString::from(refresh_token.to_owned()),
        context,
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
    let refreshed = validate_exchanged_tokens(
        previous.context.provider(),
        id_token,
        access_token,
        refresh_token,
    )?;

    // Reject account routing identity changes before the rotated bundle may be persisted.
    if !previous.context.same_account(&refreshed.context) {
        return Err(OAuth2CredentialManagerError::AccountBindingMismatch);
    }

    // Preserve the stored Grok subscription tier when a refreshed token omits the tier claim.
    let context = match (&previous.context, &refreshed.context) {
        (
            OAuth2AccountContext::Grok {
                subscription_tier: previous_tier,
                ..
            },
            OAuth2AccountContext::Grok {
                subject,
                subscription_tier,
            },
        ) if subscription_tier.is_empty() && !previous_tier.is_empty() => {
            OAuth2AccountContext::Grok {
                subject: SecretString::from(subject.expose_secret().to_owned()),
                subscription_tier: previous_tier.clone(),
            }
        }
        _ => refreshed.context,
    };
    Ok(ValidatedOAuth2Bundle {
        id_token: refreshed.id_token,
        access_token: refreshed.access_token,
        refresh_token: refreshed.refresh_token,
        context,
        expires_at: refreshed.expires_at,
        last_refresh: refreshed.last_refresh,
    })
}

/// Serializes one validated bundle using the managed auth-file envelope owned by its Provider.
pub(crate) fn serialize_auth_document(
    bundle: &ValidatedOAuth2Bundle,
) -> Result<Zeroizing<Vec<u8>>, OAuth2CredentialManagerError> {
    // Require a valid timestamp before constructing a complete persistence record.
    let last_refresh = bundle
        .last_refresh
        .as_deref()
        .ok_or(OAuth2CredentialManagerError::InvalidLastRefresh)?;
    let (auth_mode, subscription_tier) = match &bundle.context {
        OAuth2AccountContext::ChatGpt { .. } => ("chatgpt", None),
        OAuth2AccountContext::Grok {
            subscription_tier, ..
        } => (
            "grok",
            // Persist the decoded tier only when the authority supplied the claim.
            Some(subscription_tier.as_str()).filter(|tier| !tier.is_empty()),
        ),
    };
    let document = SerializableAuthDotJson {
        auth_mode,
        openai_api_key: None,
        tokens: SerializableOAuth2Tokens {
            id_token: bundle.id_token.expose_secret(),
            access_token: bundle.access_token.expose_secret(),
            refresh_token: bundle.refresh_token.expose_secret(),
            account_id: match &bundle.context {
                // ChatGPT persists the selected account binding beside the token bundle.
                OAuth2AccountContext::ChatGpt { account_id, .. } => {
                    Some(account_id.expose_secret())
                }
                OAuth2AccountContext::Grok { .. } => None,
            },
        },
        last_refresh,
        subscription_tier,
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

/// Validates the managed auth envelope for its Provider and returns its complete token bundle.
fn validate_auth_document<'a>(
    provider: ProviderKind,
    auth: &'a RawAuthDotJson<'a>,
) -> Result<&'a RawOAuth2Tokens<'a>, OAuth2CredentialManagerError> {
    // Require the explicit managed mode for the owning Provider.
    let expected_mode = match provider {
        ProviderKind::ChatGpt => "chatgpt",
        ProviderKind::Grok => "grok",
        _ => return Err(OAuth2CredentialManagerError::UnsupportedProvider),
    };
    if !auth
        .auth_mode
        .is_some_and(|mode| mode.eq_ignore_ascii_case(expected_mode))
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

/// Parses Provider-owned account routing context from the token bundle.
///
/// `persisted_tier` carries the Grok subscription tier already stored in the auth document; the
/// reference client keeps the stored value when a refreshed access token omits the tier claim.
fn parse_account_context(
    provider: ProviderKind,
    id_token: &str,
    access_token: &str,
    selected_account_id: Option<&str>,
    persisted_tier: Option<&str>,
) -> Result<OAuth2AccountContext, OAuth2CredentialManagerError> {
    match provider {
        ProviderKind::ChatGpt => {
            // Borrow only the nested ChatGPT auth claims from the decoded payload.
            let payload = decode_jwt_payload(id_token)
                .map_err(|_| OAuth2CredentialManagerError::InvalidIdToken)?;
            let claims: ChatGptIdTokenClaims<'_> = serde_json::from_slice(&payload)
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
            Ok(OAuth2AccountContext::ChatGpt {
                account_id: SecretString::from(account_id.to_owned()),
                is_fedramp_account,
            })
        }
        ProviderKind::Grok => {
            // Decode both token payloads; the reference client reads `sub` from either one.
            let id_payload = decode_jwt_payload(id_token)
                .map_err(|_| OAuth2CredentialManagerError::InvalidIdToken)?;
            let id_claims: GrokIdTokenClaims<'_> = serde_json::from_slice(&id_payload)
                .map_err(|_| OAuth2CredentialManagerError::InvalidIdToken)?;
            let access_payload = decode_jwt_payload(access_token)
                .map_err(|_| OAuth2CredentialManagerError::InvalidAccessToken)?;
            let access_claims: GrokAccessTokenClaims = serde_json::from_slice(&access_payload)
                .map_err(|_| OAuth2CredentialManagerError::InvalidAccessToken)?;

            // Require one OIDC subject and reject token pairs whose subjects disagree, so a
            // refresh can never bind another subject's access token to the stored account.
            let id_subject = id_claims.sub.filter(|value| !value.trim().is_empty());
            let access_subject = access_claims.sub.filter(|value| !value.trim().is_empty());
            if let (Some(id_subject), Some(access_subject)) = (id_subject, access_subject) {
                let id_bytes = id_subject.as_bytes();
                let access_bytes = access_subject.as_bytes();
                if id_bytes.len() != access_bytes.len()
                    || id_bytes.ct_eq(access_bytes).unwrap_u8() != 1
                {
                    return Err(OAuth2CredentialManagerError::AccountBindingMismatch);
                }
            }
            let subject = id_subject
                .or(access_subject)
                .ok_or(OAuth2CredentialManagerError::MissingAccountBinding)?;

            // Decode the subscription tier claim without signature verification; it is metadata.
            // Fall back to the persisted document value when the token omits the claim.
            let subscription_tier = access_claims.tier.map_or_else(
                || {
                    persisted_tier
                        .filter(|tier| !tier.trim().is_empty())
                        .map_or_else(String::new, str::to_owned)
                },
                normalize_grok_tier,
            );
            Ok(OAuth2AccountContext::Grok {
                subject: SecretString::from(subject.to_owned()),
                subscription_tier,
            })
        }
        _ => Err(OAuth2CredentialManagerError::UnsupportedProvider),
    }
}

/// Canonicalizes a Grok subscription tier claim to its stable snake_case identifier.
///
/// The numeric mapping and aliases mirror the pinned reference client's JWT claim handling,
/// including numeric-string labels and integral floating-point claims.
fn normalize_grok_tier(tier: GrokTierClaim) -> String {
    match tier {
        GrokTierClaim::Numeric(value) => grok_numeric_tier(value),
        GrokTierClaim::Float(value) if value >= 0.0 && value.fract() == 0.0 => {
            grok_numeric_tier(value as u64)
        }
        GrokTierClaim::Float(_) => String::new(),
        GrokTierClaim::Text(raw) => grok_text_tier(&raw),
    }
}

/// Maps the pinned numeric claim table; unknown tiers keep their decimal form.
fn grok_numeric_tier(value: u64) -> String {
    match value {
        0 => "free",
        1 => "supergrok",
        2 => "x_basic",
        3 => "x_premium",
        4 => "x_premium_plus",
        5 => "supergrok_heavy",
        6 => "supergrok_lite",
        7 => "supergrok_plus",
        other => return other.to_string(),
    }
    .to_owned()
}

/// Folds separators into underscores, resolves numeric labels, then matches the alias table.
fn grok_text_tier(raw: &str) -> String {
    let folded = raw
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");
    // The reference client parses numeric labels through the same numeric claim table.
    if let Ok(value) = folded.parse::<u64>() {
        return grok_numeric_tier(value);
    }
    match folded.as_str() {
        "" => String::new(),
        "free" | "grok_free" | "grokfree" | "free_tier" | "freetier" | "grok_basic"
        | "grokbasic" => "free".to_owned(),
        "supergrok" | "grokpro" => "supergrok".to_owned(),
        "supergrok_lite" | "supergroklite" => "supergrok_lite".to_owned(),
        "supergrok_heavy" | "supergrokheavy" => "supergrok_heavy".to_owned(),
        "supergrok_pro" | "supergrokpro" => "supergrok_pro".to_owned(),
        "supergrok_plus" | "supergrokplus" => "supergrok_plus".to_owned(),
        "x_basic" | "xbasic" | "basic" => "x_basic".to_owned(),
        "x_premium" | "xpremium" => "x_premium".to_owned(),
        "x_premium_plus" | "xpremiumplus" | "x_premium+" => "x_premium_plus".to_owned(),
        _ => folded,
    }
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
    /// Persisted Grok subscription tier carried across refreshes that omit the tier claim.
    #[serde(default)]
    subscription_tier: Option<&'a str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_tier: Option<&'a str>,
}

#[derive(Serialize)]
struct SerializableOAuth2Tokens<'a> {
    id_token: &'a str,
    access_token: &'a str,
    refresh_token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_id: Option<&'a str>,
}

#[derive(Deserialize)]
struct AccessTokenClaims {
    #[serde(default)]
    exp: Option<i64>,
}

#[derive(Deserialize)]
struct ChatGptIdTokenClaims<'a> {
    #[serde(rename = "https://api.openai.com/auth", default, borrow)]
    auth: Option<ChatGptIdTokenAuthClaims<'a>>,
}

#[derive(Deserialize)]
struct ChatGptIdTokenAuthClaims<'a> {
    #[serde(default)]
    chatgpt_account_id: Option<&'a str>,
    #[serde(default)]
    chatgpt_account_is_fedramp: bool,
}

#[derive(Deserialize)]
struct GrokIdTokenClaims<'a> {
    #[serde(default)]
    sub: Option<&'a str>,
}

#[derive(Deserialize)]
struct GrokAccessTokenClaims<'a> {
    #[serde(default)]
    sub: Option<&'a str>,
    #[serde(default)]
    tier: Option<GrokTierClaim>,
}

/// Grok subscription tier claim, which the authority encodes as a number or a label.
#[derive(Deserialize)]
#[serde(untagged)]
enum GrokTierClaim {
    /// Numeric `SubscriptionTier` claim from the access token.
    Numeric(u64),
    /// Integral floating-point tier emitted by some serializers.
    Float(f64),
    /// Free-form or numeric-string tier label emitted by some authority responses.
    Text(String),
}
