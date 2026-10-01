//! Compile explicit model declarations and deployment bindings. No alias inference.
mod bindings;
mod models;
use crate::{
    adapter::Adapter,
    lowering::generation::{GenerationRepresentationContract, ReportedFactPolicy},
    provider::{CredentialBindingId, CredentialKind, catalog},
    semantic::value::ReplayOrigin,
    topology::*,
};
pub use bindings::{API_KEY_BINDINGS, ApiKeyBinding};
impl ApiKeyBinding {
    pub fn public_model(&self) -> PublicModel {
        PublicModel {
            id: ModelId::new(self.model).expect("static id"),
            canonical_model: ModelId::new(self.canonical_model).expect("static id"),
            task: TaskKind::Generation,
            route: RouteId::new(&format!("{}-generation", self.endpoint_prefix))
                .expect("static id"),
            contract: self
                .public_slice
                .apply(models::contract(self.canonical_model)),
            standard_context: false,
            reported_facts: ReportedFactPolicy::Faithful,
        }
    }
    pub fn endpoint(&self, protocol: ProtocolProfile) -> Endpoint {
        let provider = (self.provider)();
        let (family, path, replay) = match protocol {
            ProtocolProfile::OpenAiChat => (
                crate::protocol::openai::Profile::Chat,
                provider.chat_completions.clone(),
                self.replay_chat,
            ),
            ProtocolProfile::OpenAiResponses => (
                crate::protocol::openai::Profile::Responses,
                provider.responses.clone().expect("declared entry"),
                self.replay_responses,
            ),
        };
        let scope = ReplayOrigin::new(provider.id.as_str()).expect("static scope");
        let contract = GenerationRepresentationContract {
            semantics: self
                .endpoint_slice
                .apply(models::contract(self.canonical_model)),
            replay_origin: replay.then_some(scope.clone()),
            cache: Adapter::new(family, self.dialect, None).adaptation.cache,
            identity_hints: false,
            standard_context: false,
            ..GenerationRepresentationContract::full()
        };
        Endpoint {
            id: self.endpoint_id(protocol),
            canonical_model: ModelId::new(self.canonical_model).expect("static id"),
            provider: provider.id.clone(),
            target: EndpointTarget {
                origin: provider.origin,
                path,
            },
            task: TaskKind::Generation,
            protocol,
            upstream_model: self.upstream.into(),
            representation: Adapter::new(family, self.dialect, Some(scope)).contract(&contract),
            execution: ExecutionContract {
                streaming: true,
                retry_before_commit: false,
                request_body_limit: 256 * 1024,
                response_body_limit: 8 * 1024 * 1024,
                credential_kind: CredentialKind::ApiKey,
                timeout_ms: 120_000,
            },
            credential: CredentialBindingId::new(self.credential).expect("static binding"),
        }
    }
    pub fn endpoint_id(&self, protocol: ProtocolProfile) -> EndpointId {
        let suffix = match protocol {
            ProtocolProfile::OpenAiChat => "chat",
            ProtocolProfile::OpenAiResponses => "responses",
        };
        EndpointId::new(&format!("{}-{suffix}", self.endpoint_prefix)).expect("static id")
    }
}
pub fn default_topology() -> Result<CompiledTopology, TopologyError> {
    let mut endpoints = vec![];
    let mut routes = vec![];
    let mut public_models = vec![];
    for binding in API_KEY_BINDINGS {
        let public = binding.public_model();
        routes.push(Route {
            id: public.route.clone(),
            task: public.task,
            policy: RoutePolicy::default(),
            endpoints: binding
                .protocols
                .iter()
                .map(|&p| binding.endpoint_id(p))
                .collect(),
        });
        endpoints.extend(binding.protocols.iter().map(|&p| binding.endpoint(p)));
        public_models.push(public);
    }
    compile(
        catalog::all(),
        endpoints,
        routes,
        public_models,
        models::canonical_models(),
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
        assert_eq!(
            topology
                .route_endpoints(&model.route)
                .iter()
                .map(|e| (e.id.as_str(), e.protocol))
                .collect::<Vec<_>>(),
            vec![
                ("deepseek-responses", ProtocolProfile::OpenAiResponses),
                ("deepseek-chat", ProtocolProfile::OpenAiChat)
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
            let c = &endpoint.representation.semantics;
            assert!(
                !c.strict_tools,
                "function strict is only guaranteed on /beta"
            );
            assert!(!c.custom_tools, "custom tools are undeclared");
            assert!(!c.audio_input && !c.file_input, "media undeclared");

            assert!(c.tools && c.instructions && c.structured_output && c.reasoning);
            assert!(!endpoint.execution.retry_before_commit);
            assert!(endpoint.execution.streaming);
        }
    }
}
