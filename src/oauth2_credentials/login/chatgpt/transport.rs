//! ChatGPT private device-flow HTTP transport and wire types.
//!
//! Every wire shape in this module belongs to the private Codex device interaction, not to the
//! standard OAuth device grant. Protocol drift in this flow changes this file and the ChatGPT
//! state machine only.

use std::{future::Future, time::Duration};

use http::{StatusCode, header};
use reqwest::{Client, Response, redirect::Policy};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::providers::chatgpt::oauth::ChatGptOAuthRegistration;

use super::super::common::{
    ExchangedTokens, OAuth2LoginError, TransportError, deserialize_non_empty_secret,
    deserialize_optional_interval, parse_json_body, parse_retry_after,
};

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

/// One private device interaction session created by the authority.
#[derive(Deserialize)]
pub(super) struct DeviceSession {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) device_auth_id: SecretString,
    #[serde(alias = "usercode", deserialize_with = "deserialize_non_empty_secret")]
    pub(super) user_code: SecretString,
    #[serde(default, deserialize_with = "deserialize_optional_interval")]
    pub(super) interval: Option<Duration>,
}

/// Terminal or pending outcome of one private device poll.
pub(super) enum DevicePoll {
    /// The administrator has not completed the interaction yet.
    Pending,
    /// The interaction produced authorization material ready for PKCE verification.
    Authorized(PkceGrant),
}

/// Authorization material returned by the private poll endpoint before token exchange.
#[derive(Deserialize)]
pub(super) struct PkceGrant {
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) authorization_code: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) code_challenge: SecretString,
    #[serde(deserialize_with = "deserialize_non_empty_secret")]
    pub(super) code_verifier: SecretString,
}
