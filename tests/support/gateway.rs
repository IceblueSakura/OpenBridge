//! Trusted synthetic topology shared by Router and explicit SDK gates.
use openbridge::{
    gateway::{Entry, Gateway, Limits},
    lowering::generation::GenerationRepresentationContract as Contract,
    protocol::openai::Profile,
    provider::{
        AuthScheme, CredentialBindingId, CredentialKind, EndpointPath, ProviderDefinition,
        ProviderId, SecretMaterial, TrustedOrigin,
    },
    semantic::value::ReplayOrigin,
    topology::{
        CanonicalModel, Endpoint, EndpointId, EndpointTarget, ExecutionContract,
        GenerationSemanticContract, ModelId, ProtocolProfile, PublicModel, Route, RouteId,
        TaskKind, compile,
    },
};
use std::{collections::BTreeMap, sync::Arc};
pub const CLIENT_KEY: &str = "synthetic-gateway-client-token-0001";
pub fn gateway(origin: &str, limits: Limits) -> Gateway {
    let provider = ProviderDefinition {
        id: ProviderId::new("fixture").unwrap(),
        origin: TrustedOrigin::parse(origin).unwrap(),
        chat_completions: EndpointPath::new("/chat/completions").unwrap(),
        responses: Some(EndpointPath::new("/responses").unwrap()),
        auth: AuthScheme::Bearer,
    };
    let endpoints = [
        ("chat", ProtocolProfile::OpenAiChat),
        ("responses", ProtocolProfile::OpenAiResponses),
    ]
    .into_iter()
    .map(|(id, protocol)| Endpoint {
        id: EndpointId::new(id).unwrap(),
        provider: provider.id.clone(),
        target: EndpointTarget {
            origin: provider.origin.clone(),
            path: if protocol == ProtocolProfile::OpenAiChat {
                provider.chat_completions.clone()
            } else {
                provider.responses.clone().expect("test Responses entry")
            },
        },
        task: TaskKind::Generation,
        protocol,
        upstream_model: "private-model".into(),
        canonical_model: ModelId::new("canonical-fixture").unwrap(),
        representation: Contract {
            replay_origin: if protocol == ProtocolProfile::OpenAiResponses {
                Some(ReplayOrigin::new("fixture").unwrap())
            } else {
                None
            },
            ..Contract::full()
        },
        execution: ExecutionContract {
            streaming: true,
            retry_before_commit: false,
            request_body_limit: 256 << 10,
            response_body_limit: 8 << 20,
            timeout_ms: 3000,
            credential_kind: CredentialKind::ApiKey,
        },
        credential: CredentialBindingId::new("fixture-key").unwrap(),
    })
    .collect();
    let route = Route {
        id: RouteId::new("fixture-route").unwrap(),
        task: TaskKind::Generation,
        endpoints: vec![
            EndpointId::new("responses").unwrap(),
            EndpointId::new("chat").unwrap(),
        ],
    };
    let models = ["public-model", "cross-model"]
        .into_iter()
        .map(|label| PublicModel {
            id: ModelId::new(label).unwrap(),
            canonical_model: ModelId::new("canonical-fixture").unwrap(),
            task: TaskKind::Generation,
            route: route.id.clone(),
            contract: GenerationSemanticContract::full(),
            reported_facts: openbridge::lowering::generation::ReportedFactPolicy::Faithful,
        })
        .collect();
    let canonical = CanonicalModel {
        id: ModelId::new("canonical-fixture").unwrap(),
        task: TaskKind::Generation,
        contract: GenerationSemanticContract::full(),
    };
    let topology = compile(
        vec![provider],
        endpoints,
        vec![route],
        models,
        vec![canonical],
    )
    .unwrap();
    let entries = vec![
        Entry {
            model: "public-model".into(),
            protocol: Profile::Chat,
            endpoint: EndpointId::new("chat").unwrap(),
        },
        Entry {
            model: "public-model".into(),
            protocol: Profile::Responses,
            endpoint: EndpointId::new("responses").unwrap(),
        },
        Entry {
            model: "cross-model".into(),
            protocol: Profile::Responses,
            endpoint: EndpointId::new("chat").unwrap(),
        },
    ];
    let credentials = BTreeMap::from([(
        CredentialBindingId::new("fixture-key").unwrap(),
        Arc::new(SecretMaterial::new("synthetic-upstream-credential-0001").unwrap()),
    )]);
    Gateway::new(
        topology,
        entries,
        credentials,
        SecretMaterial::new(CLIENT_KEY).unwrap(),
        limits,
        None,
    )
    .unwrap()
}
