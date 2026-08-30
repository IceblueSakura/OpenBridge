//! Canonical Event IR reducer and materializer contracts.

use openbridge::ir::generation::{
    BoundedBytes, CandidateId, ContentPart, EventEnvelope, EventInput, EventLimits, EventState,
    FinishReason, GenerationEvent, ItemHeader, ItemId, ItemIdentity, ItemRef, JsonObject,
    MaterializeError, MessageRole, OpaqueExposure, OpaqueKind, OutputIndex, OutputItem, PartDelta,
    PartId, PartIdentity, PartKind, PartRef, ReasoningPart, ReduceError, ResponseId,
    ResponseIdentity, Sequence, TerminalStatus, TextValue, ToolInput, ToolName, TurnTerminal,
    Usage, materialize, reduce,
};
use serde_json::json;

const LIMIT: usize = 1024;

fn text(value: &str) -> TextValue {
    TextValue::new(value, LIMIT).expect("fixture text must fit")
}

fn response_id(value: &str) -> ResponseId {
    ResponseId::new(value, LIMIT).expect("fixture response ID must fit")
}

fn candidate_id(value: &str) -> CandidateId {
    CandidateId::new(value, LIMIT).expect("fixture candidate ID must fit")
}

fn item_id(value: &str) -> ItemId {
    ItemId::new(value, LIMIT).expect("fixture item ID must fit")
}

fn part_id(value: &str) -> PartId {
    PartId::new(value, LIMIT).expect("fixture part ID must fit")
}

fn event(sequence: u64, event: GenerationEvent) -> EventInput {
    EventInput::Event(Box::new(EventEnvelope::new(Sequence::new(sequence), event)))
}

fn text_candidate_events(
    start: u64,
    candidate: CandidateId,
    index: u64,
    item: ItemId,
    value: &'static str,
) -> Vec<EventInput> {
    let part_name = format!("{}-part", item.as_str());
    let part = part_id(&part_name);
    vec![
        event(
            start,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(index),
                ),
            },
        ),
        event(
            start + 1,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate.clone()),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
            },
        ),
        event(
            start + 2,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item.clone()),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::Text,
            },
        ),
        event(
            start + 3,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::Text(text(value)),
            },
        ),
        event(
            start + 4,
            GenerationEvent::PartFinished {
                part: PartRef::new(part),
            },
        ),
        event(
            start + 5,
            GenerationEvent::ItemFinished {
                item: ItemRef::new(item),
            },
        ),
        event(
            start + 6,
            GenerationEvent::CandidateFinished {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                finish: FinishReason::Stop,
            },
        ),
    ]
}

fn text_turn_events() -> Vec<EventInput> {
    let candidate = candidate_id("candidate-0");
    let item = item_id("message-0");
    let part = part_id("text-0");
    vec![
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-0")),
            },
        ),
        event(
            1,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(0),
                ),
            },
        ),
        event(
            2,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate.clone()),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
            },
        ),
        event(
            3,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item.clone()),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::Text,
            },
        ),
        event(
            4,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::Text(text("hel")),
            },
        ),
        event(
            5,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::Text(text("lo")),
            },
        ),
        event(
            6,
            GenerationEvent::PartFinished {
                part: PartRef::new(part),
            },
        ),
        event(
            7,
            GenerationEvent::ItemFinished {
                item: ItemRef::new(item),
            },
        ),
        event(
            8,
            GenerationEvent::CandidateFinished {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                finish: FinishReason::Stop,
            },
        ),
        event(
            9,
            GenerationEvent::UsageSnapshot {
                usage: Usage::new(Some(3), Some(2), Some(5), None, None),
            },
        ),
        event(
            10,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Completed, None),
            },
        ),
        EventInput::Eof,
    ]
}

#[test]
fn text_event_replay_materializes_the_static_response() {
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    for input in text_turn_events() {
        state = reduce(state, input).expect("canonical text lifecycle must reduce");
    }

    let response = materialize(&state).expect("completed turn must materialize");
    assert_eq!(response.id().as_str(), "response-0");
    assert_eq!(response.usage().unwrap().total_tokens(), Some(5));
    let OutputItem::Message(message) = &response.candidates()[0].output()[0] else {
        panic!("text item must materialize as a message");
    };
    let ContentPart::Text(content) = &message.content()[0] else {
        panic!("message part must remain text");
    };
    assert_eq!(content.text().as_str(), "hello");
}

