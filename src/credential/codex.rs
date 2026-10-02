//! Codex product browser/device/refresh wire, pinned to CLI 0.160.0 (Apache-2.0 source).
//! https://github.com/openai/codex/tree/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login
//! This is not SIWC, an API-key derivation path or evidence of subscription admission.
use super::{
    CredentialError as Error,
    browser::{BrowserGrant, BrowserProfile},
    http::{AuthHttp, REQUEST_TIMEOUT},
    model::DevicePrompt,
    model::{Credential, Secret, VerifiedIdentity, now},
    oauth::{self, present},
};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::Instant;

pub(super) const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ORIGIN: &str = "https://auth.openai.com";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(900);
// Minimal identity/renewal grant; connector permissions are not login prerequisites.
const BROWSER_SCOPES: &str = "openid profile email offline_access";
/// Only refresh/revoke use the pinned CLI default metadata. Device auth/code
/// exchange use a raw auth client in the source. Keep our attribution explicit.
pub(super) fn user_agent() -> String {
    format!(
        "codex_cli_rs/0.160.0 ({}; {}) (OpenBridge/{})",
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION")
    )
}
#[derive(Clone)]
pub(super) struct CodexAuthority {
    http: AuthHttp,
    browser_port: u16,
}
#[derive(Debug)]
pub(super) struct DeviceGrant {
    pub prompt: DevicePrompt,
    device_auth_id: Secret,
    interval: Duration,
    deadline: Instant,
}
#[derive(Deserialize)]
struct DeviceWire {
    device_auth_id: String,
    #[serde(alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "interval")]
    interval: u64,
}
fn interval<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    String::deserialize(deserializer)?
        .trim()
        .parse()
        .map_err(serde::de::Error::custom)
}
#[derive(Deserialize)]
struct CodeWire {
    authorization_code: String,
    code_verifier: String,
    code_challenge: String,
}
#[derive(Deserialize)]
struct TokenWire {
    #[serde(default, deserialize_with = "present")]
    access_token: Option<String>,
    #[serde(default, deserialize_with = "present")]
    refresh_token: Option<String>,
    #[serde(default, deserialize_with = "present")]
    id_token: Option<String>,
    #[serde(default, deserialize_with = "present")]
    expires_in: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    token_type: Option<String>,
    #[serde(default, deserialize_with = "present")]
    scope: Option<String>,
}
impl CodexAuthority {
    pub fn new(proxy: Option<&str>) -> Result<Self, Error> {
        Ok(Self {
            http: AuthHttp::new(ORIGIN, proxy)?,
            browser_port: 1455,
        })
    }
    #[cfg(test)]
    pub fn synthetic(origin: &str) -> Result<Self, Error> {
        Ok(Self {
            http: AuthHttp::synthetic(origin)?,
            // Isolated callback sockets for the synthetic authority only.
            browser_port: 0,
        })
    }
    // Fixed CLI source: https://github.com/openai/codex/blob/a956835d020762cb2b570053af06f643a11c0ecc/codex-rs/login/src/server.rs
    // Admit only registered ports; never cancel another listener or switch ports.
    fn callback_port(&self, requested: Option<u16>) -> Result<u16, Error> {
        match requested {
            None => Ok(self.browser_port),
            Some(port @ (1455 | 1457)) => Ok(port),
            Some(_) => Err(Error::InvalidInput),
        }
    }
    pub async fn browser(&self, requested: Option<u16>) -> Result<BrowserGrant, Error> {
        BrowserGrant::begin(
            BrowserProfile {
                authorize: "https://auth.openai.com/oauth/authorize",
                client: CLIENT_ID,
                scopes: BROWSER_SCOPES,
                callback_path: "/auth/callback",
                extra: &[
                    ("id_token_add_organizations", "true"),
                    ("codex_cli_simplified_flow", "true"),
                    ("originator", "openbridge"),
                ],
                timeout: LOGIN_TIMEOUT,
            },
            self.callback_port(requested)?,
        )
        .await
    }
    pub async fn complete_browser(
        &self,
        grant: BrowserGrant,
    ) -> Result<(Credential, VerifiedIdentity), Error> {
        let deadline = grant.deadline;
        oauth::login_deadline(deadline, async {
            let response = grant.exchange(&self.http, "/oauth/token").await?;
            self.accept_login(
                response.status,
                &response.body,
                Some(response.nonce.expose()),
                deadline,
            )
            .await
        })
        .await
    }
    pub async fn device(&self) -> Result<DeviceGrant, Error> {
        let started = Instant::now();
        let (status, body) = self
            .http
            .json(
                "/api/accounts/deviceauth/usercode",
                &serde_json::json!({"client_id":CLIENT_ID}),
                &[],
                started + REQUEST_TIMEOUT,
            )
            .await?;
        if status == 404 {
            return Err(Error::DeviceUnavailable);
        }
        if status != 200 {
            return Err(failure(status, &body));
        }
        let wire: DeviceWire = serde_json::from_slice(&body).map_err(|_| Error::Protocol)?;
        if wire.interval > 300
            || wire.user_code.is_empty()
            || wire.user_code.len() > 64
            || !wire
                .user_code
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(Error::Protocol);
        }
        Ok(DeviceGrant {
            prompt: DevicePrompt {
                verification_uri: format!("{ORIGIN}/codex/device"),
                user_code: wire.user_code,
            },
            device_auth_id: Secret::new(wire.device_auth_id)?,
            interval: Duration::from_secs(wire.interval.max(1)),
            deadline: started + LOGIN_TIMEOUT,
        })
    }
    pub async fn complete(
        &self,
        grant: DeviceGrant,
    ) -> Result<(Credential, VerifiedIdentity), Error> {
        oauth::login_deadline(grant.deadline, self.complete_device(grant)).await
    }
    async fn complete_device(
        &self,
        grant: DeviceGrant,
    ) -> Result<(Credential, VerifiedIdentity), Error> {
        let code = loop {
            if Instant::now() >= grant.deadline {
                return Err(Error::Expired);
            }
            let (status, body) = self.http.json("/api/accounts/deviceauth/token", &serde_json::json!({
                "device_auth_id":grant.device_auth_id.expose(), "user_code":grant.prompt.user_code }),
                &[], grant.deadline).await?;
            if status == 200 {
                break serde_json::from_slice::<CodeWire>(&body).map_err(|_| Error::Protocol)?;
            }
            // These statuses mean pending only at the Codex private polling endpoint.
            if status != 403 && status != 404 {
                return Err(failure(status, &body));
            }
            super::oauth::wait_for_poll(grant.interval, grant.deadline).await?;
        };
        if code.authorization_code.is_empty()
            || code.authorization_code.len() > 4096
            || super::oauth::validate_pkce(&code.code_verifier, &code.code_challenge).is_err()
        {
            return Err(Error::Protocol);
        }
        let (status, body) = super::oauth::exchange_code(
            &self.http,
            "/oauth/token",
            super::oauth::CodeExchange {
                client: CLIENT_ID,
                code: &code.authorization_code,
                redirect: "https://auth.openai.com/deviceauth/callback",
                verifier: &code.code_verifier,
            },
            grant.deadline,
        )
        .await?;
        self.accept_login(status, &body, None, grant.deadline).await
    }
    async fn accept_login(
        &self,
        status: u16,
        body: &[u8],
        nonce: Option<&str>,
        deadline: Instant,
    ) -> Result<(Credential, VerifiedIdentity), Error> {
        if status != 200 {
            return Err(failure(status, body));
        }
        let wire: TokenWire = serde_json::from_slice(body).map_err(|_| Error::Protocol)?;
        let id = wire.id_token.as_ref().ok_or(Error::Protocol)?;
        let identity = self.identity(id, CLIENT_ID, nonce, deadline).await?;
        Ok((credential(wire, None)?, identity))
    }
    async fn identity(
        &self,
        token: &str,
        client: &str,
        nonce: Option<&str>,
        deadline: Instant,
    ) -> Result<VerifiedIdentity, Error> {
        let (status, body) = self.http.get("/.well-known/jwks.json", deadline).await?;
        if status != 200 {
            return Err(failure(status, &body));
        }
        let keys = serde_json::from_slice(&body).map_err(|_| Error::Protocol)?;
        verify_identity(token, &keys, client, nonce)
    }
    pub async fn refresh(
        &self,
        client: &str,
        previous: &Credential,
    ) -> Result<(Credential, Option<VerifiedIdentity>), Error> {
        if client != CLIENT_ID {
            return Err(Error::IdentityMismatch);
        }
        let (status, body) = self.http.json("/oauth/token", &serde_json::json!({
            "grant_type":"refresh_token", "client_id":client, "refresh_token":previous.refresh.as_ref().ok_or(Error::LoginRequired)?.expose()
        }), &[("originator", "codex_cli_rs"), ("user-agent", &user_agent())], Instant::now() + REQUEST_TIMEOUT).await?;
        if status != 200 {
            return Err(failure(status, &body));
        }
        let wire: TokenWire = serde_json::from_slice(&body).map_err(|_| Error::Protocol)?;
        let identity = match &wire.id_token {
            Some(token) => Some(
                self.identity(token, client, None, Instant::now() + REQUEST_TIMEOUT)
                    .await?,
            ),
            None => None,
        };
        Ok((credential(wire, Some(previous))?, identity))
    }
    pub async fn revoke(&self, client: &str, refresh: &str) -> Result<(), Error> {
        if client != CLIENT_ID {
            return Err(Error::IdentityMismatch);
        }
        let (status, body) = self
            .http
            .json(
                "/oauth/revoke",
                &serde_json::json!({
                    "client_id":client, "token":refresh, "token_type_hint":"refresh_token"
                }),
                &[
                    ("originator", "codex_cli_rs"),
                    ("user-agent", &user_agent()),
                ],
                Instant::now() + REQUEST_TIMEOUT,
            )
            .await?;
        if status == 200 {
            Ok(())
        } else {
            Err(failure(status, &body))
        }
    }
}
pub(super) fn verify_identity(
    token: &str,
    keys: &serde_json::Value,
    client: &str,
    nonce: Option<&str>,
) -> Result<VerifiedIdentity, Error> {
    let verified = super::jwt::verify(
        token,
        keys,
        super::jwt::Policy {
            issuer: ORIGIN,
            client,
            algorithm: super::jwt::Algorithm::Rs256,
            nonce,
        },
    )?;
    let claims: CodexClaims =
        serde_json::from_slice(&verified.payload).map_err(|_| Error::Protocol)?;
    let identity = VerifiedIdentity {
        subject: verified.subject,
        scope: Some(claims.auth.chatgpt_account_id),
    };
    if !identity.valid() || claims.auth.chatgpt_account_is_fedramp == Some(true) {
        return Err(Error::Protocol);
    }
    Ok(identity)
}
fn credential(wire: TokenWire, previous: Option<&Credential>) -> Result<Credential, Error> {
    if wire
        .token_type
        .as_deref()
        .is_some_and(|kind| !kind.eq_ignore_ascii_case("bearer"))
    {
        return Err(Error::Protocol);
    }
    let access_changed = wire.access_token.is_some();
    let access = Secret::new(
        wire.access_token
            .or_else(|| previous.map(|value| value.access.expose().into()))
            .ok_or(Error::Protocol)?,
    )?;
    let refresh = Secret::new(
        wire.refresh_token
            .or_else(|| {
                previous
                    .and_then(|value| value.refresh.as_ref())
                    .map(|v| v.expose().into())
            })
            .ok_or(Error::Protocol)?,
    )?;
    let id_token = Secret::new(
        wire.id_token
            .or_else(|| {
                previous
                    .and_then(|value| value.id_token.as_ref())
                    .map(|value| value.expose().into())
            })
            .ok_or(Error::Protocol)?,
    )?;
    let expires_at = match wire.expires_in {
        Some(0) => return Err(Error::Protocol),
        Some(value) => Some(now()?.checked_add(value).ok_or(Error::Protocol)?),
        None if !access_changed => previous.and_then(|value| value.expires_at),
        None => super::jwt::access_expiry(access.expose())?,
    };
    let scopes = wire
        .scope
        .or_else(|| previous.and_then(|value| value.scopes.clone()));
    if scopes
        .as_ref()
        .is_some_and(|s| s.len() > 1024 || !s.bytes().all(|c| (0x20..=0x7e).contains(&c)))
    {
        return Err(Error::Protocol);
    }
    Ok(Credential {
        access,
        refresh: Some(refresh),
        id_token: Some(id_token),
        expires_at,
        scopes,
    })
}
#[derive(Deserialize)]
struct CodexClaims {
    #[serde(rename = "https://api.openai.com/auth")]
    auth: Workspace,
}
#[derive(Deserialize)]
struct Workspace {
    chatgpt_account_id: String,
    chatgpt_account_is_fedramp: Option<bool>,
}
impl super::AuthDriver for CodexAuthority {
    fn profile(&self) -> &'static str {
        "codex"
    }
    fn login_client(&self, options: &super::LoginOptions) -> Result<String, Error> {
        if options.client_id.is_some() {
            return Err(Error::InvalidInput);
        }
        match options.method {
            super::LoginMethod::Device if options.callback_port.is_some() => {
                return Err(Error::InvalidInput);
            }
            super::LoginMethod::Browser => {
                self.callback_port(options.callback_port)?;
            }
            super::LoginMethod::Device => {}
        }
        Ok(CLIENT_ID.into())
    }
    fn validate(
        &self,
        client: &str,
        identity: Option<&VerifiedIdentity>,
        credential: Option<&Credential>,
    ) -> Result<(), Error> {
        if client != CLIENT_ID
            || identity.is_some_and(|i| i.scope.is_none())
            || credential.is_some_and(|c| {
                c.id_token.is_none()
                    || c.refresh.is_none()
                    || c.scopes.as_ref().is_some_and(|s| s.len() > 1024)
            })
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
    fn login<'a>(
        &'a self,
        _client: &'a str,
        options: &'a super::LoginOptions,
        notify: super::LoginObserver<'a>,
    ) -> super::DriverFuture<'a, super::Grant> {
        Box::pin(async move {
            let (credential, identity) = match options.method {
                super::LoginMethod::Device => {
                    let grant = self.device().await?;
                    notify(&super::LoginPrompt::Device(grant.prompt.clone()));
                    self.complete(grant).await?
                }
                super::LoginMethod::Browser => {
                    let grant = self.browser(options.callback_port).await?;
                    notify(&super::LoginPrompt::Browser(grant.prompt.clone()));
                    self.complete_browser(grant).await?
                }
            };
            Ok(super::Grant {
                identity,
                credential,
            })
        })
    }
    fn can_refresh(&self) -> bool {
        true
    }
    fn refresh<'a>(
        &'a self,
        client: &'a str,
        identity: &'a VerifiedIdentity,
        previous: &'a Credential,
    ) -> super::DriverFuture<'a, super::Grant> {
        Box::pin(async move {
            let (credential, reported) = self.refresh(client, previous).await?;
            Ok(super::Grant {
                credential,
                identity: reported.unwrap_or_else(|| identity.clone()),
            })
        })
    }
    fn revoke<'a>(
        &'a self,
        client: &'a str,
        credential: &'a Credential,
    ) -> super::DriverFuture<'a, ()> {
        Box::pin(async move {
            self.revoke(
                client,
                credential
                    .refresh
                    .as_ref()
                    .ok_or(Error::LoginRequired)?
                    .expose(),
            )
            .await
        })
    }
}
fn failure(status: u16, body: &[u8]) -> Error {
    let value: Option<serde_json::Value> = serde_json::from_slice(body).ok();
    let code = value
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|error| {
            error
                .as_str()
                .or_else(|| error.get("code").and_then(serde_json::Value::as_str))
        });
    match code {
        Some("invalid_grant") => Error::InvalidGrant,
        Some("refresh_token_expired") => Error::GrantExpired,
        Some("refresh_token_reused") => Error::GrantReused,
        Some("refresh_token_invalidated") => Error::GrantRevoked,
        Some("access_denied") | Some("authorization_denied") => Error::Denied,
        _ if status == 401 || status == 403 => Error::Denied,
        _ if status == 400 || status == 404 => Error::Protocol,
        _ => Error::Network,
    }
}
