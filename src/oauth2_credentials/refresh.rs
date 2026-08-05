//! Fixed ChatGPT refresh-grant transport and response classification.
//!
//! The adapter sends only to the compile-time token endpoint and never exposes response bodies.
//! It distinguishes confirmed pre-response throttling/server failures, terminal OAuth codes, and
//! outcomes that may have consumed a rotating refresh token.

use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use http::{StatusCode, header};
use reqwest::{Client, Response, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, de::Error as _};
use zeroize::Zeroizing;

use crate::providers::chatgpt::oauth::ChatGptOAuthRegistration;

const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;
const REFRESH_SCOPE: &str = "openid profile email";

/// Successful refresh fields, with optional rotated identity and refresh tokens.
pub(crate) struct RefreshTokenResponse {
    pub(crate) access_token: SecretString,
    pub(crate) id_token: Option<SecretString>,
    pub(crate) refresh_token: Option<SecretString>,
    pub(crate) token_type: Option<String>,
}

impl RefreshTokenResponse {
    /// Rejects a present token type unless it is the expected Bearer scheme.
    pub(crate) fn validate_token_type(&self) -> Result<(), RefreshTransportError> {
        if self
            .token_type
            .as_deref()
            .is_some_and(|value| !value.eq_ignore_ascii_case("bearer"))
        {
            return Err(RefreshTransportError::Ambiguous);
        }
        Ok(())
    }
}

/// Transport abstraction for deterministic refresh lifecycle tests.
pub(crate) trait ChatGptRefreshTransport: Send + Sync {
    /// Exchanges one purpose-bound refresh token through the fixed Provider registration.
    fn refresh(
        &self,
        refresh_token: &SecretString,
    ) -> impl Future<Output = Result<RefreshTokenResponse, RefreshTransportError>> + Send;
}

/// Reqwest transport restricted to the compile-time ChatGPT OAuth registration.
pub(crate) struct ReqwestChatGptRefreshTransport {
    client: Client,
    registration: &'static ChatGptOAuthRegistration,
}

impl ReqwestChatGptRefreshTransport {
    /// Builds a no-redirect client with the Provider-specific request timeout.
    pub(crate) fn new(
        registration: &'static ChatGptOAuthRegistration,
    ) -> Result<Self, RefreshTransportError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(registration.request_timeout)
            .build()
            .map_err(|_| RefreshTransportError::Transient { retry_after: None })?;
        Ok(Self {
            client,
            registration,
        })
    }
}

impl ChatGptRefreshTransport for ReqwestChatGptRefreshTransport {
    async fn refresh(
        &self,
        refresh_token: &SecretString,
    ) -> Result<RefreshTokenResponse, RefreshTransportError> {
        // Form-encode the fixed public client, grant, scope, and purpose-bound refresh token.
        let body = Zeroizing::new({
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer
                .append_pair("grant_type", "refresh_token")
                .append_pair("refresh_token", refresh_token.expose_secret())
                .append_pair("client_id", self.registration.client_id)
                .append_pair("scope", REFRESH_SCOPE);
            serializer.finish()
        });
        let response = self
            .client
            .post(self.registration.token_endpoint)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ACCEPT, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(classify_request_error)?;

        // Classify the status before reading any bounded response body.
        classify_response(response).await
    }
}

/// Classifies connection failures as retryable only when no HTTP response could exist.
fn classify_request_error(error: reqwest::Error) -> RefreshTransportError {
    if error.is_connect() {
        RefreshTransportError::Transient { retry_after: None }
    } else {
        RefreshTransportError::Ambiguous
    }
}

/// Parses successful tokens or recognizes bounded terminal/transient authority responses.
async fn classify_response(
    response: Response,
) -> Result<RefreshTokenResponse, RefreshTransportError> {
    let status = response.status();
    match status {
        StatusCode::OK => {
            let tokens: RefreshTokenResponse = parse_json_body(response)
                .await
                .map_err(|_| RefreshTransportError::Ambiguous)?;
            tokens.validate_token_type()?;
            Ok(tokens)
        }
        StatusCode::TOO_MANY_REQUESTS => Err(RefreshTransportError::Transient {
            retry_after: parse_retry_after(&response),
        }),
        status if status.is_server_error() => {
            Err(RefreshTransportError::Transient { retry_after: None })
        }
        status if status.is_client_error() => {
            let body = read_bounded_body(response)
                .await
                .map_err(|_| RefreshTransportError::ReauthRequired(RefreshTerminalReason::Other))?;
            Err(classify_oauth_error(&body))
        }
        _ => Err(RefreshTransportError::ReauthRequired(
            RefreshTerminalReason::Other,
        )),
    }
}

/// Recognizes only exact OAuth codes while treating every other client rejection as terminal.
fn classify_oauth_error(body: &[u8]) -> RefreshTransportError {
    // Parse only the structured error selector and discard every descriptive field.
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return RefreshTransportError::ReauthRequired(RefreshTerminalReason::Other);
    };
    let error = value.get("error");
    let code = match error {
        Some(serde_json::Value::String(code)) => Some(code.as_str()),
        Some(serde_json::Value::Object(error)) => error
            .get("code")
            .or_else(|| error.get("type"))
            .and_then(serde_json::Value::as_str),
        _ => None,
    };
    // Map only known exact codes to bounded value-free reasons.
    let reason = match code {
        Some("invalid_grant") => RefreshTerminalReason::InvalidGrant,
        Some("invalid_token") => RefreshTerminalReason::InvalidToken,
        Some("invalid_request") => RefreshTerminalReason::InvalidRequest,
        Some("refresh_token_reused") => RefreshTerminalReason::RefreshTokenReused,
        Some("token_revoked") => RefreshTerminalReason::TokenRevoked,
        None | Some(_) => RefreshTerminalReason::Other,
    };
    RefreshTransportError::ReauthRequired(reason)
}

