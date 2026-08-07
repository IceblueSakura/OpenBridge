//! Pre-compilation validation for model, capability, credential, and endpoint rules.

use std::collections::{BTreeMap, BTreeSet};

use url::Url;

use super::{
    ModelConfig, ModelContextLength, ModelInfo, ModelLifecycleStatus, PublicModelConfig,
    ReasoningLevel, ReasoningLevelMapping, ReasoningSupport, RegistryError, UpstreamApiModelRules,
};

/// Validates canonical-model fields, parameter names, and reasoning configuration.
pub(super) fn validate_model_config(model: &ModelConfig) -> Result<(), RegistryError> {
    // Validate that model identity and display fields are not blank.
    for (field, value) in [("id", model.id.as_str()), ("name", model.name.as_str())] {
        if value.trim().is_empty() {
            return Err(RegistryError::BlankModelField {
                model: model.id.clone(),
                field,
            });
        }
    }
    validate_namespaced_model_id(&model.id, "id")?;
    if model
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty())
    {
        return Err(RegistryError::BlankModelField {
            model: model.id.clone(),
            field: "description",
        });
    }
    // Validate optional catalog strings without treating an absent fact as an error.
    for (field, value) in [
        ("tokenizer", model.tokenizer.as_deref()),
        ("knowledge_cutoff", model.knowledge_cutoff.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(RegistryError::BlankModelField {
                model: model.id.clone(),
                field,
            });
        }
    }
    // Validate that explicit model modalities are non-empty, duplicate-free known facts.
    validate_model_modalities(
        &model.id,
        "input_modalities",
        model.input_modalities.as_deref(),
    )?;
    validate_model_modalities(
        &model.id,
        "output_modalities",
        model.output_modalities.as_deref(),
    )?;
    // Validate that known context limits are positive.
    for (limit, value) in [
        ("context", model.context_length.context_tokens()),
        ("input", model.context_length.input_tokens()),
        ("output", model.context_length.output_tokens()),
    ] {
        if value == Some(0) {
            return Err(RegistryError::InvalidModelContextLength {
                model: model.id.clone(),
                limit,
            });
        }
    }
    if model.context_length.input_tokens().is_some_and(|input| {
        model
            .context_length
            .context_tokens()
            .is_some_and(|context| input > context)
    }) || model.context_length.output_tokens().is_some_and(|output| {
        model
            .context_length
            .context_tokens()
            .is_some_and(|context| output > context)
    }) {
        return Err(RegistryError::InconsistentModelContextLength {
            model: model.id.clone(),
        });
    }
    // Validate supported parameter-name format and uniqueness.
    let mut seen = BTreeSet::new();
    for parameter in &model.supported_parameters {
        if !is_valid_parameter_name(parameter) {
            return Err(RegistryError::InvalidSupportedParameter {
                model: model.id.clone(),
                parameter: parameter.clone(),
            });
        }
        if !seen.insert(parameter) {
            return Err(RegistryError::DuplicateSupportedParameter {
                model: model.id.clone(),
                parameter: parameter.clone(),
            });
        }
    }
    // Validate consistency among reasoning state, parameter declarations, and levels.
    validate_reasoning_config(&model.id, &model.supported_parameters, model.reasoning).and_then(
        |()| validate_reasoning_levels(&model.id, model.reasoning, &model.reasoning_levels),
    )
}

/// Validates a model identity used by the definition or routing layer.
pub(super) fn validate_namespaced_model_id(
    model: &str,
    field: &'static str,
) -> Result<(), RegistryError> {
    let mut segments = model.split('/');
    let namespace = segments.next().unwrap_or_default();
    let name = segments.next().unwrap_or_default();
    if namespace.trim().is_empty() || name.trim().is_empty() || segments.next().is_some() {
        return Err(RegistryError::InvalidNamespacedModelId {
            model: model.to_owned(),
            field,
        });
    }
    Ok(())
}

/// Validates that an explicit modality set is non-empty and duplicate-free.
fn validate_model_modalities<T: Copy + Ord>(
    model: &str,
    field: &'static str,
    modalities: Option<&[T]>,
) -> Result<(), RegistryError> {
    let Some(modalities) = modalities else {
        return Ok(());
    };
    let mut seen = BTreeSet::new();
    if modalities.is_empty() || modalities.iter().any(|modality| !seen.insert(*modality)) {
        return Err(RegistryError::InconsistentModelCapabilities {
            model: model.to_owned(),
            field,
        });
    }
    Ok(())
}

