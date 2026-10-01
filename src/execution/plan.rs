//! Immutable execution plans derived from compiled topology.
//!
//! A plan is data, not a closure. Candidate order is the compiled route order;
//! deriving a plan can never expand, reorder or re-target it.

use crate::topology::{
    CandidatePolicy, CompiledTopology, Endpoint, EndpointId, ModelId, RoutePolicy, TaskKind,
};

/// Downstream interaction requirement, independent of upstream protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    Json,
    Stream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidatePlan {
    pub endpoint_id: EndpointId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    pub model: ModelId,
    pub task: TaskKind,
    pub delivery: Delivery,
    /// Fixed route order. Candidates project independently from immutable input.
    pub candidates: Vec<CandidatePlan>,
    pub policy: RoutePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PlanError {
    #[error("unknown public model label")]
    UnknownModel,
    #[error("task kind does not match the public model")]
    TaskMismatch,
    #[error("request does not satisfy public semantic admission")]
    InvalidRequest,
    #[error("no activated candidate can faithfully represent the request")]
    NoCandidate,
}

/// Preflight is pure: no credentials, network, semantic edits or route selection.
pub fn representable(endpoint: &Endpoint, request: &crate::adapter::Request) -> bool {
    if request.delivery.streaming() && !endpoint.execution.streaming {
        return false;
    }
    endpoint
        .adapter()
        .encode_request(request, &endpoint.upstream_model, &endpoint.representation)
        .ok()
        .and_then(|v| serde_json::to_vec(&v).ok())
        .is_some_and(|body| body.len() <= endpoint.execution.request_body_limit)
}
impl ExecutionPlan {
    pub fn for_request(
        topology: &CompiledTopology,
        request: &crate::adapter::Request,
    ) -> Result<Self, PlanError> {
        let public = topology
            .model(&request.model)
            .ok_or(PlanError::UnknownModel)?;
        super::admit(public, request).map_err(|_| PlanError::InvalidRequest)?;
        let mut plan = Self::derive(
            topology,
            &request.model,
            public.task,
            if request.delivery.streaming() {
                Delivery::Stream
            } else {
                Delivery::Json
            },
        )?;
        let mut eligible = vec![];
        for candidate in &plan.candidates {
            let endpoint = topology
                .endpoint(&candidate.endpoint_id)
                .ok_or(PlanError::NoCandidate)?;
            if representable(endpoint, request) {
                eligible.push(candidate.clone());
            } else if plan.policy.candidates == CandidatePolicy::RequireAll {
                return Err(PlanError::NoCandidate);
            }
        }
        if eligible.is_empty() {
            return Err(PlanError::NoCandidate);
        }
        plan.candidates = eligible;
        Ok(plan)
    }
    /// Derive the plan for one public model label. The label is the only model
    /// input a request supplies; targets, credentials and profiles stay fixed.
    pub fn derive(
        topology: &CompiledTopology,
        model: &str,
        task: TaskKind,
        delivery: Delivery,
    ) -> Result<Self, PlanError> {
        let model_contract = topology.model(model).ok_or(PlanError::UnknownModel)?;
        if model_contract.task != task {
            return Err(PlanError::TaskMismatch);
        }
        let route = topology
            .route(&model_contract.route)
            .ok_or(PlanError::UnknownModel)?;
        Ok(Self {
            model: model_contract.id.clone(),
            task,
            delivery,
            policy: route.policy.clone(),
            candidates: route
                .endpoints
                .iter()
                .map(|endpoint_id| CandidatePlan {
                    endpoint_id: endpoint_id.clone(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::catalog::default_topology;

    #[test]
    fn plan_candidates_follow_fixed_route_order() {
        let topology = default_topology().unwrap();
        let plan = ExecutionPlan::derive(
            &topology,
            "mimo-v2.6-pro",
            TaskKind::Generation,
            Delivery::Stream,
        )
        .unwrap();
        let order: Vec<_> = plan
            .candidates
            .iter()
            .map(|c| c.endpoint_id.as_str())
            .collect();
        assert_eq!(order, ["xiaomi-responses", "xiaomi-chat"]);
    }

    #[test]
    fn unknown_labels_never_plan() {
        let topology = default_topology().unwrap();
        assert_eq!(
            ExecutionPlan::derive(&topology, "gpt-42", TaskKind::Generation, Delivery::Json),
            Err(PlanError::UnknownModel)
        );
        // TaskMismatch becomes observable when a second task domain is admitted;
        // with a single TaskKind the compile-time checks already pin the binding.
    }
}
