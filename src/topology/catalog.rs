//! Trusted Generation bindings for the fixed API-key Provider catalog.
//! Each entry fixes its upstream model, wire adapter and credential ownership.
//!
//! Facts come from the scoped sources in `docs/references/providers/`. Model
//! capability is deliberately not widened here: the representation contract
//! claims wire-level representability plus documented protocol limits
//! (function strict is only guaranteed on DeepSeek `/beta`; custom tools and
//! media inputs are undeclared except for the selected image slice; cache-affinity hints are undeclared on these
//! entries and are omitted by approved inactive-hint omission). Live behavior
//! must be checked for the current target; it is not a catalog guarantee.

use crate::{
    lowering::generation::GenerationRepresentationContract,
    provider::{CredentialBindingId, CredentialKind, catalog, ident_ok},
    topology::{
        Endpoint, EndpointId, EndpointTarget, ExecutionContract, ModelId, ProtocolProfile,
        PublicModel, Route, RouteId, TaskKind, TopologyError, compile, compile::CompiledTopology,
    },
};

fn replay_scope(provider: &str) -> crate::semantic::value::ReplayOrigin {
    crate::semantic::value::ReplayOrigin::new(provider).expect("static scope")
}

fn wire_contract(
    replay_origin: Option<crate::semantic::value::ReplayOrigin>,
) -> GenerationRepresentationContract {
    GenerationRepresentationContract {
        replay_origin,
        custom_tools: false,
        strict_tools: false,
        image_input: false,
        audio_input: false,
        file_input: false,
        cache_hints: false,
        standard_context: false,
        ..GenerationRepresentationContract::full()
    }
}

/// URL/inline user image perception only; file IDs and other media stay rejected.
/// Sources: <https://api-docs.deepseek.com/guides/vision>,
/// <https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/multimodal-understanding>.
fn image_contract(
    replay_origin: Option<crate::semantic::value::ReplayOrigin>,
) -> GenerationRepresentationContract {
    GenerationRepresentationContract {
        image_input: true,
        ..wire_contract(replay_origin)
    }
}

fn execution_contract() -> ExecutionContract {
    ExecutionContract {
        streaming: true,
        retry_before_commit: false,
        request_body_limit: 256 * 1024,
        response_body_limit: 8 * 1024 * 1024,
        credential_kind: CredentialKind::ApiKey,
        timeout_ms: 120_000,
    }
}

fn endpoint(
    id: &str,
    provider: crate::provider::ProviderDefinition,
    protocol: ProtocolProfile,
    upstream_model: &str,
    credential: &str,
    replay_origin: Option<crate::semantic::value::ReplayOrigin>,
) -> Endpoint {
    debug_assert!(ident_ok(id));
    let path = match protocol {
        ProtocolProfile::OpenAiChat => provider.chat_completions,
        ProtocolProfile::OpenAiResponses => {
            provider.responses.expect("admitted native Responses entry")
        }
    };
    let dialect = match provider.id.as_str() {
        "deepseek" => crate::adapter::Dialect::DeepSeek,
        "xiaomi" => crate::adapter::Dialect::Xiaomi,
        "openrouter" => crate::adapter::Dialect::OpenRouter,
        "longcat" => crate::adapter::Dialect::LongCat,
        "nvidia" => crate::adapter::Dialect::Nvidia,
        // Sharing wire rules never shares the origin, credential or replay scope.
        "bailian" | "aliyun-tokenplan-cn" => crate::adapter::Dialect::Bailian,
        "kimi" => crate::adapter::Dialect::Kimi,
        "zhipu" => crate::adapter::Dialect::Zhipu,
        _ => unreachable!("fixed provider catalog"),
    };
    let family = match protocol {
        ProtocolProfile::OpenAiChat => crate::protocol::openai::Profile::Chat,
        ProtocolProfile::OpenAiResponses => crate::protocol::openai::Profile::Responses,
    };
    let adapter =
        crate::adapter::Adapter::new(family, dialect, Some(replay_scope(provider.id.as_str())));
    Endpoint {
        id: EndpointId::new(id).expect("static identity"),
        provider: provider.id,
        target: EndpointTarget {
            origin: provider.origin,
            path,
        },
        task: TaskKind::Generation,
        protocol,
        upstream_model: upstream_model.into(),
        representation: adapter.contract(&if dialect == crate::adapter::Dialect::OpenRouter {
            luna_contract(replay_origin)
        } else if matches!(
            (dialect, upstream_model),
            (crate::adapter::Dialect::DeepSeek, "deepseek-flash")
                | (
                    crate::adapter::Dialect::Xiaomi,
                    "mimo-v2.6-flash" | "mimo-v2.6-pro"
                )
        ) {
            image_contract(replay_origin)
        } else {
            wire_contract(replay_origin)
        }),
        execution: execution_contract(),
        credential: CredentialBindingId::new(credential).expect("static binding"),
    }
}

