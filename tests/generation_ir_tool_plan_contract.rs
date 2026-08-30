//! Trusted ToolPlan injection, stripping, and Provider-native lowering contracts.

use bytes::Bytes;
use openbridge::ir::generation::{
    ChangeKind, ChangeReason, GenerationRequest, InputItem, Instruction, InstructionAuthority,
    InstructionOrigin, JsonSchema, LossPolicy, ParallelToolCalls, ProviderNamespace,
    ProviderOrigin, ProviderServerTool, ProviderToolProfile, ServerToolConfig, ToolChoice,
    ToolDefinition, ToolDirective, ToolDirectiveId, ToolExecutor, ToolKind, ToolName, ToolOrigin,
    ToolPlan, ToolPlanError, ToolPlanId, ToolVisibility, apply_tool_plan, enforce_loss_policy,
    lower_provider_server_tool,
};
use openbridge::{
    bridge::{ProviderToolTarget, StaticBridgePlan, StaticCodecLimits},
    core::{ApiProtocol, ReasoningOutput},
};
use serde_json::json;

fn plan_id(value: &str) -> ToolPlanId {
    ToolPlanId::new(value, 64).unwrap()
}

fn directive_id(value: &str) -> ToolDirectiveId {
    ToolDirectiveId::new(value, 64).unwrap()
}

fn tool_name(value: &str) -> ToolName {
    ToolName::new(value, 64).unwrap()
}

fn provider_origin(value: &str) -> ProviderOrigin {
    ProviderOrigin::new(
        ProviderNamespace::new("openai.responses", 64).unwrap(),
        value,
        128,
    )
    .unwrap()
}

fn provider_profile(origin: ProviderOrigin) -> ProviderToolProfile {
    ProviderToolProfile::new(
        origin,
        [openbridge::ir::generation::ServerToolKind::WebSearch],
    )
}

fn request_with_function_tool() -> GenerationRequest {
    let input = vec![InputItem::Instruction(Instruction::new(
        InstructionAuthority::Developer,
        InstructionOrigin::GatewayPolicy,
        openbridge::ir::generation::TextValue::new("use tools", 64).unwrap(),
    ))];
    let function = openbridge::ir::generation::FunctionTool::new(
        None,
        JsonSchema::new(json!({"type": "object"}), 128).unwrap(),
        false,
    );
    GenerationRequest::new(input)
        .unwrap()
        .with_tools(
            vec![ToolDefinition::new(
                tool_name("lookup"),
                ToolOrigin::Downstream,
                ToolExecutor::Client,
                ToolVisibility::Public,
                ToolKind::Function(function),
            )],
            ToolChoice::Auto,
            ParallelToolCalls::Allow,
        )
        .unwrap()
}

fn provider_web_search(plan: &ToolPlanId, origin: ProviderOrigin) -> ToolDefinition {
    ToolDefinition::new(
        tool_name("web_search"),
        ToolOrigin::GatewayPolicy(plan.clone()),
        ToolExecutor::Provider(origin),
        ToolVisibility::Public,
        ToolKind::Server(ServerToolConfig::WebSearch),
    )
}

#[test]
fn inject_strip_are_authorized_idempotent_and_loss_policy_safe() {
    let plan_identity = plan_id("plan-web-search");
    let origin = provider_origin("target/openai-responses");
    let plan = ToolPlan::new(
        plan_identity.clone(),
        vec![
            ToolDirective::strip(directive_id("strip-lookup"), tool_name("lookup")),
            ToolDirective::inject(
                directive_id("inject-search"),
                provider_web_search(&plan_identity, origin.clone()),
            ),
        ],
    )
    .unwrap();

    let transformed = apply_tool_plan(request_with_function_tool(), &plan).unwrap();
    assert_eq!(transformed.value().tools().len(), 1);
    assert_eq!(transformed.value().tools()[0].name().as_str(), "web_search");
    assert_eq!(transformed.changes().len(), 2);
    assert_eq!(transformed.changes()[0].kind(), ChangeKind::Lossy);
    assert_eq!(
        transformed.changes()[0].reason(),
        ChangeReason::ToolPlanStripping
    );
    assert_eq!(transformed.changes()[1].kind(), ChangeKind::Synthesized);
    assert_eq!(
        transformed.changes()[1].reason(),
        ChangeReason::ToolPlanInjection
    );
    for change in transformed.changes() {
        let (authorized_plan, _) = change.authorization().tool_directive().unwrap();
        assert_eq!(authorized_plan, &plan_identity);
    }
    enforce_loss_policy(&transformed, LossPolicy::Reject).unwrap();

    let repeated = apply_tool_plan(transformed.into_value(), &plan).unwrap();
    assert!(
        repeated.is_exact(),
        "retry must not duplicate injection or stripping"
    );
    assert_eq!(
        lower_provider_server_tool(
            &repeated.value().tools()[0],
            &provider_profile(provider_origin("target/openai-responses"))
        )
        .unwrap(),
        ProviderServerTool::WebSearch
    );

    let (wire_plan, request) = StaticBridgePlan::prepare_with_tool_plan(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"search"}],"tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}],"parallel_tool_calls":true}"#,
        ),
        ProviderToolTarget::new(&plan, &provider_profile(origin), ReasoningOutput::Unsupported),
        StaticCodecLimits::new(256 * 1024, 256 * 1024).unwrap(),
    )
    .unwrap();
    let request: serde_json::Value = serde_json::from_slice(request.body()).unwrap();
    assert_eq!(request["tools"], json!([{"type": "web_search"}]));
    assert!(request.get("parallel_tool_calls").is_none());
    assert!(wire_plan.request_changes().len() >= 3);
}

