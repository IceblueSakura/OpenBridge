//! Trusted OAuth registration for ChatGPT subscription access.
//!
//! The device interaction is the private Codex flow observed in the pinned reference clients: it
//! yields an authorization code and PKCE material before the ordinary token exchange. None of
//! these endpoints or the public client registration can be overridden by runtime configuration.

use std::time::Duration;

use crate::oauth2_credentials::OAuth2RefreshParameters;

/// Fixed OAuth endpoints, public client identity, and timing policy for ChatGPT login and refresh.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChatGptOAuthRegistration {
    /// Private endpoint that creates a device interaction.
    pub(crate) device_authorization_endpoint: &'static str,
    /// Private endpoint polled for the authorization code and PKCE material.
    pub(crate) device_poll_endpoint: &'static str,
    /// OAuth endpoint used for authorization-code exchange and refresh grants.
    pub(crate) token_endpoint: &'static str,
    /// Fixed browser destination shown to the administrator.
    pub(crate) verification_uri: &'static str,
    /// Fixed redirect URI bound to the public client registration.
    pub(crate) redirect_uri: &'static str,
    /// Public client identifier registered for the Codex device flow.
    pub(crate) client_id: &'static str,
    /// Maximum duration of one interactive device session.
    pub(crate) device_session_timeout: Duration,
    /// Default polling interval when the authority omits one.
    pub(crate) default_poll_interval: Duration,
    /// Lower bound applied to authority-provided polling intervals.
    pub(crate) minimum_poll_interval: Duration,
    /// Upper bound applied to authority-provided polling intervals and retry hints.
    pub(crate) maximum_poll_interval: Duration,
    /// Timeout applied independently to each HTTPS request.
    pub(crate) request_timeout: Duration,
}

impl ChatGptOAuthRegistration {
    /// Returns the refresh-grant parameters derived from this registration.
    ///
    /// The refresh grant drops `offline_access` from the authorization scope, matching the pinned
    /// reference client.
    pub(crate) fn refresh_parameters(&self) -> OAuth2RefreshParameters {
        OAuth2RefreshParameters {
            token_endpoint: self.token_endpoint,
            client_id: self.client_id,
            scope: Some("openid profile email"),
            request_timeout: self.request_timeout,
        }
    }
}

/// Compile-time ChatGPT subscription OAuth registration.
pub(crate) static REGISTRATION: ChatGptOAuthRegistration = ChatGptOAuthRegistration {
    device_authorization_endpoint: "https://auth.openai.com/api/accounts/deviceauth/usercode",
    device_poll_endpoint: "https://auth.openai.com/api/accounts/deviceauth/token",
    token_endpoint: "https://auth.openai.com/oauth/token",
    verification_uri: "https://auth.openai.com/codex/device",
    redirect_uri: "https://auth.openai.com/deviceauth/callback",
    client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
    device_session_timeout: Duration::from_secs(15 * 60),
    default_poll_interval: Duration::from_secs(5),
    minimum_poll_interval: Duration::from_secs(3),
    maximum_poll_interval: Duration::from_secs(60),
    request_timeout: Duration::from_secs(15),
};
