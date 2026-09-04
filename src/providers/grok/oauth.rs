//! Trusted OAuth registration for Grok subscription access.
//!
//! The authority publishes a standard RFC 8628 device authorization endpoint and refresh grant on
//! the same token endpoint. None of these endpoints or the public client registration can be
//! overridden by runtime configuration.

use std::time::Duration;

use crate::oauth2_credentials::OAuth2RefreshParameters;

/// Fixed OAuth endpoints, public client identity, and timing policy for Grok login and refresh.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GrokOAuthRegistration {
    /// Standard RFC 8628 endpoint that creates one device authorization session.
    pub(crate) device_authorization_endpoint: &'static str,
    /// OAuth endpoint polled with the device_code grant and used for refresh grants.
    pub(crate) token_endpoint: &'static str,
    /// Fixed browser destination shown to the administrator for manual approval.
    pub(crate) verification_uri: &'static str,
    /// Public client identifier registered for the Grok CLI OAuth client.
    pub(crate) client_id: &'static str,
    /// Space-separated scope list bound to the public client registration.
    pub(crate) scope: &'static str,
    /// Maximum duration of one interactive device session.
    pub(crate) device_session_timeout: Duration,
    /// Default polling interval when the authority omits one.
    pub(crate) default_poll_interval: Duration,
    /// Lower bound applied to authority-provided polling intervals.
    pub(crate) minimum_poll_interval: Duration,
    /// Upper bound applied to authority-provided polling intervals.
    pub(crate) maximum_poll_interval: Duration,
    /// Increment applied after an authority `slow_down` rejection.
    pub(crate) slow_down_increment: Duration,
    /// Timeout applied independently to each HTTPS request.
    pub(crate) request_timeout: Duration,
}

impl GrokOAuthRegistration {
    /// Returns the refresh-grant parameters derived from this registration.
    ///
    /// The authority accepts a public-client refresh grant without a scope parameter.
    pub(crate) fn refresh_parameters(&self) -> OAuth2RefreshParameters {
        OAuth2RefreshParameters {
            token_endpoint: self.token_endpoint,
            client_id: self.client_id,
            scope: None,
            request_timeout: self.request_timeout,
        }
    }
}

/// Compile-time Grok subscription OAuth registration.
pub(crate) static REGISTRATION: GrokOAuthRegistration = GrokOAuthRegistration {
    device_authorization_endpoint: "https://auth.x.ai/oauth2/device/code",
    token_endpoint: "https://auth.x.ai/oauth2/token",
    verification_uri: "https://accounts.x.ai/oauth2/device",
    client_id: "b1a00492-073a-47ea-816f-4c329264a828",
    scope: "openid profile email offline_access grok-cli:access api:access",
    device_session_timeout: Duration::from_secs(30 * 60),
    default_poll_interval: Duration::from_secs(5),
    minimum_poll_interval: Duration::from_secs(3),
    maximum_poll_interval: Duration::from_secs(60),
    slow_down_increment: Duration::from_secs(5),
    request_timeout: Duration::from_secs(15),
};
