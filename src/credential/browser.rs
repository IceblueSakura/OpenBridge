//! One owned OIDC browser transaction; product parameters and identity stay in drivers.
use super::{
    BrowserPrompt, CredentialError as Error, Secret,
    callback::Callback,
    http::AuthHttp,
    oauth::{self, CodeExchange},
};
use std::{collections::BTreeSet, time::Duration};
use tokio::time::Instant;

pub(super) struct BrowserProfile<'a> {
    pub authorize: &'static str,
    pub client: &'a str,
    pub scopes: &'static str,
    pub callback_path: &'static str,
    pub extra: &'a [(&'a str, &'a str)],
    pub timeout: Duration,
}
pub(super) struct BrowserGrant {
    pub prompt: BrowserPrompt,
    pub deadline: Instant,
    callback: Callback,
    verifier: Secret,
    state: Secret,
    nonce: Secret,
    client: String,
}
pub(super) struct BrowserResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub nonce: Secret,
    pub client: String,
}
impl BrowserGrant {
    pub async fn begin(profile: BrowserProfile<'_>, port: u16) -> Result<Self, Error> {
        let started = Instant::now();
        let mut url = url::Url::parse(profile.authorize).map_err(|_| Error::Protocol)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || profile.timeout.is_zero()
            || profile.timeout > Duration::from_secs(900)
        {
            return Err(Error::Protocol);
        }
        let mut names: BTreeSet<&str> = [
            "response_type",
            "client_id",
            "scope",
            "redirect_uri",
            "code_challenge_method",
            "code_challenge",
            "state",
            "nonce",
        ]
        .into_iter()
        .collect();
        for (name, _) in profile.extra {
            if !names.insert(name) || names.len() > 16 {
                return Err(Error::Protocol);
            }
        }
        let callback = Callback::bind(port, profile.callback_path).await?;
        let redirect_uri = callback.redirect_uri();
        let verifier = oauth::random_token()?;
        let state = oauth::random_token()?;
        let nonce = oauth::random_token()?;
        url.query_pairs_mut()
            .extend_pairs([
                ("response_type", "code"),
                ("client_id", profile.client),
                ("scope", profile.scopes),
                ("redirect_uri", redirect_uri.as_str()),
                ("code_challenge_method", "S256"),
                (
                    "code_challenge",
                    oauth::challenge(verifier.expose()).as_str(),
                ),
                ("state", state.expose()),
                ("nonce", nonce.expose()),
            ])
            .extend_pairs(profile.extra.iter().copied());
        Ok(Self {
            prompt: BrowserPrompt {
                authorization_url: url.to_string(),
                redirect_uri,
            },
            deadline: started + profile.timeout,
            callback,
            verifier,
            state,
            nonce,
            client: profile.client.into(),
        })
    }
    pub async fn exchange(
        mut self,
        http: &AuthHttp,
        token_path: &str,
    ) -> Result<BrowserResponse, Error> {
        let code = self
            .callback
            .wait(self.state.expose(), self.deadline)
            .await?;
        // The listener and accepted connections are gone before authority I/O.
        // Receiving a code does not acknowledge identity or local publication.
        drop(self.callback);
        let (status, body) = oauth::exchange_code(
            http,
            token_path,
            CodeExchange {
                client: &self.client,
                code: code.expose(),
                redirect: &self.prompt.redirect_uri,
                verifier: self.verifier.expose(),
            },
            self.deadline,
        )
        .await?;
        Ok(BrowserResponse {
            status,
            body,
            nonce: self.nonce,
            client: self.client,
        })
    }
}
