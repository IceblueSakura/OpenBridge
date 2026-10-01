//! Encoding guards also protect crate-internal callers from forged representations.
use super::*;
use crate::semantic::{task::generation::*, value::Text};

fn text(value: &str) -> Text {
    Text::new(value, "synthetic", 256).unwrap()
}
#[test]
fn request_codecs_reject_unadmitted_result_semantics_before_rendering() {
    let outputs = [
        ToolOutput::Structured(StructuredToolOutput::new(serde_json::json!({"value":42})).unwrap()),
        ToolOutput::Error(ToolExecutionError {
            code: None,
            message: text("failed"),
        }),
        ToolOutput::Parts(vec![(
            PartId::new(1),
            ToolResultPart::Resource(Resource {
                kind: ResourceKind::Image,
                location: ResourceLocation::Url(text("https://example.invalid/image.png")),
                image_detail: None,
            }),
        )]),
    ];
    for output in outputs {
        let semantic = GenerationRequest::new(
            vec![
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
                ),
                (
                    ItemId::new(2),
                    Item::ToolResult(ToolResult {
                        call_id: text("c"),
                        output,
                        status: None,
                        context: CallContext::default(),
                    }),
                ),
            ],
            GenerationControls::default(),
        )
        .unwrap();
        let fidelity = FidelityRecords::default();
        for profile in [Profile::Chat, Profile::Responses] {
            let forged = RequestRepresentation {
                semantic: &semantic,
                fidelity: &fidelity,
                profile,
                adaptation: Default::default(),
            };
            let result = match profile {
                Profile::Chat => chat::encode_generation(&forged),
                Profile::Responses => responses::encode_generation(&forged),
            };
            assert!(matches!(result, Err(CodecError::Unsupported(_))));
        }
    }
}
