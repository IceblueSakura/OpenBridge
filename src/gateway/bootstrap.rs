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
        for (variable, binding, models) in [
            (
                "OPENBRIDGE_DEEPSEEK_API_KEY",
                "deepseek-api-key",
                &[("deepseek-flash", "deepseek")][..],
            ),
            (
                "OPENBRIDGE_XIAOMI_API_KEY",
                "xiaomi-api-key",
                &[
                    ("mimo-v2.6-pro", "xiaomi"),
                    ("mimo-v2.6-flash", "xiaomi-flash"),
                ][..],
            ),
            (
                "OPENBRIDGE_OPENROUTER_API_KEY",
                "openrouter-api-key",
                &[("gpt-6-luna", "openrouter")][..],
            ),
        ] {
            if let Some(key) = get(variable)? {
                credentials.insert(
                    CredentialBindingId::new(binding).map_err(|_| StartupError::Binding)?,
                    Arc::new(SecretMaterial::new(&key).map_err(|_| StartupError::Credentials)?),
                );
                for &(model, prefix) in models {
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
        }
        for binding in catalog::API_KEY_BINDINGS {
            if let Some(key) = get(binding.variable)? {
                credentials.insert(
                    CredentialBindingId::new(binding.credential)
                        .map_err(|_| StartupError::Binding)?,
                    Arc::new(SecretMaterial::new(&key).map_err(|_| StartupError::Credentials)?),
                );
                for protocol in binding.protocols {
                    let (protocol, suffix) = match protocol {
                        crate::topology::ProtocolProfile::OpenAiChat => (Profile::Chat, "chat"),
                        crate::topology::ProtocolProfile::OpenAiResponses => {
                            (Profile::Responses, "responses")
                        }
                    };
                    entries.push(Entry {
                        model: binding.model.into(),
                        protocol,
                        endpoint: EndpointId::new(&format!("{}-{suffix}", binding.provider))
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
        let gateway = if let Some(path) = get("OPENBRIDGE_PROBE_DIAGNOSTICS")? {
            gateway.with_probe_diagnostics(std::path::Path::new(&path))?
        } else {
            gateway
        };
        Ok(Self { gateway, listen })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openrouter_key_enables_only_its_fixed_model_and_rejects_empty_credentials() {
        let settings = |name: &str| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_OPENROUTER_API_KEY" => Some("synthetic-openrouter-only".into()),
                _ => None,
            })
        };
        let boot = Bootstrap::from_lookup(settings).unwrap();
        for profile in [Profile::Chat, Profile::Responses] {
            let entry =
                &boot.gateway.state.entries[&(super::super::family(profile), "gpt-6-luna".into())];
            assert_eq!(entry.endpoint.upstream_model, "openai/gpt-6-luna");
            assert_eq!(entry.provider.origin.as_str(), "https://openrouter.ai");
            assert_eq!(entry.endpoint.credential.as_str(), "openrouter-api-key");
        }
        assert!(
            boot.gateway
                .state
                .entries
                .keys()
                .all(|(_, model)| model == "gpt-6-luna")
        );
        assert!(matches!(
            Bootstrap::from_lookup(|name| {
                if name == "OPENBRIDGE_OPENROUTER_API_KEY" {
                    Ok(Some(String::new()))
                } else {
                    settings(name)
                }
            }),
            Err(StartupError::Credentials)
        ));
    }

    #[test]
    fn shared_provider_credentials_keep_model_and_protocol_targets_distinct() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_XIAOMI_API_KEY" | "OPENBRIDGE_LONGCAT_API_KEY" => {
                    Some("synthetic-upstream".into())
                }
                _ => None,
            })
        })
        .unwrap();
        for (model, upstream, credential, prefix) in [
            ("mimo-v2.6-pro", "mimo-v2.6-pro", "xiaomi-api-key", "/v1"),
            (
                "mimo-v2.6-flash",
                "mimo-v2.6-flash",
                "xiaomi-api-key",
                "/v1",
            ),
            (
                "longcat-2.5-preview",
                "LongCat-2.5-Preview",
                "longcat-api-key",
                "/openai/v1",
            ),
        ] {
            for protocol in [Profile::Chat, Profile::Responses] {
                let entry =
                    &boot.gateway.state.entries[&(super::super::family(protocol), model.into())];
                assert_eq!(entry.endpoint.upstream_model, upstream);
                assert_eq!(entry.endpoint.credential.as_str(), credential);
                assert_eq!(
                    entry.endpoint.target.path.as_str(),
                    format!(
                        "{prefix}/{}",
                        if protocol == Profile::Chat {
                            "chat/completions"
                        } else {
                            "responses"
                        }
                    )
                );
                let request = if protocol == Profile::Chat {
                    serde_json::json!({"model":model,"messages":[{"role":"user","content":"hi"}]})
                } else {
                    serde_json::json!({"model":model,"input":"hi"})
                };
                assert!(
                    super::super::admission::prepare(
                        &boot.gateway.state,
                        protocol,
                        &serde_json::to_vec(&request).unwrap()
                    )
                    .is_ok()
                );
            }
        }
    }

    #[test]
    fn chat_only_bootstrap_admits_chat_but_not_an_invented_responses_entry() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_NVIDIA_API_KEY" => Some("synthetic-upstream".into()),
                _ => None,
            })
        })
        .unwrap();
        assert!(
            super::super::admission::prepare(
                &boot.gateway.state,
                Profile::Chat,
                br#"{"model":"nemotron-3-super","messages":[{"role":"user","content":"hi"}]}"#
            )
            .is_ok()
        );
        assert!(
            super::super::admission::prepare(
                &boot.gateway.state,
                Profile::Responses,
                br#"{"model":"nemotron-3-super","input":"hi"}"#
            )
            .is_err()
        );
    }

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
