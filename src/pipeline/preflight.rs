//! Validates request facts against one Public Model interface before Route planning.
//!
//! Preflight owns only the fixed downstream contract. It does not inspect candidate-specific
//! capabilities, apply Provider wire mappings, or influence configured Route order.

use crate::registry::{
    ModelExecutionInterface, ModelInterfaceCapabilities, ReasoningLevel, RuntimeRegistry,
    SupportState,
};

use super::{
    error::RequestPlanningError,
    types::{RequestRequirements, RequestedCapabilities, RequestedReasoning},
};

/// Resolves the selected Public Model and validates the request against its compiled protocol interface.
pub(super) fn preflight_public_model<'a>(
    registry: &'a RuntimeRegistry,
    requirements: &RequestRequirements,
) -> Result<&'a ModelExecutionInterface, RequestPlanningError> {
    // Resolve the downstream model and its precompiled protocol interface without consulting any Route candidate.
    let public_model = registry
        .public_model(requirements.public_model())
        .ok_or(RequestPlanningError::UnknownModel)?;
    let interface = public_model
        .execution_interface(requirements.protocol().operation())
        .ok_or(RequestPlanningError::UnsupportedProtocol)?;

    // Validate every modeled request fact against the single fixed interface contract.
    validate_interface_request(
        requirements.requested_capabilities,
        requirements.requested_output_tokens,
        interface.capabilities(),
    )?;
    Ok(interface)
}

/// Returns the most specific fail-closed error from the fixed interface contract.
fn validate_interface_request(
    requested_features: RequestedCapabilities,
    requested_output_tokens: Option<u64>,
    interface: &ModelInterfaceCapabilities,
) -> Result<(), RequestPlanningError> {
    // Validate shared generation and state capabilities before any egress preparation.
    if requested_features.unmodeled_tools {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }
    if requested_features.generation.streaming && !interface.supports_streaming() {
        return Err(RequestPlanningError::StreamingUnsupported);
    }
    if (requested_features.generation.function_calling && !interface.supports_function_calling())
        || (requested_features.generation.parallel_tool_calls
            && !interface.supports_parallel_tool_calls())
        || (requested_features.generation.image_input && !interface.supports_image_input())
        || (requested_features.generation.structured_outputs
            && !interface.supports_structured_outputs())
        || (requested_features.generation.store && !interface.supports_store())
        || (requested_features.previous_response_id && !interface.supports_previous_response_id())
        || (requested_features.background && !interface.supports_background())
    {
        return Err(RequestPlanningError::UnsupportedCapabilities);
    }

    // Enforce the fixed output limit when the request carries an explicit value.
    if interface.max_output_tokens().is_some_and(|limit| {
        requested_output_tokens.is_some_and(|requested| requested > u64::from(limit))
    }) {
        return Err(RequestPlanningError::OutputLimitExceeded);
    }

    // Validate reasoning support and the fixed public level set without applying Provider mappings.
    match requested_features.reasoning {
        RequestedReasoning::None | RequestedReasoning::Level(ReasoningLevel::None) => {}
        RequestedReasoning::Unspecified
            if interface.reasoning_support() != SupportState::Supported =>
        {
            return Err(RequestPlanningError::ReasoningUnsupported);
        }
        RequestedReasoning::Level(level)
            if interface.reasoning_support() != SupportState::Supported
                || !interface.reasoning_levels().contains(&level) =>
        {
            return Err(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::UnknownLevel => {
            return Err(RequestPlanningError::ReasoningLevelUnsupported);
        }
        RequestedReasoning::Conflicting => {
            return Err(RequestPlanningError::InvalidReasoningConfiguration);
        }
        RequestedReasoning::Unspecified | RequestedReasoning::Level(_) => {}
    }
    Ok(())
}
