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
                        endpoint: EndpointId::new(&format!("{}-{suffix}", binding.endpoint_prefix))
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
            let candidate = &entry.candidates[0];
            assert_eq!(candidate.endpoint.upstream_model, "openai/gpt-6-luna");
            assert_eq!(candidate.provider.origin.as_str(), "https://openrouter.ai");
            assert_eq!(candidate.endpoint.credential.as_str(), "openrouter-api-key");
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
    fn opencode_go_key_only_activates_native_hy4_chat() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_OPENCODE_GO_API_KEY" => Some("synthetic-go-key".into()),
                _ => None,
            })
        })
        .unwrap();
        let (_, request) = super::super::admission::prepare(
            &boot.gateway.state, Profile::Chat,
            br#"{"model":"hy4-preview","session_id":"synthetic-conversation","messages":[{"role":"user","content":"hi"}]}"#,
        ).unwrap();
        assert_eq!(
            request.task.semantic.controls().max_output_tokens,
            Some(1024)
        );
        let entry = &boot.gateway.state.entries
            [&(super::super::family(Profile::Chat), "hy4-preview".into())];
        assert_eq!(entry.public.route.as_str(), "opencode-go-generation");
        let candidate = &entry.candidates[0];
        assert_eq!(candidate.endpoint.id.as_str(), "opencode-go-chat");
        assert_eq!(candidate.provider.id.as_str(), "opencode-go");
        assert!(candidate.provider.responses.is_none());
        let prepared = crate::execution::prepare(
            &candidate.endpoint,
            &candidate.provider,
            &candidate.secret,
            &request,
        )
        .unwrap();
        assert_eq!(prepared.origin, "https://opencode.ai");
        assert_eq!(prepared.path, "/zen/go/v1/chat/completions");
        assert_eq!(prepared.auth_header.1, "Bearer synthetic-go-key");
        let wire: serde_json::Value = serde_json::from_slice(&prepared.body).unwrap();
        assert_eq!(wire["model"], "hy4-preview");
        assert_eq!(wire["max_tokens"], 1024);
        for (profile, body) in [
            (
                Profile::Responses,
                br#"{"model":"hy4-preview","input":"hi"}"#.as_slice(),
            ),
            (
                Profile::Chat,
                br#"{"model":"opencode-go/hy4-preview","messages":[]}"#,
            ),
            (
                Profile::Chat,
                br#"{"model":"deepseek-flash","messages":[]}"#,
            ),
        ] {
            assert_eq!(
                super::super::admission::prepare(&boot.gateway.state, profile, body)
                    .err()
                    .unwrap()
                    .status,
                404
            );
        }
    }

    #[test]
    fn dashscope_bootstrap_uses_only_the_renamed_binding() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_ALIYUN_DASHSCOPE_CN_API_KEY" => Some("synthetic-dashscope-key".into()),
                "OPENBRIDGE_BAILIAN_API_KEY" => Some("synthetic-obsolete-key".into()),
                _ => None,
            })
        })
        .unwrap();
        for (profile, suffix) in [(Profile::Chat, "chat"), (Profile::Responses, "responses")] {
            let entry =
                &boot.gateway.state.entries[&(super::super::family(profile), "qwen3.8-max".into())];
            let candidate = &entry.candidates[0];
            assert_eq!(
                entry.public.route.as_str(),
                "aliyun-dashscope-cn-generation"
            );
            assert_eq!(candidate.provider.id.as_str(), "aliyun-dashscope-cn");
            assert_eq!(
                candidate.endpoint.id.as_str(),
                format!("aliyun-dashscope-cn-{suffix}")
            );
            assert_eq!(
                candidate.endpoint.credential.as_str(),
                "aliyun-dashscope-cn-api-key"
            );
            assert_eq!(candidate.endpoint.canonical_model.as_str(), "qwen3.8-max");
            let wire = if profile == Profile::Chat {
                serde_json::json!({"model":"qwen3.8-max","messages":[{"role":"user","content":"hi"}]})
            } else {
                serde_json::json!({"model":"qwen3.8-max","input":"hi"})
            };
            let (_, request) = super::super::admission::prepare(
                &boot.gateway.state,
                profile,
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap();
            let prepared = crate::execution::prepare(
                &candidate.endpoint,
                &candidate.provider,
                &candidate.secret,
                &request,
            )
            .unwrap();
            assert_eq!(prepared.origin, "https://dashscope.aliyuncs.com");
            assert_eq!(
                prepared.path,
                format!(
                    "/compatible-mode/v1/{}",
                    if profile == Profile::Chat {
                        "chat/completions"
                    } else {
                        "responses"
                    }
                )
            );
            assert_eq!(
                prepared.auth_header,
                (
                    "authorization".into(),
                    "Bearer synthetic-dashscope-key".into()
                )
            );
            let upstream: serde_json::Value = serde_json::from_slice(&prepared.body).unwrap();
            assert_eq!(upstream["model"], "qwen3.8-max");
        }
        assert!(matches!(
            Bootstrap::from_lookup(|name| {
                Ok(match name {
                    "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                    "OPENBRIDGE_BAILIAN_API_KEY" => Some("synthetic-obsolete-key".into()),
                    _ => None,
                })
            }),
            Err(StartupError::Binding)
        ));
    }

    #[test]
    fn shared_provider_credentials_keep_model_and_protocol_targets_distinct() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_XIAOMI_API_KEY"
                | "OPENBRIDGE_LONGCAT_API_KEY"
                | "OPENBRIDGE_ZHIPU_API_KEY" => Some("synthetic-upstream".into()),
                _ => None,
            })
        })
        .unwrap();
        for (model, upstream, credential, chat_prefix, responses_prefix) in [
            (
                "mimo-v2.6-pro",
                "mimo-v2.6-pro",
                "xiaomi-api-key",
                "/v1",
                "/v1",
            ),
            (
                "mimo-v2.6-flash",
                "mimo-v2.6-flash",
                "xiaomi-api-key",
                "/v1",
                "/v1",
            ),
            (
                "longcat-2.5-preview",
                "LongCat-2.5-Preview",
                "longcat-api-key",
                "/openai/v1",
                "/openai/v1",
            ),
            (
                "glm-5.3",
                "glm-5.3",
                "zhipu-api-key",
                "/api/paas/v4",
                "/api/v1",
            ),
            (
                "glm-5.3-flash",
                "glm-5.3-flash",
                "zhipu-api-key",
                "/api/paas/v4",
                "/api/v1",
            ),
        ] {
            for protocol in [Profile::Chat, Profile::Responses] {
                let entry =
                    &boot.gateway.state.entries[&(super::super::family(protocol), model.into())];
                let candidate = &entry.candidates[0];
                assert_eq!(candidate.endpoint.upstream_model, upstream);
                assert_eq!(candidate.endpoint.credential.as_str(), credential);
                assert_eq!(
                    candidate.endpoint.target.path.as_str(),
                    format!(
                        "{}/{}",
                        if protocol == Profile::Chat {
                            chat_prefix
                        } else {
                            responses_prefix
                        },
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
    fn subscription_and_metered_entries_never_exchange_credentials_or_origins() {
        for (subscription, metered) in [(true, false), (false, true), (true, true)] {
            let boot = Bootstrap::from_lookup(|name| {
                Ok(match name {
                    "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                    "OPENBRIDGE_ALIYUN_TOKENPLAN_CN_API_KEY" if subscription => {
                        Some("synthetic-subscription-key".into())
                    }
                    "OPENBRIDGE_ALIYUN_DASHSCOPE_CN_API_KEY" if metered => {
                        Some("synthetic-metered-key".into())
                    }
                    _ => None,
                })
            })
            .unwrap();
            for profile in [Profile::Chat, Profile::Responses] {
                for (model, enabled, origin, secret) in [
                    (
                        "qwen3.8-flash",
                        subscription,
                        "https://token-plan.cn-beijing.maas.aliyuncs.com",
                        "synthetic-subscription-key",
                    ),
                    (
                        "qwen3.8-max",
                        metered,
                        "https://dashscope.aliyuncs.com",
                        "synthetic-metered-key",
                    ),
                ] {
                    let value = if profile == Profile::Chat {
                        serde_json::json!({"model":model,"messages":[{"role":"user","content":"hi"}],"max_completion_tokens":32})
                    } else {
                        serde_json::json!({"model":model,"input":"hi","max_output_tokens":32})
                    };
                    let admitted = super::super::admission::prepare(
                        &boot.gateway.state,
                        profile,
                        &serde_json::to_vec(&value).unwrap(),
                    );
                    if !enabled {
                        assert!(admitted.is_err());
                        continue;
                    }
                    let (entry, request) = admitted.unwrap();
                    let candidate = &entry.candidates[0];
                    let prepared = crate::execution::prepare(
                        &candidate.endpoint,
                        &candidate.provider,
                        &candidate.secret,
                        &request,
                    )
                    .unwrap();
                    assert_eq!(prepared.origin, origin);
                    assert_eq!(
                        prepared.auth_header,
                        ("authorization".into(), format!("Bearer {secret}"))
                    );
                    assert_eq!(
                        prepared.path,
                        if profile == Profile::Chat {
                            "/compatible-mode/v1/chat/completions"
                        } else {
                            "/compatible-mode/v1/responses"
                        }
                    );
                    let wire: serde_json::Value = serde_json::from_slice(&prepared.body).unwrap();
                    assert_eq!(wire["model"], model);
                    assert_eq!(
                        wire[if profile == Profile::Chat {
                            "max_tokens"
                        } else {
                            "max_output_tokens"
                        }],
                        32
                    );
                }
            }
            if subscription && metered {
                let subscription_entry = &boot.gateway.state.entries
                    [&(super::super::family(Profile::Chat), "qwen3.8-flash".into())];
                let metered_entry = &boot.gateway.state.entries
                    [&(super::super::family(Profile::Chat), "qwen3.8-max".into())];
                assert_ne!(
                    subscription_entry.candidates[0]
                        .endpoint
                        .representation
                        .adaptation
                        .scope,
                    metered_entry.candidates[0]
                        .endpoint
                        .representation
                        .adaptation
                        .scope
                );
            }
        }
    }

    #[test]
    fn bootstrap_only_admits_explicit_protocol_entries() {
        let boot = Bootstrap::from_lookup(|name| {
            Ok(match name {
                "OPENBRIDGE_CLIENT_KEY" => Some("synthetic-gateway-client-token-0001".into()),
                "OPENBRIDGE_NVIDIA_API_KEY" | "OPENBRIDGE_KIMI_API_KEY" => {
                    Some("synthetic-upstream".into())
                }
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
            .is_ok()
        );
        assert!(
            super::super::admission::prepare(
                &boot.gateway.state,
                Profile::Chat,
                br#"{"model":"kimi-k3","messages":[{"role":"user","content":"hi"}]}"#
            )
            .is_ok()
        );
        assert!(
            super::super::admission::prepare(
                &boot.gateway.state,
                Profile::Responses,
                br#"{"model":"kimi-k3","input":"hi"}"#
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
