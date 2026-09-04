//! Grok OAuth2 device-flow HTTP transport and wire types.
//!
//! The transport posts to the compile-time device authorization endpoint and polls the ordinary
//! token endpoint with the RFC 8628 device-code grant; no secondary approval endpoints exist in
//! the standard flow.

use std::{future::Future, time::Duration};

use http::{StatusCode, header};
use reqwest::{Client, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, de::Error as _};
use zeroize::Zeroizing;

use crate::providers::grok::oauth::GrokOAuthRegistration;

use super::super::common::{
    ExchangedTokens, OAuth2LoginError, TransportError, deserialize_non_empty_secret,
    deserialize_optional_interval, parse_json_body, parse_retry_after,
};

/// Transport abstraction that keeps deterministic tests independent from real HTTPS endpoints.
pub(super) trait GrokOAuthTransport {
    /// Requests one standard device authorization session.
    fn request_device_session(
        &self,
    ) -> impl Future<Output = Result<GrokDeviceSession, TransportError>> + Send;

    /// Polls the token endpoint once for the device session's terminal result.
    fn poll_for_tokens(
        &self,
        session: &GrokDeviceSession,
    ) -> impl Future<Output = Result<GrokTokenPoll, TransportError>> + Send;
}

/// HTTPS transport restricted to the compile-time Grok OAuth registration.
pub(super) struct ReqwestGrokOAuthTransport {
    client: Client,
    registration: &'static GrokOAuthRegistration,
}

impl ReqwestGrokOAuthTransport {
    /// Builds a no-redirect client with the fixed per-request timeout.
    pub(super) fn new(
        registration: &'static GrokOAuthRegistration,
    ) -> Result<Self, OAuth2LoginError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(registration.request_timeout)
            .build()
            .map_err(|_| OAuth2LoginError::Client)?;
        Ok(Self {
            client,
            registration,
        })
    }
}

impl GrokOAuthTransport for ReqwestGrokOAuthTransport {
    async fn request_device_session(&self) -> Result<GrokDeviceSession, TransportError> {
        // Form-encode only the compile-time public client identity and scope.
        let body = Zeroizing::new({
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer
                .append_pair("client_id", self.registration.client_id)
                .append_pair("scope", self.registration.scope);
            serializer.finish()
        });
        let response = self
            .client
            .post(self.registration.device_authorization_endpoint)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body.to_string())
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Parse a bounded success body and normalize HTTP failures without reading error bodies.
        match response.status() {
            StatusCode::OK => parse_json_body(response).await,
            StatusCode::TOO_MANY_REQUESTS => {
                Err(TransportError::RateLimited(parse_retry_after(&response)))
            }
            status => Err(TransportError::HttpStatus(status.as_u16())),
        }
    }

    async fn poll_for_tokens(
        &self,
        session: &GrokDeviceSession,
    ) -> Result<GrokTokenPoll, TransportError> {
        // Form-encode the RFC 8628 grant with the short-lived device code.
        let body = Zeroizing::new({
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer
                .append_pair("grant_type", "urn:ietf:params:oauth:grant-type:device_code")
                .append_pair("client_id", self.registration.client_id)
                .append_pair("device_code", session.device_code.expose_secret());
            serializer.finish()
        });
        let response = self
            .client
            .post(self.registration.token_endpoint)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body.to_string())
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Parse one bounded JSON document; RFC 8628 error payloads accompany non-success statuses.
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(TransportError::RateLimited(parse_retry_after(&response)));
        }
        let payload: TokenPollPayload = parse_json_body(response).await?;

        // Successful grants carry identity and refresh tokens beside the access token.
        if let Some(access_token) = payload.access_token {
            return match (payload.id_token, payload.refresh_token) {
                (Some(id_token), Some(refresh_token)) => {
                    Ok(GrokTokenPoll::Tokens(ExchangedTokens {
                        id_token,
                        access_token,
                        refresh_token,
                    }))
                }
                _ => Err(TransportError::InvalidResponse),
            };
        }
        match payload.error.as_deref() {
            Some("authorization_pending") => Ok(GrokTokenPoll::Pending),
            Some("slow_down") => Ok(GrokTokenPoll::SlowDown),
            Some("access_denied") => Ok(GrokTokenPoll::AccessDenied),
            Some("expired_token") => Ok(GrokTokenPoll::ExpiredToken),
            Some(_) | None => Err(TransportError::InvalidResponse),
        }
    }
}

/// Standard device authorization session fields advertised by the authority.
#[derive(Deserialize)]
pub(super) struct GrokDeviceSession {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) device_code: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) user_code: SecretString,
    #[serde(deserialize_with = "deserialize_session_lifetime")]
    pub(super) expires_in: Duration,
    #[serde(default, deserialize_with = "deserialize_optional_interval")]
    pub(super) interval: Option<Duration>,
}

/// Terminal or pending outcome of one device-code token poll.
pub(super) enum GrokTokenPoll {
    /// The administrator has not approved the session yet.
    Pending,
    /// The authority asked the client to increase its polling interval.
    SlowDown,
    /// The administrator or authority denied the session.
    AccessDenied,
    /// The device session expired before approval.
    ExpiredToken,
    /// The complete token bundle issued for the approved session.
    Tokens(ExchangedTokens),
}

#[derive(Deserialize)]
struct TokenPollPayload {
    #[serde(default, deserialize_with = "deserialize_optional_secret")]
    access_token: Option<SecretString>,
    #[serde(default, deserialize_with = "deserialize_optional_secret")]
    id_token: Option<SecretString>,
    #[serde(default, deserialize_with = "deserialize_optional_secret")]
    refresh_token: Option<SecretString>,
    #[serde(default)]
    error: Option<String>,
}

/// Accepts a positive integer second count as the session lifetime.
fn deserialize_session_lifetime<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = u64::deserialize(deserializer)?;
    if seconds == 0 {
        return Err(D::Error::custom("expires_in must be positive seconds"));
    }
    Ok(Duration::from_secs(seconds))
}

/// Moves an optional non-empty JSON string into a zeroizing secret wrapper.
fn deserialize_optional_secret<'de, D>(deserializer: D) -> Result<Option<SecretString>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(value) if value.trim().is_empty() => {
            Err(D::Error::custom("secret value must not be empty"))
        }
        Some(value) => Ok(Some(SecretString::from(value))),
    }
}
