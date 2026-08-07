//! ChatGPT OAuth2 device-flow HTTP transport and wire types.

use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use http::{StatusCode, header};
use reqwest::{Client, Response, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, de::Error as _};
use zeroize::Zeroizing;

use crate::providers::chatgpt::oauth::ChatGptOAuthRegistration;

use super::OAuth2LoginError;

const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// Transport abstraction that keeps deterministic tests independent from real HTTPS endpoints.
pub(super) trait ChatGptOAuthTransport {
    /// Requests a new private device interaction.
    fn request_device_session(
        &self,
    ) -> impl Future<Output = Result<DeviceSession, TransportError>> + Send;

    /// Polls an existing private device interaction once.
    fn poll_device_session(
        &self,
        session: &DeviceSession,
    ) -> impl Future<Output = Result<DevicePoll, TransportError>> + Send;

    /// Exchanges a validated authorization code and PKCE verifier for tokens.
    fn exchange_authorization_code(
        &self,
        grant: &PkceGrant,
    ) -> impl Future<Output = Result<ExchangedTokens, TransportError>> + Send;
}

/// HTTPS transport restricted to the compile-time ChatGPT OAuth registration.
pub(super) struct ReqwestChatGptOAuthTransport {
    client: Client,
    registration: &'static ChatGptOAuthRegistration,
}

impl ReqwestChatGptOAuthTransport {
    /// Builds a no-redirect client with the fixed per-request timeout.
    pub(super) fn new(
        registration: &'static ChatGptOAuthRegistration,
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

impl ChatGptOAuthTransport for ReqwestChatGptOAuthTransport {
    async fn request_device_session(&self) -> Result<DeviceSession, TransportError> {
        // Encode only the compile-time public client identity for device-session creation.
        let body = serde_json::to_vec(&serde_json::json!({
            "client_id": self.registration.client_id,
        }))
        .map_err(|_| TransportError::InvalidResponse)?;
        let response = self
            .client
            .post(self.registration.device_authorization_endpoint)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Parse a bounded success body and normalize HTTP failures without reading error bodies.
        parse_json_response(response).await
    }

    async fn poll_device_session(
        &self,
        session: &DeviceSession,
    ) -> Result<DevicePoll, TransportError> {
        // Encode the short-lived session identifiers only for the fixed private poll endpoint.
        let body = Zeroizing::new(
            serde_json::to_vec(&serde_json::json!({
                "device_auth_id": session.device_auth_id.expose_secret(),
                "user_code": session.user_code.expose_secret(),
            }))
            .map_err(|_| TransportError::InvalidResponse)?,
        );
        let response = self
            .client
            .post(self.registration.device_poll_endpoint)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.to_vec())
            .send()
            .await
            .map_err(|_| TransportError::Network)?;

        // Treat only the observed private pending statuses as a non-terminal poll result.
        match response.status() {
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => Ok(DevicePoll::Pending),
            StatusCode::OK => parse_json_body(response).await.map(DevicePoll::Authorized),
            StatusCode::TOO_MANY_REQUESTS => {
                Err(TransportError::RateLimited(parse_retry_after(&response)))
            }
            status => Err(TransportError::HttpStatus(status.as_u16())),
        }
    }

    async fn exchange_authorization_code(
        &self,
        grant: &PkceGrant,
    ) -> Result<ExchangedTokens, TransportError> {
        // Form-encode the authorization code and verifier for the fixed OAuth token endpoint.
        let body = Zeroizing::new({
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer
                .append_pair("grant_type", "authorization_code")
                .append_pair("code", grant.authorization_code.expose_secret())
                .append_pair("redirect_uri", self.registration.redirect_uri)
                .append_pair("client_id", self.registration.client_id)
                .append_pair("code_verifier", grant.code_verifier.expose_secret());
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

        // Parse only a bounded successful exchange document.
        parse_json_response(response).await
    }
}

/// Parses a bounded JSON body only for HTTP success and classifies other statuses safely.
async fn parse_json_response<T>(response: Response) -> Result<T, TransportError>
where
    T: for<'de> Deserialize<'de>,
{
    match response.status() {
        StatusCode::OK => parse_json_body(response).await,
        StatusCode::TOO_MANY_REQUESTS => {
            Err(TransportError::RateLimited(parse_retry_after(&response)))
        }
        status => Err(TransportError::HttpStatus(status.as_u16())),
    }
}

/// Buffers one successful response under the strict OAuth document size limit.
async fn parse_json_body<T>(response: Response) -> Result<T, TransportError>
where
    T: for<'de> Deserialize<'de>,
{
    // Collect chunks while rejecting a body that reaches beyond the configured bound.
    let mut body = Zeroizing::new(Vec::new());
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| TransportError::Network)?;
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or(TransportError::BodyTooLarge)?;
        if next_len > MAX_RESPONSE_BODY_BYTES {
            return Err(TransportError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk);
    }

    // Deserialize into owned secret wrappers and zero the source buffer on return.
    serde_json::from_slice(&body).map_err(|_| TransportError::InvalidResponse)
}

/// Parses a bounded delta-seconds Retry-After hint without retaining header text.
fn parse_retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[derive(Deserialize)]
pub(super) struct DeviceSession {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) device_auth_id: SecretString,
    #[serde(alias = "usercode", deserialize_with = "deserialize_non_empty_secret")]
    pub(super) user_code: SecretString,
    #[serde(default, deserialize_with = "deserialize_optional_interval")]
    pub(super) interval: Option<Duration>,
}

pub(super) enum DevicePoll {
    Pending,
    Authorized(PkceGrant),
}

#[derive(Deserialize)]
pub(super) struct PkceGrant {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) authorization_code: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) code_challenge: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) code_verifier: SecretString,
}

#[derive(Deserialize)]
pub(super) struct ExchangedTokens {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) id_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) access_token: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) refresh_token: SecretString,
}

/// Moves one non-empty JSON string directly into a zeroizing secret wrapper.
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

/// Accepts a numeric or string interval and converts positive seconds into a duration.
fn deserialize_optional_interval<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let seconds = match value {
        None | Some(serde_json::Value::Null) => return Ok(None),
        Some(serde_json::Value::Number(number)) => number.as_u64(),
        Some(serde_json::Value::String(value)) => value.parse::<u64>().ok(),
        Some(_) => None,
    }
    .ok_or_else(|| D::Error::custom("interval must be positive seconds"))?;
    if seconds == 0 {
        return Err(D::Error::custom("interval must be positive seconds"));
    }
    Ok(Some(Duration::from_secs(seconds)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransportError {
    Network,
    HttpStatus(u16),
    RateLimited(Option<Duration>),
    InvalidResponse,
    BodyTooLarge,
}