/// DeepSeek entries; `deepseek-flash` is the admitted upstream binding.
pub fn deepseek_endpoints() -> Vec<Endpoint> {
    vec![
        endpoint(
            "deepseek-responses",
            catalog::deepseek(),
            ProtocolProfile::OpenAiResponses,
            "deepseek-flash",
            "deepseek-api-key",
            // DeepSeek reports encrypted reasoning on Responses; the fixed
            // target declares its same-origin replay scope.
            Some(replay_scope("deepseek")),
        ),
        endpoint(
            "deepseek-chat",
            catalog::deepseek(),
            ProtocolProfile::OpenAiChat,
            "deepseek-flash",
            "deepseek-api-key",
            None,
        ),
    ]
}

/// Xiaomi MiMo entries share credentials, not model or endpoint identities.
pub fn xiaomi_endpoints() -> Vec<Endpoint> {
    vec![
        endpoint(
            "xiaomi-responses",
            catalog::xiaomi(),
            ProtocolProfile::OpenAiResponses,
            "mimo-v2.6-pro",
            "xiaomi-api-key",
            None,
        ),
        endpoint(
            "xiaomi-chat",
            catalog::xiaomi(),
            ProtocolProfile::OpenAiChat,
            "mimo-v2.6-pro",
            "xiaomi-api-key",
            None,
        ),
        endpoint(
            "xiaomi-flash-responses",
            catalog::xiaomi(),
            ProtocolProfile::OpenAiResponses,
            "mimo-v2.6-flash",
            "xiaomi-api-key",
            None,
        ),
        endpoint(
            "xiaomi-flash-chat",
            catalog::xiaomi(),
            ProtocolProfile::OpenAiChat,
            "mimo-v2.6-flash",
            "xiaomi-api-key",
            None,
        ),
    ]
}

pub fn deepseek_route() -> Route {
    Route {
        id: RouteId::new("deepseek-generation").expect("static identity"),
        task: TaskKind::Generation,
        // Responses-first fixed order; fallback is only possible pre-commit.
        endpoints: vec![
            EndpointId::new("deepseek-responses").expect("static identity"),
            EndpointId::new("deepseek-chat").expect("static identity"),
        ],
    }
}

pub fn xiaomi_route() -> Route {
    Route {
        id: RouteId::new("xiaomi-generation").expect("static identity"),
        task: TaskKind::Generation,
        endpoints: vec![
            EndpointId::new("xiaomi-responses").expect("static identity"),
            EndpointId::new("xiaomi-chat").expect("static identity"),
        ],
    }
}

pub fn xiaomi_flash_route() -> Route {
    Route {
        id: RouteId::new("xiaomi-flash-generation").expect("static identity"),
        task: TaskKind::Generation,
        endpoints: vec![
            EndpointId::new("xiaomi-flash-responses").expect("static identity"),
            EndpointId::new("xiaomi-flash-chat").expect("static identity"),
        ],
    }
}

pub fn mimo_v2_6_flash() -> PublicModel {
    PublicModel {
        id: ModelId::new("mimo-v2.6-flash").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("xiaomi-flash-generation").expect("static identity"),
        contract: image_contract(None),
    }
}

pub fn deepseek_flash() -> PublicModel {
    PublicModel {
        id: ModelId::new("deepseek-flash").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("deepseek-generation").expect("static identity"),
        contract: image_contract(None),
    }
}

pub fn mimo_v2_6_pro() -> PublicModel {
    PublicModel {
        id: ModelId::new("mimo-v2.6-pro").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("xiaomi-generation").expect("static identity"),
        contract: image_contract(None),
    }
}

