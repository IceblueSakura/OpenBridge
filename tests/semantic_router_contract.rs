//! Production Router semantic regression, with an observation-level ablation experiment.
//!
//! Synthetic tasks supply data, not claims about model intelligence. The independent wire
//! projections protect conversion; bounded perturbations check the assertions, not production
//! source mutation coverage. Directional faults remain separate because the codecs are separate.

#[path = "support/semantic_replay.rs"]
mod semantic_replay;
mod support;

use semantic_replay::{ScenarioKind, SemanticEvent, SemanticObservation};
use serde_json::{Value, json};

#[tokio::test]
async fn production_router_preserves_selected_semantic_contracts() {
    let observations = semantic_replay::run_matrix().await;
    assert!(!observations.is_empty());
    for observation in &observations {
        assert!(accepts(observation), "{} semantic contract", observation.id);
        // Parallel calls are a set: reversing arrival order must not change the verdict.
        if observation.kind == ScenarioKind::ParallelArguments {
            let mut equivalent = observation.clone();
            equivalent.response.events.reverse();
            assert!(accepts(&equivalent), "{} equivalent order", observation.id);
        }
    }
    ablate(&observations);
}

fn same_events(actual: &[SemanticEvent], expected: &[SemanticEvent], unordered: bool) -> bool {
    if !unordered {
        return actual == expected;
    }
    // Remove matches rather than using a set, so duplicate calls cannot disappear.
    let mut remaining = expected.to_vec();
    for event in actual {
        let Some(index) = remaining.iter().position(|candidate| candidate == event) else {
            return false;
        };
        remaining.remove(index);
    }
    remaining.is_empty()
}

fn accepts(observation: &SemanticObservation) -> bool {
    observation.request.protocol == observation.direction.upstream()
        && observation.response.protocol == observation.direction.downstream()
        && observation.request.events == observation.expected_request_events
        && same_events(
            &observation.response.events,
            &observation.expected_response_events,
            observation.kind == ScenarioKind::ParallelArguments,
        )
        && observation.response.terminal_count == 1
        && observation.request.structured_schema == observation.expected_schema
        && observation.request.tools == observation.expected_tools
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    CallIdentity,
    CallName,
    Arguments,
    ResultAssociation,
    ResultValue,
    AssistantText,
    ToolDeclaration,
    StructuredConstraint,
    MissingTerminal,
}

impl Fault {
    const ALL: [Self; 9] = [
        Self::CallIdentity,
        Self::CallName,
        Self::Arguments,
        Self::ResultAssociation,
        Self::ResultValue,
        Self::AssistantText,
        Self::ToolDeclaration,
        Self::StructuredConstraint,
        Self::MissingTerminal,
    ];
}

/// Applies the same defect operator to every observation where its semantic field exists.
fn perturb(observation: &mut SemanticObservation, fault: Fault) -> bool {
    match fault {
        Fault::ToolDeclaration => {
            if observation.request.tools.is_empty() {
                return false;
            }
            observation.request.tools.clear();
            return true;
        }
        Fault::StructuredConstraint => {
            let Some(schema) = &mut observation.request.structured_schema else {
                return false;
            };
            *schema = json!({"type":"object"});
            return true;
        }
        Fault::MissingTerminal => {
            if !observation.stream {
                return false;
            }
            observation.response.terminal_count = 0;
            return true;
        }
        _ => {}
    }
    // Request history and generated responses share identities and values, not case-specific mutants.
    for event in observation
        .request
        .events
        .iter_mut()
        .chain(&mut observation.response.events)
    {
        match (fault, event) {
            (Fault::CallIdentity, SemanticEvent::ToolCall { call_id, .. }) => {
                call_id.push_str("_wrong")
            }
            (Fault::CallName, SemanticEvent::ToolCall { name, .. }) => name.push_str("_wrong"),
            (Fault::Arguments, SemanticEvent::ToolCall { arguments, .. }) => {
                *arguments = Value::Null
            }
            (Fault::ResultAssociation, SemanticEvent::ToolResult { call_id, .. }) => {
                call_id.push_str("_wrong")
            }
            (Fault::ResultValue, SemanticEvent::ToolResult { output, .. }) => *output = Value::Null,
            (Fault::AssistantText, SemanticEvent::AssistantMessage { text }) => {
                text.push_str(" wrong")
            }
            _ => continue,
        }
        return true;
    }
    false
}

/// Measures leave-one-out loss and a minimum cover over this bounded directional fault universe.
fn ablate(observations: &[SemanticObservation]) {
    let matrix: Vec<Vec<bool>> = observations
        .iter()
        .map(|observation| {
            Fault::ALL
                .iter()
                .map(|fault| {
                    let mut changed = observation.clone();
                    if !perturb(&mut changed, *fault) {
                        return false;
                    }
                    assert!(!accepts(&changed), "{} survived {fault:?}", observation.id);
                    true
                })
                .collect()
        })
        .collect();
    // Every declared operator needs a witness; unsupported operators cannot inflate the score.
    for column in 0..Fault::ALL.len() {
        assert!(
            matrix.iter().any(|row| row[column]),
            "unexercised fault {:?}",
            Fault::ALL[column]
        );
    }
    let covers = |mask: usize| {
        observations
            .iter()
            .enumerate()
            .all(|(source, observation)| {
                (0..Fault::ALL.len()).all(|fault| {
                    !matrix[source][fault]
                        || observations.iter().enumerate().any(|(candidate, other)| {
                            mask & (1 << candidate) != 0
                                && other.direction == observation.direction
                                && matrix[candidate][fault]
                        })
                })
            })
    };
    assert!(observations.len() < usize::BITS as usize);
    let full = (1usize << observations.len()) - 1;
    let minimum = (1..=full)
        .filter(|mask| covers(*mask))
        .min_by_key(|mask| mask.count_ones())
        .expect("full suite covers its witnesses");
    for (index, observation) in observations.iter().enumerate() {
        let retained = full & !(1 << index);
        let lost: Vec<_> = Fault::ALL
            .iter()
            .enumerate()
            .filter_map(|(fault, name)| {
                (matrix[index][fault]
                    && !observations.iter().enumerate().any(|(other, candidate)| {
                        retained & (1 << other) != 0
                            && candidate.direction == observation.direction
                            && matrix[other][fault]
                    }))
                .then_some(name)
            })
            .collect();
        println!(
            "ablation {}: detected={:?}; directional losses={lost:?}",
            observation.id,
            Fault::ALL
                .iter()
                .enumerate()
                .filter_map(|(i, fault)| matrix[index][i].then_some(fault))
                .collect::<Vec<_>>()
        );
    }
    println!(
        "bounded directional minimum: {} of {} scenarios",
        minimum.count_ones(),
        observations.len()
    );
    assert_eq!(
        minimum, full,
        "remove redundant scenarios or document a missing independent defect witness"
    );
}