/// Validates Public Model public identity, stable timestamps, and lifecycle consistency.
pub(super) fn validate_public_model_config(model: &PublicModelConfig) -> Result<(), RegistryError> {
    // Validate that the public ID is a stable identifier safe for one URL path segment.
    let mut characters = model.id.chars();
    let valid_id = model.id.len() <= 128
        && characters
            .next()
            .is_some_and(|value| value.is_ascii_alphanumeric())
        && characters.all(|value| value.is_ascii_alphanumeric() || "._:-".contains(value));
    if !valid_id {
        return Err(RegistryError::InvalidPublicModelId {
            public_model: model.id.clone(),
        });
    }

    // Validate display fields and the stable creation time.
    if model.display_name.trim().is_empty() {
        return Err(RegistryError::BlankPublicModelField {
            public_model: model.id.clone(),
            field: "display_name",
        });
    }
    if model
        .description
        .as_deref()
        .is_some_and(|description| description.trim().is_empty())
    {
        return Err(RegistryError::BlankPublicModelField {
            public_model: model.id.clone(),
            field: "description",
        });
    }
    if model.created == 0 {
        return Err(RegistryError::InvalidPublicModelCreated {
            public_model: model.id.clone(),
        });
    }

    // Validate that lifecycle timestamps are not earlier than creation and match the status.
    let invalid_lifecycle = match model.lifecycle.status {
        ModelLifecycleStatus::Active => {
            model.lifecycle.deprecated_at.is_some() || model.lifecycle.retired_at.is_some()
        }
        ModelLifecycleStatus::Deprecated => {
            model
                .lifecycle
                .deprecated_at
                .is_none_or(|deprecated| deprecated < model.created)
                || model.lifecycle.retired_at.is_some()
        }
        ModelLifecycleStatus::Retired => {
            model
                .lifecycle
                .retired_at
                .is_none_or(|retired| retired < model.created)
                || model.lifecycle.deprecated_at.is_some_and(|deprecated| {
                    deprecated < model.created
                        || model
                            .lifecycle
                            .retired_at
                            .is_some_and(|retired| deprecated > retired)
                })
        }
    };
    if invalid_lifecycle {
        return Err(RegistryError::InvalidPublicModelLifecycle {
            public_model: model.id.clone(),
        });
    }
    Ok(())
}

/// Validates that reasoning levels appear only in the supported state and are unique.
fn validate_reasoning_levels(
    model: &str,
    reasoning: ReasoningSupport,
    levels: &[ReasoningLevel],
) -> Result<(), RegistryError> {
    if reasoning != ReasoningSupport::Supported && !levels.is_empty() {
        return Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning levels require reasoning = supported",
        });
    }
    let mut seen = BTreeSet::new();
    if levels.iter().any(|level| !seen.insert(*level)) {
        return Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning levels must not contain duplicates",
        });
    }
    Ok(())
}

/// Validates consistency between reasoning state and the model's supported-parameter set.
fn validate_reasoning_config(
    model: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning = supported requires supported_parameters to include reasoning",
        }),
        (ReasoningSupport::Unsupported, true) => Err(RegistryError::InconsistentReasoningConfig {
            model: model.to_owned(),
            detail: "reasoning = unsupported conflicts with supported_parameters",
        }),
        _ => Ok(()),
    }
}

/// Applies Upstream API narrowing rules to canonical model facts.
pub(super) fn apply_model_rules(
    model: ModelInfo,
    upstream_api: &str,
    rules: UpstreamApiModelRules,
) -> Result<ModelInfo, RegistryError> {
    // First verify that context rules do not expand the canonical model ceiling.
    validate_model_limit(
        upstream_api,
        "context_length.context",
        model.context_length.context_tokens(),
        rules.context_length.context_tokens(),
    )?;
    validate_model_limit(
        upstream_api,
        "context_length.input",
        model.context_length.input_tokens(),
        rules.context_length.input_tokens(),
    )?;
    validate_model_limit(
        upstream_api,
        "context_length.output",
        model.context_length.output_tokens(),
        rules.context_length.output_tokens(),
    )?;
    // Compute the narrowed reasoning state and reject capability expansion.
    let reasoning = rules.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(RegistryError::UpstreamApiModelRuleWidensModel {
            upstream_api: upstream_api.to_owned(),
            field: "reasoning",
        });
    }
    // Apply disabled parameters and reject parameters not declared by the model.
    let disabled = rules.disabled_parameters.iter().collect::<BTreeSet<_>>();
    for parameter in &disabled {
        if !model.supported_parameters.contains(parameter) {
            return Err(
                RegistryError::UpstreamApiModelRuleDisablesUnknownParameter {
                    upstream_api: upstream_api.to_owned(),
                    parameter: (*parameter).clone(),
                },
            );
        }
    }
    // Build the effective parameter set and revalidate reasoning semantics.
    let supported_parameters = model
        .supported_parameters
        .iter()
        .filter(|parameter| !disabled.contains(parameter))
        .cloned()
        .collect::<Vec<_>>();
    validate_effective_reasoning_config(upstream_api, &supported_parameters, reasoning)?;
    let context_length = ModelContextLength::new(
        min_known_limit(
            model.context_length.context_tokens(),
            rules.context_length.context_tokens(),
        ),
        min_known_limit(
            model.context_length.input_tokens(),
            rules.context_length.input_tokens(),
        ),
        min_known_limit(
            model.context_length.output_tokens(),
            rules.context_length.output_tokens(),
        ),
    );
    if context_length.input_tokens().is_some_and(|input| {
        context_length
            .context_tokens()
            .is_some_and(|context| input > context)
    }) || context_length.output_tokens().is_some_and(|output| {
        context_length
            .context_tokens()
            .is_some_and(|context| output > context)
    }) {
        return Err(RegistryError::InconsistentUpstreamApiModelRules {
            upstream_api: upstream_api.to_owned(),
            detail: "effective input or output limit exceeds the total context window",
        });
    }
    Ok(ModelInfo {
        id: model.id,
        name: model.name,
        description: model.description,
        context_length,
        mode: model.mode,
        input_modalities: model.input_modalities,
        output_modalities: model.output_modalities,
        tokenizer: model.tokenizer,
        knowledge_cutoff: model.knowledge_cutoff,
        supported_parameters,
        reasoning,
        reasoning_levels: if reasoning == ReasoningSupport::Supported {
            model.reasoning_levels
        } else {
            Vec::new()
        },
    })
}

