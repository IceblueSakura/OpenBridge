//! Tool result kind, artifact lifecycle and wire representability are independent.
use crate::events_support::text;
use openbridge::{
    lowering::generation::{
        GenerationRepresentationContract as Contract, RepresentationError, lower_request,
    },
    protocol::{fidelity::FidelityRecords, openai::Profile},
    semantic::task::generation::*,
};
use serde_json::json;
fn call() -> (ItemId, Item) {
    (
        ItemId::new(1),
        Item::ToolCall(ToolCall {
            call_id: text("c"),
            name: text("lookup"),
            arguments: "{}".into(),
            message: None,
            status: ItemLifecycle::Completed,
            context: CallContext::default(),
        }),
    )
}
fn request(
    output: ToolOutput,
    status: Option<ItemLifecycle>,
) -> Result<GenerationRequest, GenerationError> {
    GenerationRequest::new(
        vec![
            call(),
            (
                ItemId::new(2),
                Item::ToolResult(ToolResult {
                    call_id: text("c"),
                    output,
                    status,
                    context: CallContext::default(),
                }),
            ),
        ],
        GenerationControls::default(),
    )
}
fn image(url: &str) -> Resource {
    Resource {
        kind: ResourceKind::Image,
        location: ResourceLocation::Url(text(url)),
        image_detail: None,
    }
}
fn media(url: &str) -> ToolOutput {
    ToolOutput::Parts(vec![
        (PartId::new(10), ToolResultPart::Text(text("caption"))),
        (PartId::new(11), ToolResultPart::Resource(image(url))),
    ])
}
#[test]
fn structured_value_preserves_integer_precision_order_and_bounds() {
    let value: serde_json::Value =
        serde_json::from_str(r#"{"z":18446744073709551615,"a":null}"#).unwrap();
    let structured = StructuredToolOutput::new(value.clone()).unwrap();
    assert_eq!(structured.value()["z"].as_u64(), Some(u64::MAX));
    assert_eq!(
        structured
            .value()
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    let request = request(ToolOutput::Structured(structured), None).unwrap();
    assert!(GenerationRequirements::derive(&request).structured_tool_results);
    assert_eq!(request.continuation(), Continuation::Unreported);
    let mut deep = json!(null);
    for _ in 0..40 {
        deep = json!([deep]);
    }
    assert_eq!(
        StructuredToolOutput::new(deep).unwrap_err(),
        GenerationError::Limit
    );
    assert!(StructuredToolOutput::new(json!(vec![0; 8193])).is_err());
    assert!(StructuredToolOutput::new(json!("x".repeat(MAX_TEXT_BYTES + 1))).is_err());
}
#[test]
fn execution_error_is_a_terminal_result_not_a_generation_failure() {
    let error = ToolOutput::Error(ToolExecutionError {
        code: Some(text("not_found")),
        message: text("synthetic failure"),
    });
    assert!(request(error.clone(), Some(ItemLifecycle::InProgress)).is_err());
    let history = request(error, Some(ItemLifecycle::Completed)).unwrap();
    let response = GenerationResponse::new(vec![call()], Outcome::Completed).unwrap();
    let exchange = ResponseContinuation::new(
        ResponseRelation::new(TurnId::new(1), ResponseId::new(1)),
        &response,
    );
    assert_eq!(
        exchange.inspect(&history).unwrap(),
        ResultReadiness::ResultsComplete
    );
    assert_eq!(response.outcome(), Outcome::Completed);
    assert!(GenerationRequirements::derive(&history).tool_result_errors);
}
#[test]
fn ordered_media_has_unique_parts_and_distinct_requirements() {
    let history = request(
        media("https://example.invalid/a.png"),
        Some(ItemLifecycle::Incomplete),
    )
    .unwrap();
    let requirements = GenerationRequirements::derive(&history);
    assert_eq!(requirements.tool_result_images, 1);
    assert_eq!(requirements.image_inputs, 0);
    let proof = RequestDependencyProof::capture(
        &history,
        HistoryDependency::PrefixThrough(ItemId::new(2)),
        true,
    )
    .unwrap();
    assert!(
        proof
            .check(
                &request(
                    media("https://example.invalid/b.png"),
                    Some(ItemLifecycle::Incomplete)
                )
                .unwrap()
            )
            .is_err()
    );
    assert!(
        request(
            ToolOutput::Parts(vec![
                (PartId::new(10), ToolResultPart::Text(text("one"))),
                (PartId::new(10), ToolResultPart::Text(text("two")))
            ]),
            None
        )
        .is_err()
    );
    assert!(request(media("https://user:secret@example.invalid/a.png"), None).is_err());
    let mut opaque = image("https://example.invalid/a.png");
    opaque.location = ResourceLocation::OpaqueReference(text("issuer-file"));
    assert!(
        request(
            ToolOutput::Parts(vec![(PartId::new(10), ToolResultPart::Resource(opaque))]),
            None
        )
        .is_err()
    );
}
#[test]
fn new_results_fail_lowering_without_a_carrier() {
    let outputs = [
        ToolOutput::Structured(StructuredToolOutput::new(json!({"answer":42})).unwrap()),
        ToolOutput::Error(ToolExecutionError {
            code: None,
            message: text("failed"),
        }),
        media("https://example.invalid/a.png"),
    ];
    for output in outputs {
        let history = request(output, None).unwrap();
        let fidelity = FidelityRecords::default();
        for profile in [Profile::Chat, Profile::Responses] {
            assert_eq!(
                lower_request(&history, &fidelity, profile, Contract::full()).err(),
                Some(RepresentationError::Tools)
            );
        }
        assert!(
            GenerationSemanticContract::text_images()
                .check(&history)
                .is_err()
        );
    }
}
