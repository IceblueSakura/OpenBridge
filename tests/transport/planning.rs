//! Candidate filtering must be value-sensitive and preserve trusted ordering.
use openbridge::{
    adapter::{Adapter, Dialect},
    execution::ExecutionPlan,
    lowering::generation::GenerationRepresentationContract as Representation,
    protocol::openai::Profile,
    provider::{CredentialBindingId, catalog},
    semantic::task::generation::ImageFormat,
    topology::*,
};
use serde_json::json;
fn topology(policy: CandidatePolicy) -> CompiledTopology {
    let model_id = ModelId::new("synthetic-canonical").unwrap();
    let mut endpoints = vec![];
    for (id, provider, formats) in [
        ("a", catalog::deepseek(), vec![ImageFormat::Png]),
        ("b", catalog::xiaomi(), vec![ImageFormat::Bmp]),
    ] {
        let mut representation = Adapter::new(Profile::Responses, Dialect::OpenBridge, None)
            .contract(&Representation::full());
        representation.images.inline_formats = formats;
        endpoints.push(Endpoint {
            id: EndpointId::new(id).unwrap(),
            provider: provider.id.clone(),
            target: EndpointTarget {
                origin: provider.origin.clone(),
                path: provider.responses.clone().unwrap(),
            },
            canonical_model: model_id.clone(),
            task: TaskKind::Generation,
            protocol: ProtocolProfile::OpenAiResponses,
            upstream_model: format!("{id}-alias"),
            representation,
            execution: ExecutionContract {
                streaming: true,
                retry_before_commit: false,
                request_body_limit: 4096,
                response_body_limit: 4096,
                timeout_ms: 1000,
                credential_kind: openbridge::provider::CredentialKind::ApiKey,
            },
            credential: CredentialBindingId::new(&format!("{id}-key")).unwrap(),
        });
    }
    let route = Route {
        id: RouteId::new("route").unwrap(),
        task: TaskKind::Generation,
        endpoints: vec![EndpointId::new("a").unwrap(), EndpointId::new("b").unwrap()],
        policy: RoutePolicy {
            candidates: policy,
            ..RoutePolicy::default()
        },
    };
    let public = PublicModel::new(
        ModelId::new("public").unwrap(),
        model_id.clone(),
        TaskKind::Generation,
        route.id.clone(),
        GenerationSemanticContract::text_images(),
    );
    compile(
        vec![catalog::deepseek(), catalog::xiaomi()],
        endpoints,
        vec![route],
        vec![public],
        vec![CanonicalModel {
            id: model_id,
            task: TaskKind::Generation,
            contract: GenerationSemanticContract::full(),
        }],
    )
    .unwrap()
}
#[test]
fn full_requests_are_filtered_without_mutation_or_reordering() {
    let client = Adapter::new(Profile::Responses, Dialect::OpenBridge, None);
    let request=client.decode_request(&serde_json::to_vec(&json!({"model":"public","input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/bmp;base64,AQ=="}]}]})).unwrap()).unwrap();
    let before = request.clone();
    let plan =
        ExecutionPlan::for_request(&topology(CandidatePolicy::SkipUnrepresentable), &request)
            .unwrap();
    assert_eq!(
        plan.candidates
            .iter()
            .map(|c| c.endpoint_id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    assert_eq!(request, before);
    assert_eq!(plan.rejections.len(), 1);
    assert_eq!(plan.rejections[0].endpoint_id.as_str(), "a");
    assert_eq!(
        plan.rejections[0].reason,
        openbridge::execution::plan::RejectionReason::Image
    );
    let compiled = topology(CandidatePolicy::SkipUnrepresentable);
    let active = openbridge::execution::plan::select_candidates(
        &request,
        CandidatePolicy::SkipUnrepresentable,
        compiled.route_endpoints(&RouteId::new("route").unwrap()),
    )
    .unwrap();
    assert_eq!(active.candidates, plan.candidates);
    assert_eq!(active.rejections, plan.rejections);
    assert!(ExecutionPlan::for_request(&topology(CandidatePolicy::RequireAll), &request).is_err());
    let both=client.decode_request(&serde_json::to_vec(&json!({"model":"public","input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/bmp;base64,AQ=="},{"type":"input_image","image_url":"data:image/png;base64,AQ=="}]}]})).unwrap()).unwrap();
    let error = ExecutionPlan::for_request(&topology(CandidatePolicy::SkipUnrepresentable), &both)
        .unwrap_err();
    let openbridge::execution::PlanError::NoCandidate { rejections } = error else {
        panic!("expected all candidate rejections")
    };
    assert_eq!(rejections.len(), 2);
    assert!(
        rejections
            .iter()
            .all(|r| r.reason == openbridge::execution::plan::RejectionReason::Image)
    );
    let text = client
        .decode_request(br#"{"model":"public","input":"x"}"#)
        .unwrap();
    let plan =
        ExecutionPlan::for_request(&topology(CandidatePolicy::SkipUnrepresentable), &text).unwrap();
    assert_eq!(
        plan.candidates
            .iter()
            .map(|c| c.endpoint_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    let endpoint = compiled.endpoint(&EndpointId::new("a").unwrap()).unwrap();
    assert_eq!(
        openbridge::execution::plan::select_candidates(
            &text,
            CandidatePolicy::SkipUnrepresentable,
            std::iter::repeat_n(endpoint, 65)
        ),
        Err(openbridge::execution::PlanError::CandidateLimit)
    );
}