/// Validates that one Upstream API model limit is positive and within the canonical ceiling.
fn validate_model_limit(
    upstream_api: &str,
    field: &'static str,
    model_limit: Option<u32>,
    constraint_limit: Option<u32>,
) -> Result<(), RegistryError> {
    let Some(constraint_limit) = constraint_limit else {
        return Ok(());
    };
    if constraint_limit == 0 {
        return Err(RegistryError::InvalidUpstreamApiModelRule {
            upstream_api: upstream_api.to_owned(),
            field,
        });
    }
    if model_limit.is_some_and(|model_limit| constraint_limit > model_limit) {
        return Err(RegistryError::UpstreamApiModelLimitExceedsModel {
            upstream_api: upstream_api.to_owned(),
            field,
        });
    }
    Ok(())
}

/// Validates reasoning state and parameters after narrowing rules are applied.
fn validate_effective_reasoning_config(
    upstream_api: &str,
    parameters: &[String],
    reasoning: ReasoningSupport,
) -> Result<(), RegistryError> {
    let declared = parameters.iter().any(|parameter| parameter == "reasoning");
    match (reasoning, declared) {
        (ReasoningSupport::Supported, false) => {
            Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning = supported requires the effective parameter set to include reasoning",
            })
        }
        (ReasoningSupport::Unsupported, true) => {
            Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning = unsupported conflicts with the effective parameter set",
            })
        }
        _ => Ok(()),
    }
}

/// Validates that mappings do not expand the canonical reasoning contract and compiles a read-only
/// table with one target per source level.
pub(super) fn validate_reasoning_level_mappings(
    upstream_api: &str,
    model: &ModelInfo,
    mappings: Vec<ReasoningLevelMapping>,
) -> Result<BTreeMap<ReasoningLevel, String>, RegistryError> {
    // Verify each source level is declared by the effective model and each target is a restricted wire name.
    let mut resolved = BTreeMap::new();
    for mapping in mappings {
        if model.reasoning() != ReasoningSupport::Supported
            || !model.reasoning_levels().contains(&mapping.downstream)
        {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping source must be supported by the effective model",
            });
        }
        if !is_valid_parameter_name(&mapping.upstream) {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping target must be a safe wire name",
            });
        }

        // Each source level may map to one fixed target only, avoiding ambiguity within a candidate.
        if resolved
            .insert(mapping.downstream, mapping.upstream)
            .is_some()
        {
            return Err(RegistryError::InconsistentUpstreamApiModelRules {
                upstream_api: upstream_api.to_owned(),
                detail: "reasoning level mapping sources must be unique",
            });
        }
    }
    Ok(resolved)
}

/// Merges two optional limits and returns the smaller known limit.
fn min_known_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

/// Maps a reasoning state to a comparable conservatism rank.
fn reasoning_rank(reasoning: ReasoningSupport) -> u8 {
    match reasoning {
        ReasoningSupport::Unsupported => 0,
        ReasoningSupport::Unknown => 1,
        ReasoningSupport::Supported => 2,
    }
}

/// Returns whether a model parameter name matches the restricted lowercase wire format.
fn is_valid_parameter_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Validates and normalizes an endpoint base that allows only HTTPS, no credentials, and a safe path prefix.
pub(super) fn normalize_endpoint_base(value: &str) -> Option<Url> {
    // Parse the endpoint and reject invalid scheme, credentials, query, fragment, or host/path shapes.
    let mut url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !is_safe_endpoint_prefix(url.path())
    {
        return None;
    }
    // Normalize the trailing slash so later relative-URI joins preserve the directory prefix.
    if url.path() != "/" && !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Some(url)
}

/// Rejects authority bypasses, empty segments, and dot segments in an endpoint path.
fn is_safe_endpoint_prefix(path: &str) -> bool {
    if path.is_empty() || path == "/" {
        return true;
    }
    if !path.starts_with('/') || path.contains("//") {
        return false;
    }
    path.trim_matches('/').split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
            })
    })
}
