//! 注册表编译前的模型、能力、credential 与 endpoint 规则校验。

use std::collections::{BTreeMap, BTreeSet};

use url::Url;

use super::{
    ModelConfig, ModelContextLength, ModelInfo, ReasoningLevel, ReasoningLevelMapping,
    ReasoningSupport, RegistryError, UpstreamApiModelRules,
};

/// 校验 canonical 模型字段、参数名称和 reasoning 配置的一致性。
pub(super) fn validate_model_config(model: &ModelConfig) -> Result<(), RegistryError> {
    for (field, value) in [("id", model.id.as_str()), ("name", model.name.as_str())] {
        if value.trim().is_empty() {
            return Err(RegistryError::BlankModelField {
                model: model.id.clone(),
                field,
            });
        }
    }
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
    for (limit, value) in [
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
    validate_reasoning_config(&model.id, &model.supported_parameters, model.reasoning).and_then(
        |()| validate_reasoning_levels(&model.id, model.reasoning, &model.reasoning_levels),
    )
}

/// 校验 reasoning level 只在 supported 状态下出现且不重复。
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

/// 校验 reasoning 状态与模型支持参数集合的一致性。
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

/// 将 Upstream API 的收窄规则应用到 canonical 模型事实。
pub(super) fn apply_model_rules(
    model: ModelInfo,
    upstream_api: &str,
    rules: UpstreamApiModelRules,
) -> Result<ModelInfo, RegistryError> {
    // 先验证上下文长度规则不会扩大 canonical 模型上限。
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
    // 计算 reasoning 收窄结果并拒绝能力扩大。
    let reasoning = rules.reasoning.unwrap_or(model.reasoning);
    if reasoning_rank(reasoning) > reasoning_rank(model.reasoning) {
        return Err(RegistryError::UpstreamApiModelRuleWidensModel {
            upstream_api: upstream_api.to_owned(),
            field: "reasoning",
        });
    }
    // 应用参数禁用集合，并拒绝禁用模型未声明的参数。
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
    // 构造有效参数集合并重新验证 reasoning 语义。
    let supported_parameters = model
        .supported_parameters
        .iter()
        .filter(|parameter| !disabled.contains(parameter))
        .cloned()
        .collect::<Vec<_>>();
    validate_effective_reasoning_config(upstream_api, &supported_parameters, reasoning)?;
    Ok(ModelInfo {
        id: model.id,
        name: model.name,
        description: model.description,
        context_length: ModelContextLength::new(
            min_known_limit(
                model.context_length.input_tokens(),
                rules.context_length.input_tokens(),
            ),
            min_known_limit(
                model.context_length.output_tokens(),
                rules.context_length.output_tokens(),
            ),
        ),
        supported_parameters,
        reasoning,
        reasoning_levels: if reasoning == ReasoningSupport::Supported {
            model.reasoning_levels
        } else {
            Vec::new()
        },
    })
}

/// 校验 Upstream API 的单项模型限制为正且不超过 canonical 上限。
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

/// 校验应用收窄规则后的 reasoning 状态和参数集合。
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

/// 校验映射不会扩大 canonical reasoning 契约，并编译为唯一源 level 的只读表。
pub(super) fn validate_reasoning_level_mappings(
    upstream_api: &str,
    model: &ModelInfo,
    mappings: Vec<ReasoningLevelMapping>,
) -> Result<BTreeMap<ReasoningLevel, String>, RegistryError> {
    // 逐项校验源 level 已由有效模型声明，目标是受限 wire 名称。
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

        // 同一源 level 只能映射到一个确定目标，避免候选内出现歧义。
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

/// 合并两个可选限制，并取已知限制中的较小值。
fn min_known_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

/// 将 reasoning 状态映射为可比较的保守性等级。
fn reasoning_rank(reasoning: ReasoningSupport) -> u8 {
    match reasoning {
        ReasoningSupport::Unsupported => 0,
        ReasoningSupport::Unknown => 1,
        ReasoningSupport::Supported => 2,
    }
}

/// 判断模型参数名是否符合内部的受限小写 wire 名称格式。
fn is_valid_parameter_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// 校验并规范化只允许 HTTPS、无凭据和安全 path 前缀的 endpoint base。
pub(super) fn normalize_endpoint_base(value: &str) -> Option<Url> {
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
    if url.path() != "/" && !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Some(url)
}

/// 拒绝 endpoint path 中的 authority 绕过、空段和 dot-segment。
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