/// Parses a successful bounded JSON document into secret-owning fields.
async fn parse_json_body<T>(response: Response) -> Result<T, ()>
where
    T: for<'de> Deserialize<'de>,
{
    let body = read_bounded_body(response).await?;
    serde_json::from_slice(&body).map_err(|_| ())
}

/// Reads at most one small OAuth document and zeros the buffer when it leaves scope.
async fn read_bounded_body(response: Response) -> Result<Zeroizing<Vec<u8>>, ()> {
    // Collect response chunks while enforcing the independent OAuth body limit.
    let mut body = Zeroizing::new(Vec::new());
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ())?;
        let next_len = body.len().checked_add(chunk.len()).ok_or(())?;
        if next_len > MAX_RESPONSE_BODY_BYTES {
            return Err(());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Parses a safe bounded delta-seconds Retry-After hint.
fn parse_retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[derive(Deserialize)]
struct RawRefreshTokenResponse {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    access_token: SecretString,
    #[serde(default, deserialize_with = "deserialize_optional_secret")]
    id_token: Option<SecretString>,
    #[serde(default, deserialize_with = "deserialize_optional_secret")]
    refresh_token: Option<SecretString>,
    #[serde(default)]
    token_type: Option<String>,
}

impl<'de> Deserialize<'de> for RefreshTokenResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize strings directly into secret wrappers before validating token type later.
        let raw = RawRefreshTokenResponse::deserialize(deserializer)?;
        Ok(Self {
            access_token: raw.access_token,
            id_token: raw.id_token,
            refresh_token: raw.refresh_token,
            token_type: raw.token_type,
        })
    }
}

/// Moves one non-empty string into a zeroizing secret wrapper.
fn deserialize_non_empty_secret<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Err(D::Error::custom("secret value must not be empty"));
    }
    Ok(SecretString::from(value))
}

/// Preserves omission while rejecting a present blank optional token.
fn deserialize_optional_secret<'de, D>(deserializer: D) -> Result<Option<SecretString>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value {
        Some(value) if value.trim().is_empty() => {
            Err(D::Error::custom("optional secret value must not be empty"))
        }
        Some(value) => Ok(Some(SecretString::from(value))),
        None => Ok(None),
    }
}

/// Value-free refresh transport outcome used by the manager state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefreshTransportError {
    /// The request can be retried after a bounded delay without assuming rotation succeeded.
    Transient { retry_after: Option<Duration> },
    /// The authority rejected the credential and interactive login is required.
    ReauthRequired(RefreshTerminalReason),
    /// The request may have consumed a rotating token, so the old token must not be reused.
    Ambiguous,
}

/// Bounded terminal authority classification containing no upstream text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefreshTerminalReason {
    /// The refresh grant is no longer valid.
    InvalidGrant,
    /// The supplied token is invalid.
    InvalidToken,
    /// The authority rejected the refresh request shape or registration.
    InvalidRequest,
    /// A rotated refresh token was reused.
    RefreshTokenReused,
    /// The refresh token was revoked.
    TokenRevoked,
    /// Any unrecognized or unstructured client rejection.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_response_requires_access_and_preserves_optional_token_omission() {
        // Parse the minimal response accepted by current reference clients.
        let response: RefreshTokenResponse =
            serde_json::from_str(r#"{"access_token":"synthetic-access","token_type":"Bearer"}"#)
                .unwrap();
        assert_eq!(response.access_token.expose_secret(), "synthetic-access");
        assert!(response.id_token.is_none());
        assert!(response.refresh_token.is_none());
        assert_eq!(response.validate_token_type(), Ok(()));

        // Reject missing access tokens, blank rotations, and non-Bearer token types.
        assert!(serde_json::from_str::<RefreshTokenResponse>(r#"{}"#).is_err());
        assert!(
            serde_json::from_str::<RefreshTokenResponse>(
                r#"{"access_token":"a","refresh_token":" "}"#
            )
            .is_err()
        );
        let response: RefreshTokenResponse =
            serde_json::from_str(r#"{"access_token":"a","token_type":"mac"}"#).unwrap();
        assert_eq!(
            response.validate_token_type(),
            Err(RefreshTransportError::Ambiguous)
        );
    }

    #[test]
    fn oauth_error_classification_is_value_free_and_terminal() {
        // Recognize both OAuth string and nested OpenAI-style error objects.
        for (body, reason) in [
            (
                br#"{"error":"invalid_grant"}"#.as_slice(),
                RefreshTerminalReason::InvalidGrant,
            ),
            (
                br#"{"error":{"code":"refresh_token_reused"}}"#.as_slice(),
                RefreshTerminalReason::RefreshTokenReused,
            ),
            (
                br#"{"error":"synthetic-unknown"}"#.as_slice(),
                RefreshTerminalReason::Other,
            ),
            (b"not-json".as_slice(), RefreshTerminalReason::Other),
        ] {
            assert_eq!(
                classify_oauth_error(body),
                RefreshTransportError::ReauthRequired(reason)
            );
        }
    }
}