#[test]
fn fragmented_tool_arguments_parse_only_when_the_part_finishes() {
    let candidate = candidate_id("candidate-tool");
    let item = item_id("tool-item");
    let part = part_id("tool-arguments");
    let call = openbridge::ir::generation::CallId::new("call-1", LIMIT).unwrap();
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    let inputs = vec![
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-tool")),
            },
        ),
        event(
            1,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(0),
                ),
            },
        ),
        event(
            2,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate.clone()),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::ToolCall {
                    call: call.clone(),
                    tool: ToolName::new("lookup", LIMIT).unwrap(),
                },
            },
        ),
        event(
            3,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item.clone()),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::ToolArguments,
            },
        ),
        event(
            4,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::ToolArguments(text("{\"city\":")),
            },
        ),
        event(
            5,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::ToolArguments(text("\"Hangzhou\"}")),
            },
        ),
    ];
    for input in inputs {
        state = reduce(state, input).expect("open fragments need not be valid JSON yet");
    }
    assert_eq!(
        reduce(
            state.clone(),
            event(
                6,
                GenerationEvent::PartDelta {
                    part: PartRef::new(part.clone()),
                    delta: PartDelta::ToolArguments(text("broken")),
                },
            ),
        )
        .and_then(|state| {
            reduce(
                state,
                event(
                    7,
                    GenerationEvent::PartFinished {
                        part: PartRef::new(part.clone()),
                    },
                ),
            )
        }),
        Err(ReduceError::InvalidToolArguments)
    );

    state = reduce(
        state,
        event(
            6,
            GenerationEvent::PartFinished {
                part: PartRef::new(part),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            7,
            GenerationEvent::ItemFinished {
                item: ItemRef::new(item),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            8,
            GenerationEvent::CandidateFinished {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                finish: FinishReason::ToolCalls,
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            9,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Completed, None),
            },
        ),
    )
    .unwrap();

    let response = materialize(&state).unwrap();
    let OutputItem::ToolCall(call) = &response.candidates()[0].output()[0] else {
        panic!("tool item must remain a tool call");
    };
    let ToolInput::Function(arguments) = call.input() else {
        panic!("function arguments must materialize as JSON");
    };
    assert_eq!(
        JsonObject::new(json!({"city": "Hangzhou"}), LIMIT).unwrap(),
        arguments.clone()
    );
}

#[test]
fn sequence_identity_terminal_eof_and_bounds_fail_closed() {
    let limits = EventLimits::new(4, 5, 8).unwrap();
    let state = EventState::new(limits);
    assert_eq!(
        reduce(
            state.clone(),
            event(
                1,
                GenerationEvent::ResponseStarted {
                    response: ResponseIdentity::new(response_id("response")),
                },
            ),
        ),
        Err(ReduceError::InvalidSequence)
    );
    assert_eq!(
        reduce(state, EventInput::Eof),
        Err(ReduceError::EofBeforeTerminal)
    );

    let mut completed = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    for input in text_turn_events().into_iter().take(11) {
        completed = reduce(completed, input).unwrap();
    }
    completed = reduce(completed, EventInput::Eof).unwrap();
    assert_eq!(
        reduce(completed.clone(), EventInput::Eof),
        Err(ReduceError::DuplicateEof)
    );
    assert_eq!(
        reduce(
            completed,
            event(
                11,
                GenerationEvent::UsageSnapshot {
                    usage: Usage::new(None, None, None, None, None),
                },
            ),
        ),
        Err(ReduceError::InputAfterEof)
    );

    let candidate = candidate_id("candidate-bound");
    let item = item_id("item-bound");
    let part = part_id("part-bound");
    let mut bounded = EventState::new(EventLimits::new(8, 5, 8).unwrap());
    for input in [
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-bound")),
            },
        ),
        event(
            1,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(0),
                ),
            },
        ),
        event(
            2,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
            },
        ),
        event(
            3,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::Text,
            },
        ),
    ] {
        bounded = reduce(bounded, input).unwrap();
    }
    assert_eq!(
        reduce(
            bounded,
            event(
                4,
                GenerationEvent::PartDelta {
                    part: PartRef::new(part),
                    delta: PartDelta::Text(text("123456")),
                },
            ),
        ),
        Err(ReduceError::PartLimitExceeded)
    );
}

