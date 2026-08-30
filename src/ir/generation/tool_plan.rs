//! Trusted immutable tool plans and pure candidate-specific server-tool lowering.

use std::collections::BTreeSet;

use thiserror::Error;

use super::{
    ChangeAuthorization, ChangeKind, ChangeReason, GenerationRequest, ParallelToolCalls,
    ProviderOrigin, SemanticChange, SemanticPath, ServerToolConfig, ServerToolKind, ToolChoice,
    ToolDefinition, ToolDirectiveId, ToolExecutor, ToolKind, ToolName, ToolOrigin, ToolPlanId,
    Transform,
};

/// One trusted immutable ToolPlan directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolDirective {
    id: ToolDirectiveId,
    action: ToolDirectiveAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ToolDirectiveAction {
    Inject(Box<ToolDefinition>),
    Strip(ToolName),
}

impl ToolDirective {
    /// Creates one directive that injects a complete canonical declaration.
    pub fn inject(id: ToolDirectiveId, tool: ToolDefinition) -> Self {
        Self {
            id,
            action: ToolDirectiveAction::Inject(Box::new(tool)),
        }
    }

    /// Creates one directive that strips a named canonical declaration.
    pub fn strip(id: ToolDirectiveId, tool: ToolName) -> Self {
        Self {
            id,
            action: ToolDirectiveAction::Strip(tool),
        }
    }

    /// Returns the stable directive identity.
    pub fn id(&self) -> &ToolDirectiveId {
        &self.id
    }
}

/// Trusted ordered ToolPlan compiled outside business requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolPlan {
    id: ToolPlanId,
    directives: Vec<ToolDirective>,
}

impl ToolPlan {
    /// Creates a plan after validating directive identity and injected declaration provenance.
    pub fn new(id: ToolPlanId, directives: Vec<ToolDirective>) -> Result<Self, ToolPlanError> {
        if directives.is_empty() {
            return Err(ToolPlanError::EmptyPlan);
        }
        let mut identities = BTreeSet::new();
        let mut stripped = BTreeSet::new();
        let mut injected = BTreeSet::new();
        for directive in &directives {
            if !identities.insert(directive.id().as_str()) {
                return Err(ToolPlanError::DuplicateDirectiveId);
            }
            match &directive.action {
                ToolDirectiveAction::Inject(tool) => {
                    injected.insert(tool.name().as_str().to_owned());
                    if !matches!(tool.origin(), ToolOrigin::GatewayPolicy(plan) if plan == &id) {
                        return Err(ToolPlanError::PlanIdentityMismatch);
                    }
                    if matches!(tool.executor(), ToolExecutor::Client) {
                        return Err(ToolPlanError::InvalidInjectedExecutor);
                    }
                }
                ToolDirectiveAction::Strip(name) => {
                    stripped.insert(name.as_str().to_owned());
                }
            }
        }
        // One plan cannot both strip and inject one name; the outcome would depend on order and
        // break the documented idempotent-retry guarantee.
        if !stripped.is_disjoint(&injected) {
            return Err(ToolPlanError::ConflictingDirective);
        }
        Ok(Self { id, directives })
    }

    /// Returns the stable plan identity.
    pub fn id(&self) -> &ToolPlanId {
        &self.id
    }

    /// Returns directives in deterministic application order.
    pub fn directives(&self) -> &[ToolDirective] {
        &self.directives
    }
}

/// Closed Provider-native server-tool DTO selected only after origin matching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderServerTool {
    /// Native Responses web-search declaration.
    WebSearch,
}

/// Trusted Provider-native tool support for one fixed candidate origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderToolProfile {
    origin: ProviderOrigin,
    server_tools: BTreeSet<ServerToolKind>,
}

impl ProviderToolProfile {
    /// Creates one closed candidate profile from startup-compiled capability facts.
    pub fn new(
        origin: ProviderOrigin,
        server_tools: impl IntoIterator<Item = ServerToolKind>,
    ) -> Self {
        Self {
            origin,
            server_tools: server_tools.into_iter().collect(),
        }
    }

    /// Returns the fixed candidate origin.
    pub fn origin(&self) -> &ProviderOrigin {
        &self.origin
    }

    /// Returns whether this candidate explicitly accepts one server-tool kind.
    pub fn supports(&self, kind: ServerToolKind) -> bool {
        self.server_tools.contains(&kind)
    }
}

