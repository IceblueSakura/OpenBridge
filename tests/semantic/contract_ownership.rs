//! Semantic admission must not depend on a wire profile or Provider binding.
use openbridge::{
    adapter::{Adapter, Dialect},
    protocol::openai::Profile,
    semantic::task::generation::{GenerationFeature, GenerationSemanticContract},
};
#[test]
fn semantic_contract_checks_final_request_without_a_representation_contract() {
    let request=Adapter::new(Profile::Responses,Dialect::Standard,None).decode_request(br#"{"model":"alias","input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.invalid/a"}]}]}"#).unwrap();
    let mut contract = GenerationSemanticContract::text_images();
    assert!(contract.check(&request.task.semantic).is_ok());
    contract.image_input = false;
    assert_eq!(
        contract.check(&request.task.semantic).unwrap_err(),
        GenerationFeature::ImageInput
    );
}