// Public catalog parameters do not declare sampling, logprobs or parallel calls.
// Media is a codec gap, not a claim that the upstream model lacks media support.
fn luna_contract(
    replay_origin: Option<crate::semantic::value::ReplayOrigin>,
) -> GenerationRepresentationContract {
    GenerationRepresentationContract {
        temperature: false,
        top_p: false,
        logprobs: false,
        verbosity: false,
        truncation: false,
        parallel_tool_calls: false,
        ..wire_contract(replay_origin)
    }
}

pub fn openrouter_endpoints() -> Vec<Endpoint> {
    vec![
        endpoint(
            "openrouter-responses",
            catalog::openrouter(),
            ProtocolProfile::OpenAiResponses,
            "openai/gpt-6-luna",
            "openrouter-api-key",
            Some(replay_scope("openrouter")),
        ),
        endpoint(
            "openrouter-chat",
            catalog::openrouter(),
            ProtocolProfile::OpenAiChat,
            "openai/gpt-6-luna",
            "openrouter-api-key",
            Some(replay_scope("openrouter")),
        ),
    ]
}

pub fn openrouter_route() -> Route {
    Route {
        id: RouteId::new("openrouter-generation").expect("static identity"),
        task: TaskKind::Generation,
        endpoints: vec![
            EndpointId::new("openrouter-responses").expect("static identity"),
            EndpointId::new("openrouter-chat").expect("static identity"),
        ],
    }
}

pub fn gpt_6_luna() -> PublicModel {
    PublicModel {
        id: ModelId::new("gpt-6-luna").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("openrouter-generation").expect("static identity"),
        contract: luna_contract(None),
    }
}