#[test]
fn tool_plan_rejects_forged_identity_conflicts_and_invalid_lowering() {
    let expected = plan_id("plan-expected");
    let forged = plan_id("plan-forged");
    let origin = provider_origin("target/openai-responses");
    assert!(matches!(
        ToolPlan::new(
            expected.clone(),
            vec![ToolDirective::inject(
                directive_id("inject"),
                provider_web_search(&forged, origin.clone()),
            )],
        ),
        Err(ToolPlanError::PlanIdentityMismatch)
    ));
    assert!(matches!(
        ToolPlan::new(
            expected.clone(),
            vec![
                ToolDirective::strip(directive_id("duplicate"), tool_name("a")),
                ToolDirective::strip(directive_id("duplicate"), tool_name("b")),
            ],
        ),
        Err(ToolPlanError::DuplicateDirectiveId)
    ));

    let gateway_tool = ToolDefinition::new(
        tool_name("web_search"),
        ToolOrigin::GatewayPolicy(expected.clone()),
        ToolExecutor::Gateway,
        ToolVisibility::Internal,
        ToolKind::Server(ServerToolConfig::WebSearch),
    );
    let origin = provider_origin("target/openai-responses");
    assert!(matches!(
        lower_provider_server_tool(&gateway_tool, &provider_profile(origin.clone())),
        Err(ToolPlanError::ProviderOriginMismatch)
    ));

    // A plan cannot inject a declaration naming a different plan identity.
    assert!(matches!(
        ToolPlan::new(
            expected.clone(),
            vec![ToolDirective::inject(
                directive_id("inject-forged"),
                provider_web_search(&forged, origin.clone()),
            )],
        ),
        Err(ToolPlanError::PlanIdentityMismatch)
    ));
}

#[test]
fn plans_reject_same_name_strip_inject_and_specific_choice_survives_only_compatible_kinds() {
    // One plan cannot both strip and inject one tool name.
    let identity = plan_id("plan-conflict");
    let origin = provider_origin("target/openai-responses");
    assert!(matches!(
        ToolPlan::new(
            identity.clone(),
            vec![
                ToolDirective::strip(directive_id("strip"), tool_name("web_search")),
                ToolDirective::inject(
                    directive_id("inject"),
                    provider_web_search(&identity, origin.clone()),
                ),
            ],
        ),
        Err(ToolPlanError::ConflictingDirective)
    ));

    // A plan that only replaces tools is valid, and Specific choice for the stripped function
    // must not survive as a function-shaped choice over a server tool.
    let replace = ToolPlan::new(
        identity,
        vec![ToolDirective::strip(
            directive_id("strip"),
            tool_name("lookup"),
        )],
    )
    .unwrap();
    let chosen = GenerationRequest::new(vec![InputItem::Instruction(Instruction::new(
        InstructionAuthority::Developer,
        InstructionOrigin::Downstream,
        openbridge::ir::generation::TextValue::new("use lookup", 64).unwrap(),
    ))])
    .unwrap();
    let function = openbridge::ir::generation::FunctionTool::new(
        None,
        JsonSchema::new(json!({"type": "object"}), 128).unwrap(),
        false,
    );
    let chosen = chosen
        .with_tools(
            vec![ToolDefinition::new(
                tool_name("lookup"),
                ToolOrigin::Downstream,
                ToolExecutor::Client,
                ToolVisibility::Public,
                ToolKind::Function(function),
            )],
            ToolChoice::Specific(tool_name("lookup")),
            ParallelToolCalls::Inactive,
        )
        .unwrap();
    let transformed = apply_tool_plan(chosen, &replace).unwrap();
    assert!(transformed.value().tools().is_empty());
    // The named choice cannot survive without its tool, and Required is invalid on an empty set.
    assert!(matches!(
        transformed.value().tool_choice(),
        ToolChoice::None
    ));
}

#[test]
fn fallback_candidates_keep_web_search_semantics_equal_while_binding_distinct_origins() {
    let base = GenerationRequest::new(vec![InputItem::Instruction(Instruction::new(
        InstructionAuthority::Developer,
        InstructionOrigin::GatewayPolicy,
        openbridge::ir::generation::TextValue::new("search when needed", 64).unwrap(),
    ))])
    .unwrap();
    let candidates = [
        (
            plan_id("plan-provider-a"),
            provider_origin("target/provider-a"),
        ),
        (
            plan_id("plan-provider-b"),
            provider_origin("target/provider-b"),
        ),
    ];
    let lowered = candidates.map(|(plan_id, origin)| {
        let plan = ToolPlan::new(
            plan_id.clone(),
            vec![ToolDirective::inject(
                directive_id("inject-search"),
                provider_web_search(&plan_id, origin.clone()),
            )],
        )
        .unwrap();
        let transformed = apply_tool_plan(base.clone(), &plan).unwrap();
        assert_eq!(
            transformed.value().tools()[0].visibility(),
            ToolVisibility::Public
        );
        (
            transformed.value().tools()[0].kind().clone(),
            lower_provider_server_tool(&transformed.value().tools()[0], &provider_profile(origin))
                .unwrap(),
        )
    });
    assert_eq!(lowered[0], lowered[1]);
}
