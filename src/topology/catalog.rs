//! Trusted text bindings for DeepSeek, Xiaomi MiMo and OpenRouter.
//! Each entry fixes its upstream model, wire adapter and credential ownership.
//!
//! Facts come from `docs/references/providers/{deepseek,xiaomi,openrouter}-api.md`. Model
//! capability is deliberately not widened here: the representation contract
//! claims wire-level representability plus documented protocol limits
//! (function strict is only guaranteed on DeepSeek `/beta`; custom tools and
//! media inputs are undeclared; cache-affinity hints are undeclared on these
//! entries and are omitted by approved inactive-hint omission). Observed model
//! behavior is recorded in implementation evidence, not in this catalog.

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
        ProtocolProfile::OpenAiResponses => provider.responses,
    };
    let dialect = match provider.id.as_str() {
        "deepseek" => crate::adapter::Dialect::DeepSeek,
        "xiaomi" => crate::adapter::Dialect::Xiaomi,
        "openrouter" => crate::adapter::Dialect::OpenRouter,
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

/// Xiaomi MiMo entries; `mimo-v2.6-pro` is the admitted upstream binding.
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

pub fn deepseek_flash() -> PublicModel {
    PublicModel {
        id: ModelId::new("deepseek-flash").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("deepseek-generation").expect("static identity"),
        contract: wire_contract(None),
    }
}

pub fn mimo_v2_6_pro() -> PublicModel {
    PublicModel {
        id: ModelId::new("mimo-v2.6-pro").expect("static identity"),
        task: TaskKind::Generation,
        route: RouteId::new("xiaomi-generation").expect("static identity"),
        contract: wire_contract(None),
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

/// Compile the fixed default topology for the admitted slice.
pub fn default_topology() -> Result<CompiledTopology, TopologyError> {
    compile(
        catalog::all(),
        [
            deepseek_endpoints(),
            xiaomi_endpoints(),
            openrouter_endpoints(),
        ]
        .concat(),
        vec![deepseek_route(), xiaomi_route(), openrouter_route()],
        vec![deepseek_flash(), mimo_v2_6_pro(), gpt_6_luna()],
    )
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
            assert!(
                !c.image_input && !c.audio_input && !c.file_input,
                "media undeclared"
            );
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
