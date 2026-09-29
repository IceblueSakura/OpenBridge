//! Environment-only bootstrap. No archived configuration files or ambient proxy
//! settings are read; only explicitly enabled fixed catalog entries are exposed.
use super::{Credentials, Entry, Gateway, Limits, StartupError};
use crate::{
    protocol::openai::Profile,
    provider::{CredentialBindingId, SecretMaterial},
    topology::{EndpointId, catalog},
};
use std::{net::SocketAddr, sync::Arc};
pub struct Bootstrap {
    pub gateway: Gateway,
    pub listen: SocketAddr,
}
impl Bootstrap {
    pub fn from_env() -> Result<Self, StartupError> {
        Self::from_lookup(|name| match std::env::var(name) {
            Ok(v) => Ok(Some(v)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(StartupError::Environment),
        })
    }
    fn from_lookup(
        mut get: impl FnMut(&str) -> Result<Option<String>, StartupError>,
    ) -> Result<Self, StartupError> {
        let listen: SocketAddr = get("OPENBRIDGE_BIND")?
            .as_deref()
            .unwrap_or("127.0.0.1:8080")
            .parse()
            .map_err(|_| StartupError::Listener)?;
        if !listen.ip().is_loopback() {
            return Err(StartupError::Listener);
        }
        let client_key =
            SecretMaterial::new(&get("OPENBRIDGE_CLIENT_KEY")?.ok_or(StartupError::Credentials)?)
                .map_err(|_| StartupError::Credentials)?;
        let mut credentials = Credentials::new();
        let mut entries = vec![];
        for (variable, binding, model, prefix) in [
            (
                "OPENBRIDGE_DEEPSEEK_API_KEY",
                "deepseek-api-key",
                "deepseek-flash",
                "deepseek",
            ),
            (
                "OPENBRIDGE_XIAOMI_API_KEY",
                "xiaomi-api-key",
                "mimo-v2.6-pro",
                "xiaomi",
            ),
        ] {
            if let Some(key) = get(variable)? {
                credentials.insert(
                    CredentialBindingId::new(binding).map_err(|_| StartupError::Binding)?,
                    Arc::new(SecretMaterial::new(&key).map_err(|_| StartupError::Credentials)?),
                );
                for (protocol, suffix) in
                    [(Profile::Chat, "chat"), (Profile::Responses, "responses")]
                {
                    entries.push(Entry {
                        model: model.into(),
                        protocol,
                        endpoint: EndpointId::new(&format!("{prefix}-{suffix}"))
                            .map_err(|_| StartupError::Binding)?,
                    });
                }
            }
        }
        let proxy = get("OPENBRIDGE_PROXY")?;
        let gateway = Gateway::new(
            catalog::default_topology().map_err(|_| StartupError::Binding)?,
            entries,
            credentials,
            client_key,
            Limits::default(),
            proxy.as_deref(),
        )?;
        Ok(Self { gateway, listen })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_is_loopback_and_requires_explicit_auth_and_provider_keys() {
        let settings = |name: &str| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_DEEPSEEK_API_KEY" => Some("synthetic-upstream-only".into()),
                _ => None,
            })
        };
        assert!(Bootstrap::from_lookup(settings).is_ok());
        assert!(matches!(
            Bootstrap::from_lookup(|_| Ok(None)),
            Err(StartupError::Credentials)
        ));
        assert!(matches!(
            Bootstrap::from_lookup(|n| if n == "OPENBRIDGE_BIND" {
                Ok(Some("0.0.0.0:8080".into()))
            } else {
                settings(n)
            }),
            Err(StartupError::Listener)
        ));
        assert!(
            Bootstrap::from_lookup(|n| if n == "OPENBRIDGE_DEEPSEEK_API_KEY" {
                Ok(None)
            } else {
                settings(n)
            })
            .is_err()
        );
    }
}