/// Fixed API-key bindings. Native Responses must be explicitly admitted,
/// independently of OpenAI-compatible Chat support.
pub struct ApiKeyBinding {
    pub provider: &'static str,
    /// Per-model Route/Endpoint namespace, independent of shared auth ownership.
    pub endpoint_prefix: &'static str,
    pub model: &'static str,
    pub upstream: &'static str,
    pub credential: &'static str,
    pub variable: &'static str,
    pub protocols: &'static [ProtocolProfile],
}
pub const API_KEY_BINDINGS: &[ApiKeyBinding] = &[
    ApiKeyBinding {
        provider: "longcat",
        endpoint_prefix: "longcat",
        model: "longcat-2.5-preview",
        upstream: "LongCat-2.5-Preview",
        credential: "longcat-api-key",
        variable: "OPENBRIDGE_LONGCAT_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
    ApiKeyBinding {
        provider: "nvidia",
        endpoint_prefix: "nvidia",
        model: "nemotron-3-super",
        upstream: "nvidia/nemotron-3-super-120b-a12b",
        credential: "nvidia-api-key",
        variable: "OPENBRIDGE_NVIDIA_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
    ApiKeyBinding {
        provider: "bailian",
        endpoint_prefix: "bailian",
        model: "qwen3.8-max",
        upstream: "qwen3.8-max",
        credential: "bailian-api-key",
        variable: "OPENBRIDGE_BAILIAN_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
    ApiKeyBinding {
        provider: "aliyun-tokenplan-cn",
        endpoint_prefix: "aliyun-tokenplan-cn",
        model: "qwen3.8-flash",
        upstream: "qwen3.8-flash",
        credential: "aliyun-tokenplan-cn-api-key",
        variable: "OPENBRIDGE_ALIYUN_TOKENPLAN_CN_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
    ApiKeyBinding {
        provider: "kimi",
        endpoint_prefix: "kimi",
        model: "kimi-k3",
        upstream: "kimi-k3",
        credential: "kimi-api-key",
        variable: "OPENBRIDGE_KIMI_API_KEY",
        protocols: &[ProtocolProfile::OpenAiChat],
    },
    ApiKeyBinding {
        provider: "zhipu",
        endpoint_prefix: "zhipu",
        model: "glm-5.3",
        upstream: "glm-5.3",
        credential: "zhipu-api-key",
        variable: "OPENBRIDGE_ZHIPU_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
    ApiKeyBinding {
        provider: "zhipu",
        endpoint_prefix: "zhipu-flash",
        model: "glm-5.3-flash",
        upstream: "glm-5.3-flash",
        credential: "zhipu-api-key",
        variable: "OPENBRIDGE_ZHIPU_API_KEY",
        protocols: &[
            ProtocolProfile::OpenAiResponses,
            ProtocolProfile::OpenAiChat,
        ],
    },
];

/// Compile the fixed default topology for the admitted slice.
pub fn default_topology() -> Result<CompiledTopology, TopologyError> {
    let providers = catalog::all();
    let mut endpoints = [
        deepseek_endpoints(),
        xiaomi_endpoints(),
        openrouter_endpoints(),
    ]
    .concat();
    let mut routes = vec![
        deepseek_route(),
        xiaomi_route(),
        xiaomi_flash_route(),
        openrouter_route(),
    ];
    let mut models = vec![
        deepseek_flash(),
        mimo_v2_6_pro(),
        mimo_v2_6_flash(),
        gpt_6_luna(),
    ];
    for binding in API_KEY_BINDINGS {
        let definition = providers
            .iter()
            .find(|p| p.id.as_str() == binding.provider)
            .expect("fixed provider")
            .clone();
        let route_id = RouteId::new(&format!("{}-generation", binding.endpoint_prefix))
            .expect("static identity");
        let mut members = Vec::new();
        for &protocol in binding.protocols {
            let suffix = match protocol {
                ProtocolProfile::OpenAiChat => "chat",
                ProtocolProfile::OpenAiResponses => "responses",
            };
            let endpoint_id = format!("{}-{suffix}", binding.endpoint_prefix);
            endpoints.push(endpoint(
                &endpoint_id,
                definition.clone(),
                protocol,
                binding.upstream,
                binding.credential,
                None,
            ));
            members.push(EndpointId::new(&endpoint_id).expect("static identity"));
        }
        routes.push(Route {
            id: route_id.clone(),
            task: TaskKind::Generation,
            endpoints: members,
        });
        models.push(PublicModel {
            id: ModelId::new(binding.model).expect("static identity"),
            task: TaskKind::Generation,
            route: route_id,
            contract: wire_contract(None),
        });
    }
    compile(providers, endpoints, routes, models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_topology_resolves_the_admitted_models_and_fixed_routes() {
        let topology = default_topology().unwrap();
        let model = topology.model("deepseek-flash").unwrap();
        assert_eq!(model.route.as_str(), "deepseek-generation");
        let order: Vec<_> = topology
            .route_endpoints(&model.route)
            .into_iter()
            .map(|e| (e.id.as_str(), e.protocol))
            .collect();
        assert_eq!(
            order,
            [
                ("deepseek-responses", ProtocolProfile::OpenAiResponses),
                ("deepseek-chat", ProtocolProfile::OpenAiChat),
            ]
        );
        assert!(topology.model("mimo-v2.6-pro").is_some());
        assert!(topology.model("gpt-42").is_none());
    }

    #[test]
    fn endpoints_pin_researched_targets_and_upstream_model_bindings() {
        let topology = default_topology().unwrap();
        let responses = topology
            .endpoint(&EndpointId::new("xiaomi-responses").unwrap())
            .unwrap();
        assert_eq!(
            responses.target.origin.as_str(),
            "https://api.xiaomimimo.com"
        );
        assert_eq!(responses.target.path.as_str(), "/v1/responses");
        assert_eq!(responses.upstream_model, "mimo-v2.6-pro");
        assert_eq!(responses.credential.as_str(), "xiaomi-api-key");
        let chat = topology
            .endpoint(&EndpointId::new("deepseek-chat").unwrap())
            .unwrap();
        assert_eq!(chat.target.origin.as_str(), "https://api.deepseek.com");
        assert_eq!(chat.target.path.as_str(), "/chat/completions");
        assert_eq!(chat.upstream_model, "deepseek-flash");
    }

    #[test]
    fn contracts_claim_only_documented_protocol_limits() {
        for endpoint in default_topology().unwrap().endpoints() {
            let c = &endpoint.representation;
            assert!(
                !c.strict_tools,
                "function strict is only guaranteed on /beta"
            );
            assert!(!c.custom_tools, "custom tools are undeclared");
            assert!(!c.audio_input && !c.file_input, "media undeclared");
            assert!(
                !c.cache_hints,
                "cache hints are undeclared on these entries"
            );
            assert!(c.tools && c.instructions && c.structured_output && c.reasoning);
            assert!(!endpoint.execution.retry_before_commit);
            assert!(endpoint.execution.streaming);
        }
    }
}
