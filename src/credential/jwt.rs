//! Fixed-profile OIDC validation; JWT headers never choose algorithms or key URLs.
use super::{CredentialError as Error, model::now};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde::Deserialize;
use subtle::ConstantTimeEq;

#[derive(Clone, Copy)]
pub(super) enum Algorithm {
    Rs256,
    Es256,
}
impl Algorithm {
    fn name(self) -> &'static str {
        match self {
            Self::Rs256 => "RS256",
            Self::Es256 => "ES256",
        }
    }
}
pub(super) struct Policy<'a> {
    pub issuer: &'a str,
    pub client: &'a str,
    pub algorithm: Algorithm,
    pub nonce: Option<&'a str>,
}
pub(super) struct VerifiedClaims {
    pub subject: String,
    pub payload: Vec<u8>,
}
#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
    crit: Option<Vec<String>>,
    b64: Option<bool>,
}
#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct Key {
    kid: Option<String>,
    kty: String,
    n: Option<String>,
    e: Option<String>,
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
    alg: Option<String>,
    #[serde(rename = "use")]
    usage: Option<String>,
    key_ops: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}
#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: Audience,
    sub: String,
    exp: u64,
    azp: Option<String>,
    nbf: Option<u64>,
    iat: Option<u64>,
    nonce: Option<String>,
}
fn parts(jwt: &str) -> Result<(&str, &str, &str), Error> {
    if jwt.len() > 16384 {
        return Err(Error::Protocol);
    }
    let mut parts = jwt.split('.');
    let result = (
        parts.next().ok_or(Error::Protocol)?,
        parts.next().ok_or(Error::Protocol)?,
        parts.next().ok_or(Error::Protocol)?,
    );
    if parts.next().is_some() || result.0.is_empty() || result.1.is_empty() || result.2.is_empty() {
        return Err(Error::Protocol);
    }
    Ok(result)
}
pub(super) fn verify(
    jwt: &str,
    keys: &serde_json::Value,
    policy: Policy<'_>,
) -> Result<VerifiedClaims, Error> {
    let client = policy.client;
    let (header, payload, signature) = parts(jwt)?;
    let header: Header = serde_json::from_slice(&decode(header)?).map_err(|_| Error::Protocol)?;
    if header.alg != policy.algorithm.name()
        || header.kid.is_empty()
        || header.kid.len() > 256
        || header.crit.is_some_and(|values| !values.is_empty())
        || header.b64 == Some(false)
    {
        return Err(Error::Protocol);
    }
    let keys: Jwks = serde_json::from_value(keys.clone()).map_err(|_| Error::Protocol)?;
    if keys.keys.len() > 16 {
        return Err(Error::Protocol);
    }
    let mut matching = keys
        .keys
        .iter()
        .filter(|key| key.kid.as_deref() == Some(header.kid.as_str()));
    let key = matching.next().ok_or(Error::Protocol)?;
    if matching.next().is_some()
        || key
            .alg
            .as_deref()
            .is_some_and(|a| a != policy.algorithm.name())
        || key.usage.as_deref().is_some_and(|value| value != "sig")
        || key
            .key_ops
            .as_ref()
            .is_some_and(|ops| !ops.iter().any(|op| op == "verify"))
    {
        return Err(Error::Protocol);
    }
    let signature = decode(signature)?;
    let split = jwt.rfind('.').ok_or(Error::Protocol)?;
    let message = &jwt.as_bytes()[..split];
    match policy.algorithm {
        Algorithm::Rs256 => {
            if key.kty != "RSA" {
                return Err(Error::Protocol);
            }
            let n = decode(key.n.as_ref().ok_or(Error::Protocol)?)?;
            let e = decode(key.e.as_ref().ok_or(Error::Protocol)?)?;
            if !(256..=1024).contains(&n.len()) || e.is_empty() || e.len() > 8 {
                return Err(Error::Protocol);
            }
            ring::signature::RsaPublicKeyComponents { n: &n, e: &e }
                .verify(
                    &ring::signature::RSA_PKCS1_2048_8192_SHA256,
                    message,
                    &signature,
                )
                .map_err(|_| Error::Protocol)?;
        }
        Algorithm::Es256 => {
            if key.kty != "EC" || key.crv.as_deref() != Some("P-256") || signature.len() != 64 {
                return Err(Error::Protocol);
            }
            let x = decode(key.x.as_ref().ok_or(Error::Protocol)?)?;
            let y = decode(key.y.as_ref().ok_or(Error::Protocol)?)?;
            if x.len() != 32 || y.len() != 32 {
                return Err(Error::Protocol);
            }
            let mut point = [0u8; 65];
            point[0] = 4;
            point[1..33].copy_from_slice(&x);
            point[33..].copy_from_slice(&y);
            ring::signature::UnparsedPublicKey::new(
                &ring::signature::ECDSA_P256_SHA256_FIXED,
                point,
            )
            .verify(message, &signature)
            .map_err(|_| Error::Protocol)?;
        }
    }
    let payload = decode(payload)?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| Error::Protocol)?;
    let timestamp = now()?;
    let valid_audience = match &claims.aud {
        Audience::One(value) => value == client,
        Audience::Many(values) => {
            values.len() <= 16
                && values.iter().any(|value| value == client)
                && (values.len() == 1 || claims.azp.as_deref() == Some(client))
        }
    };
    if claims.iss != policy.issuer
        || !valid_audience
        || claims.exp <= timestamp
        || claims.azp.as_deref().is_some_and(|value| value != client)
        || claims
            .nbf
            .is_some_and(|value| value > timestamp.saturating_add(30))
        || claims
            .iat
            .is_some_and(|value| value > timestamp.saturating_add(30))
        || !identity_field(&claims.sub)
    {
        return Err(Error::Protocol);
    }
    if let Some(nonce) = policy.nonce {
        let actual = claims.nonce.as_deref().ok_or(Error::Protocol)?;
        if nonce.is_empty() || !bool::from(actual.as_bytes().ct_eq(nonce.as_bytes())) {
            return Err(Error::Protocol);
        }
    }
    Ok(VerifiedClaims {
        subject: claims.sub,
        payload,
    })
}
fn decode(value: &str) -> Result<Vec<u8>, Error> {
    B64.decode(value).map_err(|_| Error::Protocol)
}
fn identity_field(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
/// Access expiry is scheduling metadata from the authenticated token response,
/// not identity proof. Opaque/missing exp remains unknown, never an ID-token TTL.
pub(super) fn access_expiry(access: &str) -> Result<Option<u64>, Error> {
    if !access.contains('.') {
        return Ok(None);
    }
    let Ok((_, payload, _)) = parts(access) else {
        return Ok(None);
    };
    let Ok(bytes) = B64.decode(payload) else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    match value.get("exp") {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or(Error::Protocol),
    }
}
