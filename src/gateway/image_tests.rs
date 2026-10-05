//! Operator image budgets apply after typed decode and before any projection/publication.
use super::*;
use crate::semantic::task::image_generation::{
    ImageData, ImageGenerationRequest, ImageGenerationResponse,
};
use base64::Engine;
#[test]
fn image_decoded_budget_is_independent_and_inclusive() {
    let encoded = base64::engine::general_purpose::STANDARD.encode(vec![0; 3 << 20]);
    let response = ImageGenerationResponse::new(1, ImageData::new(&encoded).unwrap());
    let request = ImageGenerationRequest::new("synthetic").unwrap();
    assert!(
        check_response(
            &response,
            &request,
            super::super::Limits::default().image_bytes
        )
        .is_err()
    );
    assert!(check_response(&response, &request, 3 << 20).is_ok());
    assert!(check_response(&response, &request, (3 << 20) - 1).is_err());
}