#[test]
fn non_completed_terminal_cannot_materialize_a_success_response() {
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    state = reduce(
        state,
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-failed")),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            1,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Failed, Some(text("provider failed"))),
            },
        ),
    )
    .unwrap();

    assert_eq!(
        materialize(&state),
        Err(MaterializeError::NonCompletedTerminal)
    );
}

#[test]
fn candidate_item_part_identity_and_parent_lifecycle_fail_closed() {
    let candidate = candidate_id("candidate-lifecycle");
    let item = item_id("item-lifecycle");
    let part = part_id("part-lifecycle");
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    state = reduce(
        state,
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-lifecycle")),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            1,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(0),
                ),
            },
        ),
    )
    .unwrap();
    assert_eq!(
        reduce(
            state.clone(),
            event(
                2,
                GenerationEvent::CandidateStarted {
                    candidate: openbridge::ir::generation::CandidateIdentity::new(
                        candidate.clone(),
                        OutputIndex::new(1),
                    ),
                },
            ),
        ),
        Err(ReduceError::DuplicateIdentity)
    );
    state = reduce(
        state,
        event(
            2,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate.clone()),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::Message {
                    role: MessageRole::Assistant,
                },
            },
        ),
    )
    .unwrap();
    assert_eq!(
        reduce(
            state.clone(),
            event(
                3,
                GenerationEvent::CandidateFinished {
                    candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                    finish: FinishReason::Stop,
                },
            ),
        ),
        Err(ReduceError::IncompleteChildren)
    );
    assert_eq!(
        reduce(
            state.clone(),
            event(
                3,
                GenerationEvent::PartDelta {
                    part: PartRef::new(part.clone()),
                    delta: PartDelta::Text(text("orphan")),
                },
            ),
        ),
        Err(ReduceError::UnknownReference)
    );
    state = reduce(
        state,
        event(
            3,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::Text,
            },
        ),
    )
    .unwrap();
    assert_eq!(
        reduce(
            state,
            event(
                4,
                GenerationEvent::PartFinished {
                    part: PartRef::new(part),
                },
            ),
        ),
        Err(ReduceError::InvalidItemShape)
    );
}

#[test]
fn usage_snapshots_keep_missing_and_zero_distinct_and_reject_regression() {
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    state = reduce(
        state,
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response_id("response-usage")),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            1,
            GenerationEvent::UsageSnapshot {
                usage: Usage::new(None, Some(0), Some(0), None, None),
            },
        ),
    )
    .unwrap();
    state = reduce(
        state,
        event(
            2,
            GenerationEvent::UsageSnapshot {
                usage: Usage::new(Some(0), Some(1), Some(1), Some(0), Some(0)),
            },
        ),
    )
    .unwrap();
    assert_eq!(state.usage().unwrap().input_tokens(), Some(0));
    assert_eq!(state.usage().unwrap().output_tokens(), Some(1));
    assert_eq!(
        reduce(
            state,
            event(
                3,
                GenerationEvent::UsageSnapshot {
                    usage: Usage::new(Some(0), Some(0), Some(1), Some(0), Some(0)),
                },
            ),
        ),
        Err(ReduceError::UsageRegressed)
    );
}

#[test]
fn event_and_turn_limits_are_independent() {
    fn open_part(limits: EventLimits) -> (EventState, PartId) {
        let candidate = candidate_id("candidate-limits");
        let item = item_id("item-limits");
        let part = part_id("part-limits");
        let mut state = EventState::new(limits);
        for input in [
            event(
                0,
                GenerationEvent::ResponseStarted {
                    response: ResponseIdentity::new(response_id("response-limits")),
                },
            ),
            event(
                1,
                GenerationEvent::CandidateStarted {
                    candidate: openbridge::ir::generation::CandidateIdentity::new(
                        candidate.clone(),
                        OutputIndex::new(0),
                    ),
                },
            ),
            event(
                2,
                GenerationEvent::ItemStarted {
                    candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                    item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                    header: ItemHeader::Message {
                        role: MessageRole::Assistant,
                    },
                },
            ),
            event(
                3,
                GenerationEvent::PartStarted {
                    item: ItemRef::new(item),
                    part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                    kind: PartKind::Text,
                },
            ),
        ] {
            state = reduce(state, input).unwrap();
        }
        (state, part)
    }

    let (state, part) = open_part(EventLimits::new(5, 20, 20).unwrap());
    assert_eq!(
        reduce(
            state,
            event(
                4,
                GenerationEvent::PartDelta {
                    part: PartRef::new(part),
                    delta: PartDelta::Text(text("123456")),
                },
            ),
        ),
        Err(ReduceError::EventLimitExceeded)
    );

    let (mut state, part) = open_part(EventLimits::new(4, 20, 5).unwrap());
    state = reduce(
        state,
        event(
            4,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::Text(text("1234")),
            },
        ),
    )
    .unwrap();
    assert_eq!(
        reduce(
            state,
            event(
                5,
                GenerationEvent::PartDelta {
                    part: PartRef::new(part),
                    delta: PartDelta::Text(text("56")),
                },
            ),
        ),
        Err(ReduceError::TurnLimitExceeded)
    );
}

