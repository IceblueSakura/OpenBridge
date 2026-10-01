//! Fixed route ordering and the public model contract.

use crate::{
    lowering::generation::ReportedFactPolicy,
    topology::GenerationSemanticContract,
    topology::{EndpointId, ModelId, RouteId, TaskKind},
};

/// Ordered candidate endpoints. The order is fixed at compile time; planning,
/// retry or fallback never expand or reorder it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub id: RouteId,
    pub task: TaskKind,
    pub endpoints: Vec<EndpointId>,
}

/// A public model is an intentional downstream contract for one task, compiled
/// from trusted catalog facts and required to be satisfiable by its route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicModel {
    pub id: ModelId,
    pub canonical_model: ModelId,
    pub task: TaskKind,
    pub route: RouteId,
    pub contract: GenerationSemanticContract,
    pub reported_facts: ReportedFactPolicy,
}