/// ToolPlan validation, application, or Provider-lowering failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolPlanError {
    /// A plan without directives has no policy meaning.
    #[error("trusted ToolPlan must contain at least one directive")]
    EmptyPlan,
    /// Directive identities must be unique within one plan.
    #[error("trusted ToolPlan contains a duplicate directive identity")]
    DuplicateDirectiveId,
    /// An injected declaration names a different plan as its origin.
    #[error("injected tool origin does not match its ToolPlan")]
    PlanIdentityMismatch,
    /// Trusted plans cannot fabricate client-owned execution.
    #[error("trusted ToolPlan cannot inject a client-executed tool")]
    InvalidInjectedExecutor,
    /// One plan strips and injects the same tool name without a deterministic outcome.
    #[error("trusted ToolPlan strips and injects the same tool name")]
    ConflictingDirective,
    /// An injection conflicts with a different declaration under the same name.
    #[error("trusted ToolPlan injection conflicts with an existing tool name")]
    ConflictingToolName,
    /// Applying the directives would violate canonical tool-choice or parallelism invariants.
    #[error("trusted ToolPlan produces an invalid canonical request")]
    InvalidRequest,
    /// Provider execution is not bound to the selected target origin.
    #[error("server tool executor does not match the selected Provider origin")]
    ProviderOriginMismatch,
    /// The selected Provider-native profile cannot lower this server tool.
    #[error("server tool has no supported Provider-native lowering")]
    UnsupportedProviderTool,
}

/// Applies one trusted plan without mutating the source request.
///
/// Reapplying the same plan to its own output is exact: identical injections and absent strips are
/// no-ops, so retries cannot duplicate declarations or fidelity records.
pub fn apply_tool_plan(
    request: GenerationRequest,
    plan: &ToolPlan,
) -> Result<Transform<GenerationRequest>, ToolPlanError> {
    let mut tools = request.tools().to_vec();
    let mut changes = Vec::new();
    let mut replaced = BTreeSet::new();
    for directive in plan.directives() {
        match &directive.action {
            ToolDirectiveAction::Inject(tool) => {
                if let Some(existing) = tools.iter().find(|existing| existing.name() == tool.name())
                {
                    if existing == tool.as_ref() {
                        continue;
                    }
                    return Err(ToolPlanError::ConflictingToolName);
                }
                tools.push((**tool).clone());
                changes.push(directive_change(
                    plan,
                    directive,
                    tool.name(),
                    ChangeKind::Synthesized,
                    ChangeReason::ToolPlanInjection,
                ));
            }
            ToolDirectiveAction::Strip(name) => {
                let Some(index) = tools.iter().position(|tool| tool.name() == name) else {
                    continue;
                };
                if let ToolChoice::Specific(chosen) = request.tool_choice()
                    && chosen == name
                {
                    replaced.insert(name.clone());
                }
                tools.remove(index);
                changes.push(directive_change(
                    plan,
                    directive,
                    name,
                    ChangeKind::Lossy,
                    ChangeReason::ToolPlanStripping,
                ));
            }
        }
    }
    let mut tool_choice = request.tool_choice().clone();
    if !replaced.is_empty() && tools.is_empty() {
        // A stripped named choice cannot survive without tools; Required is invalid on an empty set.
        tool_choice = ToolChoice::None;
    }
    let parallel = if tools
        .iter()
        .any(|tool| matches!(tool.kind(), ToolKind::Function(_)))
    {
        request.parallel_tool_calls()
    } else {
        ParallelToolCalls::Inactive
    };
    let value = request
        .with_tools(tools, tool_choice, parallel)
        .map_err(|_| ToolPlanError::InvalidRequest)?;
    Ok(Transform::new(value, changes))
}

/// Lowers a canonical server tool only when its Provider executor matches the fixed target origin.
pub fn lower_provider_server_tool(
    tool: &ToolDefinition,
    target: &ProviderToolProfile,
) -> Result<ProviderServerTool, ToolPlanError> {
    if !matches!(tool.executor(), ToolExecutor::Provider(origin) if origin == target.origin()) {
        return Err(ToolPlanError::ProviderOriginMismatch);
    }
    match tool.kind() {
        ToolKind::Server(ServerToolConfig::WebSearch)
            if target.supports(ServerToolKind::WebSearch) =>
        {
            Ok(ProviderServerTool::WebSearch)
        }
        ToolKind::Function(_) | ToolKind::Server(_) | ToolKind::Extension(_) => {
            Err(ToolPlanError::UnsupportedProviderTool)
        }
    }
}

fn directive_change(
    plan: &ToolPlan,
    directive: &ToolDirective,
    tool: &ToolName,
    kind: ChangeKind,
    reason: ChangeReason,
) -> SemanticChange {
    let path = SemanticPath::new(format!("tools[{}]", tool.as_str()));
    let authorization = ChangeAuthorization::from_tool_directive(
        plan.id().clone(),
        directive.id().clone(),
        path.clone(),
        reason,
    );
    SemanticChange::new(path, kind, reason, authorization)
}
