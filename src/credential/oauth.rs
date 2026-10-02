//! Small OAuth mechanisms, not product token defaults or a workflow engine.
use super::{CredentialError as Error, Secret};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Missing may inherit by the owning grant's contract; explicit null is not missing.
pub(super) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
pub(super) fn challenge(verifier: &str) -> String {
    B64.encode(Sha256::digest(verifier.as_bytes()))
}
pub(super) fn random_token() -> Result<Secret, Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Protocol)?;
    Secret::new(B64.encode(bytes))
}
/// Shared RFC 6749 code exchange. Paths and registration come from the driver;
/// token schema and error interpretation do not belong to this mechanism.
pub(super) struct CodeExchange<'a> {
    pub client: &'a str,
    pub code: &'a str,
    pub redirect: &'a str,
    pub verifier: &'a str,
}
pub(super) async fn exchange_code(
    http: &super::http::AuthHttp,
    path: &str,
    exchange: CodeExchange<'_>,
    metadata: &[(&str, &str)],
    deadline: tokio::time::Instant,
) -> Result<(u16, Vec<u8>), Error> {
    http.request_with_metadata(
        path,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", exchange.client),
            ("code", exchange.code),
            ("redirect_uri", exchange.redirect),
            ("code_verifier", exchange.verifier),
        ],
        None,
        metadata,
        deadline,
    )
    .await
}
/// The caller decides whether the first poll waits and how wire errors change interval.
pub(super) async fn wait_for_poll(
    interval: std::time::Duration,
    deadline: tokio::time::Instant,
) -> Result<(), Error> {
    let wake = tokio::time::Instant::now()
        .checked_add(interval)
        .ok_or(Error::Expired)?;
    if wake >= deadline {
        return Err(Error::Expired);
    }
    tokio::time::sleep_until(wake).await;
    Ok(())
}
/// One deadline covers human interaction, exchange and identity verification.
pub(super) async fn login_deadline<T>(
    deadline: tokio::time::Instant,
    operation: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let result = tokio::time::timeout_at(deadline, operation)
        .await
        .map_err(|_| Error::Expired)?;
    if tokio::time::Instant::now() >= deadline {
        return Err(Error::Expired);
    }
    result
}
pub(super) fn validate_pkce(verifier: &str, expected: &str) -> Result<(), Error> {
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
        || challenge(verifier) != expected
    {
        return Err(Error::Protocol);
    }
    Ok(())
}
