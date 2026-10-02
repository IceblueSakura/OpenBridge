//! Encoding guards also protect crate-internal callers from forged representations.
use super::*;
use crate::semantic::{task::generation::*, value::Text};

fn text(value: &str) -> Text {
    Text::new(value, "synthetic", 256).unwrap()
}
#[test]
fn request_codecs_enforce_result_carriers_before_rendering() {
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
        let media = matches!(&semantic.items()[1].1, Item::ToolResult(r) if matches!(r.output, ToolOutput::Parts(_)));
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
            if media && profile == Profile::Responses {
                assert_eq!(
                    result.unwrap()["input"][1]["output"],
                    serde_json::json!([
                        {"type":"input_image","image_url":"https://example.invalid/image.png"}
                    ])
                );
                let mut breakpoint = FidelityRecords::default();
                breakpoint.record_cache_breakpoint(PartId::new(1)).unwrap();
                let forged = RequestRepresentation {
                    semantic: &semantic,
                    fidelity: &breakpoint,
                    profile,
                    adaptation: Default::default(),
                };
                assert!(matches!(
                    responses::encode_generation(&forged),
                    Err(CodecError::Unsupported(_))
                ));
            } else {
                assert!(matches!(result, Err(CodecError::Unsupported(_))));
            }
        }
    }
}
