//! Compile trusted topology input into one immutable relation set.

use crate::{
    lowering::generation::GenerationRepresentationContract,
    provider::ProviderDefinition,
    topology::{Endpoint, EndpointId, ProtocolProfile, PublicModel, Route, RouteId},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TopologyError {
    #[error("duplicate provider")]
    DuplicateProvider,
    #[error("duplicate endpoint")]
    DuplicateEndpoint,
    #[error("duplicate route")]
    DuplicateRoute,
    #[error("duplicate public model")]
    DuplicateModel,
    #[error("endpoint references an unknown provider")]
    UnknownProvider,
    #[error("route references an unknown endpoint")]
    UnknownEndpoint,
    #[error("model references an unknown route")]
    UnknownRoute,
    #[error("endpoint target is not bound to its provider origin/entry")]
    TargetMismatch,
    #[error("task kind mismatch across topology relations")]
    TaskMismatch,
    #[error("route has no candidates")]
    EmptyRoute,
    #[error("public contract is not satisfied by every route candidate")]
    ContractUnsatisfiable,
    #[error("execution limits must be non-zero")]
    InvalidExecutionLimits,
    #[error("invalid upstream model binding")]
    InvalidModelBinding,
}

/// `promise` must be pointwise implied by `endpoint`. Whether a narrower promise
/// is published is a route policy decision made in trusted configuration.
fn promised(
    promise: &GenerationRepresentationContract,
    endpoint: &GenerationRepresentationContract,
) -> bool {
    let flags = [
        (promise.instructions, endpoint.instructions),
        (promise.temperature, endpoint.temperature),
        (promise.max_output_tokens, endpoint.max_output_tokens),
        (promise.tools, endpoint.tools),
        (promise.custom_tools, endpoint.custom_tools),
        (promise.text_metadata, endpoint.text_metadata),
        (promise.top_p, endpoint.top_p),
        (promise.logprobs, endpoint.logprobs),
        (promise.verbosity, endpoint.verbosity),
        (promise.truncation, endpoint.truncation),
        (promise.structured_output, endpoint.structured_output),
        (promise.reasoning, endpoint.reasoning),
        (promise.image_input, endpoint.image_input),
        (promise.audio_input, endpoint.audio_input),
        (promise.file_input, endpoint.file_input),
        (promise.parallel_tool_calls, endpoint.parallel_tool_calls),
        (promise.strict_tools, endpoint.strict_tools),
        (promise.cache_hints, endpoint.cache_hints),
        (promise.standard_context, endpoint.standard_context),
    ];
    let replay_ok = promise
        .replay_origin
        .as_ref()
        .is_none_or(|p| endpoint.replay_origin.as_ref() == Some(p));
    flags.into_iter().all(|(p, e)| !p || e) && replay_ok
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledTopology {
    providers: BTreeMap<String, ProviderDefinition>,
    endpoints: BTreeMap<String, Endpoint>,
    routes: BTreeMap<String, Route>,
    models: BTreeMap<String, PublicModel>,
}

impl CompiledTopology {
    pub fn provider(&self, id: &str) -> Option<&ProviderDefinition> {
        self.providers.get(id)
    }

    pub fn endpoint(&self, id: &EndpointId) -> Option<&Endpoint> {
        self.endpoints.get(id.as_str())
    }

    pub fn route(&self, id: &RouteId) -> Option<&Route> {
        self.routes.get(id.as_str())
    }

    pub fn model(&self, label: &str) -> Option<&PublicModel> {
        self.models.get(label)
    }

    /// Route candidates in fixed order; none for unknown routes.
    pub fn route_endpoints(&self, id: &RouteId) -> Vec<&Endpoint> {
        self.routes
            .get(id.as_str())
            .map(|route| {
                route
                    .endpoints
                    .iter()
                    .filter_map(|e| self.endpoints.get(e.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn models(&self) -> impl Iterator<Item = &PublicModel> {
        self.models.values()
    }

    pub fn endpoints(&self) -> impl Iterator<Item = &Endpoint> {
        self.endpoints.values()
    }
}

/// Compile immutable topology. Validation pins provider/target binding, fixed
/// route order and public-contract satisfiability before any request exists.
pub fn compile(
    providers: Vec<ProviderDefinition>,
    endpoints: Vec<Endpoint>,
    routes: Vec<Route>,
    models: Vec<PublicModel>,
) -> Result<CompiledTopology, TopologyError> {
    let mut provider_map = BTreeMap::new();
    for provider in providers {
        if provider_map
            .insert(provider.id.as_str().to_string(), provider)
            .is_some()
        {
            return Err(TopologyError::DuplicateProvider);
        }
    }
    let mut endpoint_map = BTreeMap::new();
    for endpoint in endpoints {
        endpoint.validate()?;
        let definition = provider_map
            .get(endpoint.provider.as_str())
            .ok_or(TopologyError::UnknownProvider)?;
        if definition.origin != endpoint.target.origin {
            return Err(TopologyError::TargetMismatch);
        }
        let bound_path = match endpoint.protocol {
            ProtocolProfile::OpenAiChat => &definition.chat_completions,
            ProtocolProfile::OpenAiResponses => definition
                .responses
                .as_ref()
                .ok_or(TopologyError::TargetMismatch)?,
        };
        if bound_path != &endpoint.target.path {
            return Err(TopologyError::TargetMismatch);
        }
        if definition.auth.kind() != endpoint.execution.credential_kind {
            return Err(TopologyError::TargetMismatch);
        }
        if endpoint_map
            .insert(endpoint.id.as_str().to_string(), endpoint)
            .is_some()
        {
            return Err(TopologyError::DuplicateEndpoint);
        }
    }
    let mut route_map = BTreeMap::new();
    for route in routes {
        if route.endpoints.is_empty() {
            return Err(TopologyError::EmptyRoute);
        }
        for candidate in &route.endpoints {
            let endpoint = endpoint_map
                .get(candidate.as_str())
                .ok_or(TopologyError::UnknownEndpoint)?;
            if endpoint.task != route.task {
                return Err(TopologyError::TaskMismatch);
            }
        }
        if route_map
            .insert(route.id.as_str().to_string(), route)
            .is_some()
        {
            return Err(TopologyError::DuplicateRoute);
        }
    }
    let mut model_map = BTreeMap::new();
    for model in models {
        let route = route_map
            .get(model.route.as_str())
            .ok_or(TopologyError::UnknownRoute)?;
        if route.task != model.task {
            return Err(TopologyError::TaskMismatch);
        }
        for candidate in &route.endpoints {
            let endpoint = &endpoint_map[candidate.as_str()];
            if !promised(&model.contract, &endpoint.representation) {
                return Err(TopologyError::ContractUnsatisfiable);
            }
        }
        if model_map
            .insert(model.id.as_str().to_string(), model)
            .is_some()
        {
            return Err(TopologyError::DuplicateModel);
        }
    }
    Ok(CompiledTopology {
        providers: provider_map,
        endpoints: endpoint_map,
        routes: route_map,
        models: model_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lowering::generation::GenerationRepresentationContract,
        provider::{
            CredentialBindingId, CredentialKind, EndpointPath, ProviderDefinition, ProviderId,
            TrustedOrigin, auth::AuthScheme,
        },
        topology::{Endpoint, EndpointTarget, ExecutionContract, ModelId, TaskKind},
    };

    fn provider() -> ProviderDefinition {
        ProviderDefinition {
            id: ProviderId::new("fixture").unwrap(),
            origin: TrustedOrigin::parse("http://127.0.0.1:39217").unwrap(),
            chat_completions: EndpointPath::new("/chat/completions").unwrap(),
            responses: Some(EndpointPath::new("/responses").unwrap()),
            auth: AuthScheme::Bearer,
        }
    }

    fn endpoint(protocol: ProtocolProfile) -> Endpoint {
        let definition = provider();
        let path = match protocol {
            ProtocolProfile::OpenAiChat => definition.chat_completions,
            ProtocolProfile::OpenAiResponses => definition.responses.expect("test Responses entry"),
        };
        Endpoint {
            id: EndpointId::new("fixture-endpoint").unwrap(),
            provider: ProviderId::new("fixture").unwrap(),
            target: EndpointTarget {
                origin: definition.origin,
                path,
            },
            task: TaskKind::Generation,
            protocol,
            upstream_model: "fixture-model".into(),
            representation: GenerationRepresentationContract::full(),
            execution: ExecutionContract {
                streaming: true,
                retry_before_commit: false,
                request_body_limit: 4096,
                response_body_limit: 4096,
                credential_kind: CredentialKind::ApiKey,
                timeout_ms: 1000,
            },
            credential: CredentialBindingId::new("fixture-key").unwrap(),
        }
    }

    fn route() -> Route {
        Route {
            id: RouteId::new("fixture-route").unwrap(),
            task: TaskKind::Generation,
            endpoints: vec![EndpointId::new("fixture-endpoint").unwrap()],
        }
    }

    fn model(contract: GenerationRepresentationContract) -> PublicModel {
        PublicModel {
            id: ModelId::new("fixture-model").unwrap(),
            task: TaskKind::Generation,
            route: RouteId::new("fixture-route").unwrap(),
            contract,
        }
    }

    fn compile_fixture(
        endpoints: Vec<Endpoint>,
        routes: Vec<Route>,
        models: Vec<PublicModel>,
    ) -> Result<CompiledTopology, TopologyError> {
        compile(vec![provider()], endpoints, routes, models)
    }

    #[test]
    fn fixed_route_order_survives_compilation() {
        let mut responses = endpoint(ProtocolProfile::OpenAiResponses);
        responses.id = EndpointId::new("e-responses").unwrap();
        let mut chat = endpoint(ProtocolProfile::OpenAiChat);
        chat.id = EndpointId::new("e-chat").unwrap();
        let topology = compile_fixture(
            vec![chat, responses],
            vec![Route {
                id: RouteId::new("fixture-route").unwrap(),
                task: TaskKind::Generation,
                endpoints: vec![
                    EndpointId::new("e-responses").unwrap(),
                    EndpointId::new("e-chat").unwrap(),
                ],
            }],
            vec![model(GenerationRepresentationContract::full())],
        )
        .unwrap();
        let order: Vec<_> = topology
            .route_endpoints(&RouteId::new("fixture-route").unwrap())
            .into_iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(order, ["e-responses", "e-chat"]);
    }

    #[test]
    fn duplicate_relations_are_rejected() {
        assert_eq!(
            compile_fixture(
                vec![
                    endpoint(ProtocolProfile::OpenAiChat),
                    endpoint(ProtocolProfile::OpenAiChat)
                ],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::DuplicateEndpoint)
        );
        assert_eq!(
            compile_fixture(
                vec![endpoint(ProtocolProfile::OpenAiChat)],
                vec![route(), route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::DuplicateRoute)
        );
        assert_eq!(
            compile_fixture(
                vec![endpoint(ProtocolProfile::OpenAiChat)],
                vec![route()],
                vec![
                    model(GenerationRepresentationContract::full()),
                    model(GenerationRepresentationContract::full()),
                ],
            ),
            Err(TopologyError::DuplicateModel)
        );
    }

    #[test]
    fn target_must_stay_bound_to_provider_origin_and_protocol_entry() {
        let mut chat_only = provider();
        chat_only.responses = None;
        assert_eq!(
            compile(
                vec![chat_only.clone()],
                vec![endpoint(ProtocolProfile::OpenAiResponses)],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())]
            ),
            Err(TopologyError::TargetMismatch)
        );
        assert!(
            compile(
                vec![chat_only],
                vec![endpoint(ProtocolProfile::OpenAiChat)],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())]
            )
            .is_ok()
        );
        // Wrong origin: business-selectable targets never compile.
        let mut wrong_origin = endpoint(ProtocolProfile::OpenAiResponses);
        wrong_origin.target.origin = TrustedOrigin::parse("http://127.0.0.1:40000").unwrap();
        assert_eq!(
            compile_fixture(
                vec![wrong_origin],
                vec![Route {
                    id: RouteId::new("fixture-route").unwrap(),
                    task: TaskKind::Generation,
                    endpoints: vec![EndpointId::new("fixture-endpoint").unwrap()],
                }],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::TargetMismatch)
        );
        // Wrong entry: a Responses endpoint cannot ride the chat entry path.
        let mut wrong_entry = endpoint(ProtocolProfile::OpenAiResponses);
        wrong_entry.target.path = provider().chat_completions;
        assert_eq!(
            compile_fixture(
                vec![wrong_entry],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::TargetMismatch)
        );
    }

    #[test]
    fn dangling_and_mismatched_relations_are_rejected() {
        assert_eq!(
            compile_fixture(
                vec![],
                vec![Route {
                    id: RouteId::new("fixture-route").unwrap(),
                    task: TaskKind::Generation,
                    endpoints: vec![EndpointId::new("missing").unwrap()],
                }],
                vec![],
            ),
            Err(TopologyError::UnknownEndpoint)
        );
        assert_eq!(
            compile_fixture(
                vec![endpoint(ProtocolProfile::OpenAiChat)],
                vec![],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::UnknownRoute)
        );
        assert_eq!(
            compile_fixture(
                vec![endpoint(ProtocolProfile::OpenAiChat)],
                vec![Route {
                    id: RouteId::new("fixture-route").unwrap(),
                    task: TaskKind::Generation,
                    endpoints: vec![],
                }],
                vec![],
            ),
            Err(TopologyError::EmptyRoute)
        );
        assert_eq!(
            compile_fixture(
                vec![Endpoint {
                    provider: ProviderId::new("missing").unwrap(),
                    ..endpoint(ProtocolProfile::OpenAiChat)
                }],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::UnknownProvider)
        );
    }

    #[test]
    fn public_contract_must_be_satisfiable_by_every_candidate() {
        let promise = GenerationRepresentationContract {
            strict_tools: true,
            ..GenerationRepresentationContract::full()
        };
        let candidate = GenerationRepresentationContract {
            strict_tools: false,
            ..GenerationRepresentationContract::full()
        };
        assert_eq!(
            compile_fixture(
                vec![Endpoint {
                    representation: candidate,
                    ..endpoint(ProtocolProfile::OpenAiChat)
                }],
                vec![route()],
                vec![model(promise)],
            ),
            Err(TopologyError::ContractUnsatisfiable)
        );
    }

    #[test]
    fn execution_limits_and_model_bindings_must_be_concrete() {
        assert_eq!(
            compile_fixture(
                vec![Endpoint {
                    execution: ExecutionContract {
                        request_body_limit: 0,
                        ..endpoint(ProtocolProfile::OpenAiChat).execution
                    },
                    ..endpoint(ProtocolProfile::OpenAiChat)
                }],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::InvalidExecutionLimits)
        );
        assert_eq!(
            compile_fixture(
                vec![Endpoint {
                    upstream_model: "model with spaces".into(),
                    ..endpoint(ProtocolProfile::OpenAiChat)
                }],
                vec![route()],
                vec![model(GenerationRepresentationContract::full())],
            ),
            Err(TopologyError::InvalidModelBinding)
        );
    }
}
