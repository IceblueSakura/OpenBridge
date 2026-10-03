//! Semantic admission must not depend on a wire profile or Provider binding.
use openbridge::semantic::{task::generation::*, value::Text};
#[test]
fn semantic_contract_checks_final_request_without_a_representation_contract() {
    let request = GenerationRequest::new(
        vec![(
            ItemId::new(1),
            Item::Message(Message {
                role: MessageRole::User,
                parts: vec![Part {
                    id: PartId::new(1),
                    content: ContentPart::Resource(Resource {
                        kind: ResourceKind::Image,
                        location: ResourceLocation::Url(
                            Text::new("https://example.invalid/a", "synthetic", 128).unwrap(),
                        ),
                        image_detail: None,
                    }),
                }],
                status: ItemLifecycle::Completed,
                phase: None,
            }),
        )],
        GenerationControls::default(),
    )
    .unwrap();
    let mut contract = GenerationSemanticContract::text_images();
    assert!(contract.check(&request).is_ok());
    contract.image_input = false;
    assert_eq!(
        contract.check(&request).unwrap_err(),
        GenerationFeature::ImageInput
    );
}