#[test]
fn opaque_reasoning_survives_event_materialization_as_internal_state() {
    let response = response_id("response-opaque");
    let candidate = candidate_id("candidate-opaque");
    let item = item_id("reasoning-opaque");
    let part = part_id("part-opaque");
    let events = vec![
        event(
            0,
            GenerationEvent::ResponseStarted {
                response: ResponseIdentity::new(response),
            },
        ),
        event(
            1,
            GenerationEvent::CandidateStarted {
                candidate: openbridge::ir::generation::CandidateIdentity::new(
                    candidate.clone(),
                    OutputIndex::new(0),
                ),
            },
        ),
        event(
            2,
            GenerationEvent::ItemStarted {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate.clone()),
                item: ItemIdentity::new(item.clone(), OutputIndex::new(0), None),
                header: ItemHeader::Reasoning,
            },
        ),
        event(
            3,
            GenerationEvent::PartStarted {
                item: ItemRef::new(item.clone()),
                part: PartIdentity::new(part.clone(), OutputIndex::new(0)),
                kind: PartKind::Opaque,
            },
        ),
        event(
            4,
            GenerationEvent::PartDelta {
                part: PartRef::new(part.clone()),
                delta: PartDelta::Opaque(BoundedBytes::from_slice(b"opaque", 64).unwrap()),
            },
        ),
        event(
            5,
            GenerationEvent::PartFinished {
                part: PartRef::new(part),
            },
        ),
        event(
            6,
            GenerationEvent::ItemFinished {
                item: ItemRef::new(item),
            },
        ),
        event(
            7,
            GenerationEvent::CandidateFinished {
                candidate: openbridge::ir::generation::CandidateRef::new(candidate),
                finish: FinishReason::Stop,
            },
        ),
        event(
            8,
            GenerationEvent::Terminal {
                terminal: TurnTerminal::new(TerminalStatus::Completed, None),
            },
        ),
    ];
    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    for input in events {
        state = reduce(state, input).unwrap();
    }
    let response = materialize(&state).unwrap();
    let OutputItem::Reasoning(reasoning) = &response.candidates()[0].output()[0] else {
        panic!("opaque state must remain a reasoning item");
    };
    let ReasoningPart::Opaque(opaque) = &reasoning.parts()[0] else {
        panic!("opaque reasoning must not become visible text");
    };
    assert_eq!(opaque.kind(), OpaqueKind::EncryptedContent);
    assert_eq!(opaque.exposure(), OpaqueExposure::InternalOnly);
    assert_eq!(opaque.payload().as_slice(), b"opaque");
}

#[test]
fn multiple_candidates_materialize_by_output_index_not_start_order() {
    let mut inputs = vec![event(
        0,
        GenerationEvent::ResponseStarted {
            response: ResponseIdentity::new(response_id("response-multiple")),
        },
    )];
    inputs.extend(text_candidate_events(
        1,
        candidate_id("candidate-second"),
        1,
        item_id("item-second"),
        "second",
    ));
    inputs.extend(text_candidate_events(
        8,
        candidate_id("candidate-first"),
        0,
        item_id("item-first"),
        "first",
    ));
    inputs.push(event(
        15,
        GenerationEvent::Terminal {
            terminal: TurnTerminal::new(TerminalStatus::Completed, None),
        },
    ));

    let mut state = EventState::new(EventLimits::new(64, 256, 1024).unwrap());
    for input in inputs {
        state = reduce(state, input).unwrap();
    }
    let response = materialize(&state).unwrap();

    assert_eq!(response.candidates()[0].id().as_str(), "candidate-first");
    assert_eq!(response.candidates()[1].id().as_str(), "candidate-second");
}
